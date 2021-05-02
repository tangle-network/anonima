// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use chain_sync::SyncConfig;
use network::Libp2pConfig;
use serde::Deserialize;
use utils::get_home_dir;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub network: Libp2pConfig,
    pub data_dir: String,
    pub enable_rpc: bool,
    pub rpc_port: String,
    /// Skips loading import CAR file and assumes it's already been loaded.
    /// Will use the cids in the header of the file to index the chain.
    pub skip_load: bool,
    pub encrypt_keystore: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: Libp2pConfig::default(),
            data_dir: get_home_dir() + "/.forest",
            enable_rpc: true,
            rpc_port: "1234".to_string(),
            sync: SyncConfig::default(),
            encrypt_keystore: false,
        }
    }
}
