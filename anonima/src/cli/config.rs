// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anonima_libp2p::utils::get_home_dir;
use anonima_libp2p::Libp2pConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub network: Libp2pConfig,
    pub data_dir: String,
    pub enable_rpc: bool,
    pub rpc_port: String,
    pub encrypt_keystore: bool,
    #[serde(skip)]
    pub tmp_dir: Option<temp_dir::TempDir>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: Libp2pConfig::default(),
            data_dir: get_home_dir() + "/.forest",
            enable_rpc: true,
            rpc_port: "1234".to_string(),
            encrypt_keystore: false,
            tmp_dir: None,
        }
    }
}

impl Config {
    pub fn tmp() -> Self {
        let mut cfg = Self::default();
        let tmp_dir = temp_dir::TempDir::new().expect("failed to create temp dir");
        Self {
            data_dir: tmp_dir.path().display().to_string(),
            tmp_dir: Some(tmp_dir),
            ..Default::default()
        }
    }
}
