// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::{
    discovery::DiscoveryOut,
    rpc::RequestResponseError,
};
use crate::{config::Libp2pConfig, discovery::DiscoveryBehaviour};
use crate::{
    discovery::DiscoveryConfig,
    hello::{HelloCodec, HelloProtocolName, HelloRequest, HelloResponse},
};

use futures::channel::oneshot::{self, Sender as OneShotSender};
use futures::{prelude::*, stream::FuturesUnordered};
use git_version::git_version;
use libp2p::core::PeerId;
use libp2p::gossipsub::{
    error::PublishError, error::SubscriptionError, Gossipsub, GossipsubConfigBuilder,
    GossipsubEvent, IdentTopic as Topic, MessageAuthenticity, MessageId, TopicHash, ValidationMode,
};
use libp2p::identify::{Identify, IdentifyEvent};
use libp2p::ping::{
    handler::{PingFailure, PingSuccess},
    Ping, PingEvent,
};
use libp2p::request_response::{
    ProtocolSupport, RequestId, RequestResponse, RequestResponseConfig, RequestResponseEvent,
    RequestResponseMessage, ResponseChannel,
};
use libp2p::swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters};
use libp2p::NetworkBehaviour;
use libp2p::{core::identity::Keypair, kad::QueryId};
use log::{debug, trace, warn};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::error::Error;
use std::pin::Pin;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, convert::TryInto};
use std::{task::Context, task::Poll};

lazy_static! {
    static ref VERSION: &'static str = env!("CARGO_PKG_VERSION");
    static ref CURRENT_COMMIT: &'static str = git_version!();
}

/// Libp2p behaviour for the Forest node. This handles all sub protocols needed for a Filecoin node.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ForestBehaviourEvent", poll_method = "poll")]
pub(crate) struct ForestBehaviour {
    gossipsub: Gossipsub,
    discovery: DiscoveryBehaviour,
    ping: Ping,
    identify: Identify,
    // TODO would be nice to have this handled together and generic, to avoid duplicated polling
    // but is fine for now, since the protocols are handled slightly differently.
    hello: RequestResponse<HelloCodec>,
    #[behaviour(ignore)]
    events: Vec<ForestBehaviourEvent>,
    /// Keeps track of hello requests indexed by request ID to route response.
    #[behaviour(ignore)]
    hello_request_table:
        HashMap<RequestId, OneShotSender<Result<HelloResponse, RequestResponseError>>>,
}

/// Event type which is emitted from the [ForestBehaviour] into the libp2p service.
#[derive(Debug)]
pub(crate) enum ForestBehaviourEvent {
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    GossipMessage {
        source: PeerId,
        topic: TopicHash,
        message: Vec<u8>,
    },
    HelloRequest {
        peer: PeerId,
        request: HelloRequest,
    },
}

