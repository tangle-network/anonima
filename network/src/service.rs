// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::{ForestBehaviour, ForestBehaviourEvent, Libp2pConfig};
use crate::hello::{HelloRequest, HelloResponse};
use crate::rpc::RequestResponseError;
use anonima_encoding::from_slice;
use async_std::channel::{unbounded, Receiver, Sender};
use async_std::stream;
use futures::channel::oneshot::Sender as OneShotSender;
use futures::select;
use futures_util::stream::StreamExt;
pub use libp2p::gossipsub::{IdentTopic, Topic};
use message::SignedMessage;

use libp2p::core::connection::ConnectionLimits;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::core::transport::Boxed;
use libp2p::core::Multiaddr;
use libp2p::identity::{ed25519, Keypair};
use libp2p::swarm::SwarmBuilder;
use libp2p::{core, mplex, noise, yamux, PeerId, Swarm, Transport};
use log::{debug, error, info, trace, warn};

use crate::utils::read_file_to_vec;
use std::time::Duration;

/// Gossipsub Filecoin blocks topic identifier.
pub const PUBSUB_DKG_STR: &str = "/anonima/dkg";
/// Gossipsub Filecoin messages topic identifier.
pub const PUBSUB_MSG_STR: &str = "/anonima/msgs";

// const PUBSUB_TOPICS: [&str; 2] = [PUBSUB_DKG_STR, PUBSUB_MSG_STR];

/// Events emitted by this Service.
#[derive(Debug)]
pub enum NetworkEvent {
    PubsubMessage {
        source: PeerId,
        message: PubsubMessage,
    },
    HelloRequest {
        request: HelloRequest,
        source: PeerId,
    },
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

/// Message types that can come over GossipSub
#[derive(Debug, Clone)]
pub enum PubsubMessage {
    /// Messages that come over the message topic
    Message(SignedMessage),
}

/// Messages into the service to handle.
#[derive(Debug)]
pub enum NetworkMessage {
    PubsubMessage {
        topic: IdentTopic,
        message: Vec<u8>,
    },
    HelloRequest {
        peer_id: PeerId,
        request: HelloRequest,
        response_channel: OneShotSender<Result<HelloResponse, RequestResponseError>>,
    },
    JSONRPCRequest {
        method: NetRPCMethods,
    },
}

/// Network RPC API methods used to gather data from libp2p node.
#[derive(Debug)]
pub enum NetRPCMethods {
    NetAddrsListen(OneShotSender<(PeerId, Vec<Multiaddr>)>),
}

/// The Libp2pService listens to events from the Libp2p swarm.
pub struct Libp2pService {
    swarm: Swarm<ForestBehaviour>,
    network_receiver_in: Receiver<NetworkMessage>,
    network_sender_in: Sender<NetworkMessage>,
    network_receiver_out: Receiver<NetworkEvent>,
    network_sender_out: Sender<NetworkEvent>,
    network_name: String,
}

impl Libp2pService {
    pub fn new(config: Libp2pConfig, net_keypair: Keypair, network_name: &str) -> Self {
        let peer_id = PeerId::from(net_keypair.public());

        let transport = build_transport(net_keypair.clone());

        let limits = ConnectionLimits::default()
            .with_max_pending_incoming(Some(10))
            .with_max_pending_outgoing(Some(30))
            .with_max_established_incoming(Some(config.target_peer_count))
            .with_max_established_outgoing(Some(config.target_peer_count))
            .with_max_established_per_peer(Some(5));

        let mut swarm = SwarmBuilder::new(
            transport,
            ForestBehaviour::new(&net_keypair, &config, network_name),
            peer_id,
        )
        .connection_limits(limits)
        .notify_handler_buffer_size(std::num::NonZeroUsize::new(20).expect("Not zero"))
        .connection_event_buffer_size(64)
        .build();

        Swarm::listen_on(&mut swarm, config.listening_multiaddr).unwrap();

        // Bootstrap with Kademlia
        if let Err(e) = swarm.bootstrap() {
            warn!("Failed to bootstrap with Kademlia: {}", e);
        }

        let (network_sender_in, network_receiver_in) = unbounded();
        let (network_sender_out, network_receiver_out) = unbounded();

        Libp2pService {
            swarm,
            network_receiver_in,
            network_sender_in,
            network_receiver_out,
            network_sender_out,
            network_name: network_name.to_owned(),
        }
    }

