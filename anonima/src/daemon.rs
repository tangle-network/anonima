// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::cli::{block_until_sigint, Config};
use async_std::{channel::bounded, sync::RwLock, task};
use libp2p::identity::{ed25519, Keypair};
use log::{debug, info, trace};
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use utils::write_to_file;
use wallet::ENCRYPTED_KEYSTORE_NAME;
use wallet::{KeyStore, KeyStoreConfig};

/// Starts daemon process
pub(super) async fn start(config: Config) {
    info!("Starting Anonima daemon");
    let net_keypair = get_keypair(&format!("{}{}", &config.data_dir, "/libp2p/keypair"))
        .unwrap_or_else(|| {
            // Keypair not found, generate and save generated keypair
            let gen_keypair = ed25519::Keypair::generate();
            // Save Ed25519 keypair to file
            // TODO rename old file to keypair.old(?)
            if let Err(e) = write_to_file(
                &gen_keypair.encode(),
                &format!("{}{}", &config.data_dir, "/libp2p/"),
                "keypair",
            ) {
                info!("Could not write keystore to disk!");
                trace!("Error {:?}", e);
            };
            Keypair::Ed25519(gen_keypair)
        });

    // Initialize keystore
    let mut ks = if config.encrypt_keystore {
        loop {
            print!("keystore passphrase: ");
            std::io::stdout().flush().unwrap();

            let passphrase = read_password().expect("Error reading passphrase");

            let mut data_dir = PathBuf::from(&config.data_dir);
            data_dir.push(ENCRYPTED_KEYSTORE_NAME);

            if !data_dir.exists() {
                print!("confirm passphrase: ");
                std::io::stdout().flush().unwrap();

                read_password().expect("Passphrases do not match");
            }

            let key_store_init_result = KeyStore::new(KeyStoreConfig::Encrypted(
                PathBuf::from(&config.data_dir),
                passphrase,
            ));

            match key_store_init_result {
                Ok(ks) => break ks,
                Err(_) => {
                    log::error!("incorrect passphrase")
                }
            };
        }
    } else {
        KeyStore::new(KeyStoreConfig::Persistent(PathBuf::from(&config.data_dir)))
            .expect("Error initializing keystore")
    };

    if ks.get(JWT_IDENTIFIER).is_err() {
        ks.put(JWT_IDENTIFIER.to_owned(), generate_priv_key())
            .unwrap();
    }
    let keystore = Arc::new(RwLock::new(ks));

    // Initialize database (RocksDb will be default if both features enabled)
    #[cfg(all(feature = "sled", not(feature = "rocksdb")))]
    let db = db::sled::SledDb::open(config.data_dir + "/sled").unwrap();

    #[cfg(feature = "rocksdb")]
    let db = db::rocks::RocksDb::open(config.data_dir + "/db").unwrap();

    let db = Arc::new(db);

    let keystore_write = task::spawn(async move {
        keystore.read().await.flush().unwrap();
    });

    // Cancel all async services
    sync_task.cancel().await;
    if let Some(task) = rpc_task {
        task.cancel().await;
    }
    keystore_write.await;

    info!("Anonima finish shutdown");
}