impl NetworkBehaviourEventProcess<DiscoveryOut> for ForestBehaviour {
    fn inject_event(&mut self, event: DiscoveryOut) {
        match event {
            DiscoveryOut::Connected(peer) => {
                self.events.push(ForestBehaviourEvent::PeerConnected(peer));
            }
            DiscoveryOut::Disconnected(peer) => {
                self.events
                    .push(ForestBehaviourEvent::PeerDisconnected(peer));
            }
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for ForestBehaviour {
    fn inject_event(&mut self, message: GossipsubEvent) {
        if let GossipsubEvent::Message {
            propagation_source,
            message,
            message_id: _,
        } = message
        {
            self.events.push(ForestBehaviourEvent::GossipMessage {
                source: propagation_source,
                topic: message.topic,
                message: message.data,
            })
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for ForestBehaviour {
    fn inject_event(&mut self, event: PingEvent) {
        match event.result {
            Result::Ok(PingSuccess::Ping { rtt }) => {
                trace!(
                    "PingSuccess::Ping rtt to {} is {} ms",
                    event.peer.to_base58(),
                    rtt.as_millis()
                );
            }
            Result::Ok(PingSuccess::Pong) => {
                trace!("PingSuccess::Pong from {}", event.peer.to_base58());
            }
            Result::Err(PingFailure::Timeout) => {
                debug!("PingFailure::Timeout {}", event.peer.to_base58());
            }
            Result::Err(PingFailure::Other { error }) => {
                debug!("PingFailure::Other {}: {}", event.peer.to_base58(), error);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for ForestBehaviour {
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received {
                peer_id,
                info,
                observed_addr,
            } => {
                trace!("Identified Peer {}", peer_id);
                trace!("protocol_version {}", info.protocol_version);
                trace!("agent_version {}", info.agent_version);
                trace!("listening_ addresses {:?}", info.listen_addrs);
                trace!("observed_address {}", observed_addr);
                trace!("protocols {:?}", info.protocols);
            }
            IdentifyEvent::Sent { .. } => (),
            IdentifyEvent::Error { .. } => (),
        }
    }
}

impl NetworkBehaviourEventProcess<RequestResponseEvent<HelloRequest, HelloResponse>>
    for ForestBehaviour
{
    fn inject_event(&mut self, event: RequestResponseEvent<HelloRequest, HelloResponse>) {
        match event {
            RequestResponseEvent::Message { peer, message } => match message {
                RequestResponseMessage::Request {
                    request,
                    channel,
                    request_id: _,
                } => {
                    let arrival = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("System time before unix epoch")
                        .as_nanos()
                        .try_into()
                        .expect("System time since unix epoch should not exceed u64");

                    debug!("Received hello request: {:?}", request);
                    let sent = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("System time before unix epoch")
                        .as_nanos()
                        .try_into()
                        .expect("System time since unix epoch should not exceed u64");

                    // Send hello response immediately, no need to have the overhead of emitting
                    // channel and polling future here.
                    if let Err(e) = self
                        .hello
                        .send_response(channel, HelloResponse { arrival, sent })
                    {
                        debug!("Failed to send HelloResponse: {:?}", e)
                    };
                    self.events
                        .push(ForestBehaviourEvent::HelloRequest { request, peer });
                }
                RequestResponseMessage::Response {
                    request_id,
                    response,
                } => {
                    // Send the sucessful response through channel out.
                    let tx = self.hello_request_table.remove(&request_id);
                    if let Some(tx) = tx {
                        if tx.send(Ok(response)).is_err() {
                            debug!("RPCResponse receive timed out");
                        }
                    } else {
                        debug!("RPCResponse receive failed: channel not found");
                    };
                }
            },
            RequestResponseEvent::OutboundFailure {
                peer,
                request_id,
                error,
            } => {
                debug!(
                    "Hello outbound error (peer: {:?}) (id: {:?}): {:?}",
                    peer, request_id, error
                );

                // Send error through channel out.
                let tx = self.hello_request_table.remove(&request_id);
                if let Some(tx) = tx {
                    if tx.send(Err(error.into())).is_err() {
                        debug!("RPCResponse receive failed");
                    }
                }
            }
            RequestResponseEvent::InboundFailure {
                peer,
                error,
                request_id: _,
            } => {
                debug!("Hello inbound error (peer: {:?}): {:?}", peer, error);
            }
            RequestResponseEvent::ResponseSent { .. } => (),
        }
    }
}

impl ForestBehaviour {
    pub fn new(local_key: &Keypair, config: &Libp2pConfig, network_name: &str) -> Self {
        let mut gs_config_builder = GossipsubConfigBuilder::default();
        gs_config_builder.max_transmit_size(1 << 20);
        gs_config_builder.validation_mode(ValidationMode::Strict);

        let gossipsub_config = gs_config_builder.build().unwrap();
        let gossipsub = Gossipsub::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .unwrap();

        // TODO: Figure out why this delays incoming blocks. See gossip_params.rs for the params settings.
        // gossipsub.with_peer_score(build_peer_score_params(network_name), build_peer_score_threshold()).unwrap();

        let mut discovery_config = DiscoveryConfig::new(local_key.public(), network_name);
        discovery_config
            .with_mdns(config.mdns)
            .with_kademlia(config.kademlia)
            .with_user_defined(config.bootstrap_peers.clone())
            // TODO allow configuring this through config.
            .discovery_limit(config.target_peer_count as u64);

        let hp = std::iter::once((HelloProtocolName, ProtocolSupport::Full));

        let mut req_res_config = RequestResponseConfig::default();
        req_res_config.set_request_timeout(Duration::from_secs(20));
        req_res_config.set_connection_keep_alive(Duration::from_secs(20));

        ForestBehaviour {
            gossipsub,
            discovery: discovery_config.finish(),
            ping: Ping::default(),
            identify: Identify::new(
                "ipfs/0.1.0".into(),
                format!("forest-{}-{}", *VERSION, *CURRENT_COMMIT),
                local_key.public(),
            ),
            hello: RequestResponse::new(HelloCodec::default(), hp, req_res_config.clone()),
            hello_request_table: Default::default(),
            events: vec![],
        }
    }

    /// Bootstrap Kademlia network
    pub fn bootstrap(&mut self) -> Result<QueryId, String> {
        self.discovery.bootstrap()
    }

    /// Publish data over the gossip network.
    pub fn publish(
        &mut self,
        topic: Topic,
        data: impl Into<Vec<u8>>,
    ) -> Result<MessageId, PublishError> {
        self.gossipsub.publish(topic, data)
    }

    /// Subscribe to a gossip topic.
    pub fn subscribe(&mut self, topic: &Topic) -> Result<bool, SubscriptionError> {
        self.gossipsub.subscribe(topic)
    }

    /// Send a hello request or response to some peer.
    pub fn send_hello_request(
        &mut self,
        peer_id: &PeerId,
        request: HelloRequest,
        response_channel: OneShotSender<Result<HelloResponse, RequestResponseError>>,
    ) {
        let req_id = self.hello.send_request(peer_id, request);
        self.hello_request_table.insert(req_id, response_channel);
    }

    /// Adds peer to the peer set.
    pub fn peers(&mut self) -> &HashSet<PeerId> {
        self.discovery.peers()
    }
}