    /// Starts the libp2p service networking stack. This Future resolves when
    /// shutdown occurs.
    pub async fn run(self) {
        let mut swarm_stream = self.swarm.fuse();
        let mut network_stream = self.network_receiver_in.fuse();
        let mut interval = stream::interval(Duration::from_secs(5)).fuse();
        let _pubsub_dkg_str = format!("{}/{}", PUBSUB_DKG_STR, self.network_name);
        let pubsub_msg_str = format!("{}/{}", PUBSUB_MSG_STR, self.network_name);
        loop {
            futures::select! {
                swarm_event = swarm_stream.next() => match swarm_event {
                    // outbound events
                    Some(event) => match event {
                        ForestBehaviourEvent::PeerConnected(peer_id) => {
                            debug!("Peer connected, {:?}", peer_id);
                            emit_event(&self.network_sender_out,
                                NetworkEvent::PeerConnected(peer_id)).await;
                            emit_event(&self.network_sender_out,
                                NetworkEvent::HelloRequest {
                                    source: peer_id,
                                    request: Default::default(),
                                }).await;
                        }
                        ForestBehaviourEvent::PeerDisconnected(peer_id) => {
                            emit_event(&self.network_sender_out, NetworkEvent::PeerDisconnected(peer_id)).await;
                        }
                        ForestBehaviourEvent::GossipMessage {
                            source,
                            topic,
                            message,
                        } => {
                            trace!("Got a Gossip Message from {:?}", source);
                            let topic = topic.as_str();
                            if topic == pubsub_msg_str {
                                match from_slice::<SignedMessage>(&message) {
                                    Ok(m) => {
                                        emit_event(&self.network_sender_out, NetworkEvent::PubsubMessage{
                                            source,
                                            message: PubsubMessage::Message(m),
                                        }).await;
                                    }
                                    Err(e) => {
                                        warn!("Gossip Message from peer {:?} could not be deserialized: {}", source, e);
                                    }
                                }
                            } else {
                                warn!("Getting gossip messages from unknown topic: {}", topic);
                            }
                        }
                        ForestBehaviourEvent::HelloRequest { request,  peer } => {
                            debug!("Received hello request (peer_id: {:?})", peer);
                            emit_event(&self.network_sender_out, NetworkEvent::HelloRequest {
                                request,
                                source: peer,
                            }).await;
                        }
                    }
                    None => { break; }
                },
                rpc_message = network_stream.next() => match rpc_message {
                    // Inbound messages
                    Some(message) =>  match message {
                        NetworkMessage::PubsubMessage { topic, message } => {
                            if let Err(e) = swarm_stream.get_mut().publish(topic, message) {
                                warn!("Failed to send gossipsub message: {:?}", e);
                            }
                        }
                        NetworkMessage::HelloRequest { peer_id, request, response_channel } => {
                            swarm_stream.get_mut().send_hello_request(&peer_id, request, response_channel);
                        }
                        NetworkMessage::JSONRPCRequest { method } => {
                            match method {
                                NetRPCMethods::NetAddrsListen(response_channel) => {
                                let listeners: Vec<_> = Swarm::listeners( swarm_stream.get_mut()).cloned().collect();
                                let peer_id = Swarm::local_peer_id(swarm_stream.get_mut());
                                    if response_channel.send((*peer_id, listeners)).is_err() {
                                        warn!("Failed to get Libp2p listeners");
                                    }
                                }
                            }
                        }
                    }
                    None => { break; }
                },
                interval_event = interval.next() => if interval_event.is_some() {
                    // Print peer count on an interval.
                    info!("Peers connected: {}", swarm_stream.get_mut().peers().len());
                }
            };
        }
    }

    /// Returns a sender which allows sending messages to the libp2p service.
    pub fn network_sender(&self) -> Sender<NetworkMessage> {
        self.network_sender_in.clone()
    }

    /// Returns a receiver to listen to network events emitted from the service.
    pub fn network_receiver(&self) -> Receiver<NetworkEvent> {
        self.network_receiver_out.clone()
    }
}

async fn emit_event(sender: &Sender<NetworkEvent>, event: NetworkEvent) {
    debug!("Sending {:?} to peers", event);
    if sender.send(event).await.is_err() {
        error!("Failed to emit event: Network channel receiver has been dropped");
    }
}

/// Builds the transport stack that LibP2P will communicate over.
pub fn build_transport(local_key: Keypair) -> Boxed<(PeerId, StreamMuxerBox)> {
    let transport = libp2p::tcp::TcpConfig::new().nodelay(true);
    let transport = libp2p::websocket::WsConfig::new(transport.clone()).or_transport(transport);
    let transport = libp2p::dns::DnsConfig::new(transport).unwrap();

    let auth_config = {
        let dh_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&local_key)
            .expect("Noise key generation failed");

        noise::NoiseConfig::xx(dh_keys).into_authenticated()
    };

    let mplex_config = {
        let mut mplex_config = mplex::MplexConfig::new();
        mplex_config.set_max_buffer_size(usize::MAX);

        let mut yamux_config = yamux::YamuxConfig::default();
        yamux_config.set_max_buffer_size(16 * 1024 * 1024);
        yamux_config.set_receive_window_size(16 * 1024 * 1024);
        // yamux_config.set_window_update_mode(WindowUpdateMode::OnRead);
        core::upgrade::SelectUpgrade::new(yamux_config, mplex_config)
    };

    transport
        .upgrade(core::upgrade::Version::V1)
        .authenticate(auth_config)
        .multiplex(mplex_config)
        .timeout(Duration::from_secs(20))
        .boxed()
}

/// Fetch keypair from disk, returning none if it cannot be decoded.
pub fn get_keypair(path: &str) -> Option<Keypair> {
    let result: Result<Vec<u8>, _> = read_file_to_vec(&path);
    match result {
        Err(e) => {
            info!("Networking keystore not found!");
            trace!("Error {:?}", e);
            None
        }
        Ok(mut vec) => match ed25519::Keypair::decode(&mut vec) {
            Ok(kp) => {
                info!("Recovered libp2p keypair from {:?}", &path);
                Some(Keypair::Ed25519(kp))
            }
            Err(e) => {
                info!("Could not decode networking keystore!");
                trace!("Error {:?}", e);
                None
            }
        },
    }
}
