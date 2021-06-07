// Copyright 2020 ChainSafe Systems
// Copyright 2021 Webb Technologies
// SPDX-License-Identifier: Apache-2.0, MIT

mod config;

pub use self::config::Config;

use anonima_libp2p::utils::{read_file_to_string, read_toml};
use jsonrpc_v2::Error as JsonRpcError;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{io, process};
use structopt::StructOpt;

/// CLI structure generated when interacting with Forest binary
#[derive(StructOpt)]
#[structopt(
    name = "anonima",
    version = "0.0.1",
    about = "Anonima implementation in Rust. This command will start the daemon process",
    author = "Webb Technologies <drew@webb.to>"
)]
pub struct CLI {
    #[structopt(flatten)]
    pub daemon_opts: DaemonOpts,
}

/// Daemon process command line options.
#[derive(StructOpt, Debug)]
pub struct DaemonOpts {
    #[structopt(short, long, help = "A toml file containing relevant configurations")]
    pub config: Option<String>,
    #[structopt(short, long, help = "The genesis CAR file")]
    pub genesis: Option<String>,
    #[structopt(short, long, help = "Allow rpc to be active or not (default = true)")]
    pub rpc: Option<bool>,
    #[structopt(short, long, help = "The port used for communication")]
    pub port: Option<String>,
    #[structopt(short, long, help = "Allow Kademlia (default = true)")]
    pub kademlia: Option<bool>,
    #[structopt(short, long, help = "Allow MDNS (default = true)")]
    pub mdns: Option<bool>,
    #[structopt(long, help = "Number of worker sync tasks spawned (default is 1")]
    pub worker_tasks: Option<usize>,
    #[structopt(
        long,
        help = "Number of tipsets requested over chain exchange (default is 200)"
    )]
    pub req_window: Option<i64>,
    #[structopt(
        long,
        help = "Amount of Peers we want to be connected to (default is 75)"
    )]
    pub target_peer_count: Option<u32>,
    #[structopt(
        long,
        help = "Use a temporary directory instead of data dir provided from config file.
        useful for local testing"
    )]
    pub tmp: bool,
    #[structopt(long, help = "Enable development logging (default is false)")]
    pub dev: bool,
}

impl DaemonOpts {
    pub fn to_config(&self) -> Result<Config, io::Error> {
        let logger_env = if self.dev {
            super::logger::LogEnv::Develoment
        } else {
            super::logger::LogEnv::Production
        };
        super::logger::setup_logger(logger_env);
        let mut cfg: Config = match &self.config {
            Some(config_file) => {
                // Read from config file
                let toml = read_file_to_string(&*config_file)?;
                // Parse and return the configuration file
                read_toml(&toml)?
            }
            None => Config::default(),
        };

        if self.tmp {
            cfg = Config::tmp();
        }

        if self.rpc.unwrap_or(cfg.enable_rpc) {
            cfg.enable_rpc = true;
            cfg.rpc_port = self.port.to_owned().unwrap_or(cfg.rpc_port);
        } else {
            cfg.enable_rpc = false;
        }

        cfg.network.kademlia = self.kademlia.unwrap_or(cfg.network.kademlia);
        cfg.network.mdns = self.mdns.unwrap_or(cfg.network.mdns);
        if let Some(target_peer_count) = self.target_peer_count {
            cfg.network.target_peer_count = target_peer_count;
        }

        Ok(cfg)
    }
}

/// Blocks current thread until ctrl-c is received
pub(super) async fn block_until_sigint() {
    let (ctrlc_send, ctrlc_oneshot) = futures::channel::oneshot::channel();
    let ctrlc_send_c = RefCell::new(Some(ctrlc_send));

    let running = Arc::new(AtomicUsize::new(0));
    ctrlc::set_handler(move || {
        let prev = running.fetch_add(1, Ordering::SeqCst);
        if prev == 0 {
            println!("Got interrupt, shutting down...");
            // Send sig int in channel to blocking task
            if let Some(ctrlc_send) = ctrlc_send_c.try_borrow_mut().unwrap().take() {
                ctrlc_send.send(()).expect("Error sending ctrl-c message");
            }
        } else {
            process::exit(0);
        }
    })
    .expect("Error setting Ctrl-C handler");

    ctrlc_oneshot.await.unwrap();
}

/// Returns a stringified JSON-RPC error
pub(super) fn stringify_rpc_err(e: JsonRpcError) -> String {
    match e {
        JsonRpcError::Full {
            code,
            message,
            data: _,
        } => {
            return format!("JSON RPC Error: Code: {} Message: {}", code, message);
        }
        JsonRpcError::Provided { code, message } => {
            return format!("JSON RPC Error: Code: {} Message: {}", code, message);
        }
    }
}
