use hex_literal::hex;
use std::{time, thread};
use web3::{
    contract::{Contract, Options},
    futures::StreamExt,
    types::{FilterBuilder,H256,Address},
    transports::Http,
};
use crypto::sha3::Sha3;
use std::convert::TryInto;
use hex as hexa;
use serde_json;
use std::fs::File;
use std::io::{BufWriter, Write};
use secp256k1::SecretKey;
use crypto::digest::Digest;


#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConfigEvent{
    chain_id: usize,
    account: Address,
    time_interval: u64,
    events: [Option<&'static str>;4],
    stream: bool,
    url: &'static str
}

impl ConfigEvent{
    pub fn new(chain_id: usize,
               account: Address,
               time_interval: u64,
               events: [Option<&'static str>;4],
               stream: bool,
               url: &'static str)
               -> Self{
        Self{
            chain_id,
            account,
            time_interval,
            events,
            stream,
            url
        }
    }
}

#[derive(Debug, Clone)]
pub struct Deployer{
    chain_id: usize,
    account: Address,
    function: &'static str,
    contract: Contract<Http>,
    signing_key: SecretKey,
    url: &'static str
}

impl Deployer {
    pub fn new(chain_id: usize,
               account: Address,
               function: &'static str,
               contract: Contract<Http>,
               signing_key: SecretKey,
               url: &'static str)
               -> Self {
        Self {
            chain_id,
            account,
            function,
            contract,
            signing_key,
            url
        }
    }


    pub async fn call_function_in_smart_contract(deploy: Deployer) -> web3::contract::Result<()> {
        if deploy.chain_id != 1
        {
            return Ok(())
        }
        let secs = time::Duration::from_secs(3);

        thread::sleep(secs);
        let tx = deploy.contract.signed_call_with_confirmations(deploy.function, (), Options::default(), 1, &deploy.signing_key).await?;
        println!("got tx: {:?}", tx);
        Ok(())
    }
}

impl ConfigEvent {
    pub async fn event_listener(config: ConfigEvent, to_file: File) -> web3::contract::Result<()> {
        let web3 = web3::Web3::new(web3::transports::Http::new(config.url)?);
        let mut buf = BufWriter::new(to_file);
        let filter = FilterBuilder::default()
            .address(vec![config.account])
            .topics(
                topic_converter(config.events[0]),
                topic_converter(config.events[1]),
                topic_converter(config.events[2]),
                topic_converter(config.events[3])
            )
            .build();
        if config.stream {
            let filter = web3.eth_filter().create_logs_filter(filter).await?;

            let logs_stream = filter.stream(time::Duration::from_secs(1));

            futures::pin_mut!(logs_stream);
            let log_result = dbg!(tokio::time::timeout(time::Duration::from_secs(3), logs_stream.next()).await);
            if let Ok(log) = log_result {
                let log_json = serde_json::to_string(&log.unwrap().unwrap()).unwrap();
                buf.write_all(&log_json.clone().into_bytes()).unwrap();
                buf.flush().unwrap();
                let logs = logs_stream.collect::<Vec<_>>().await;
                println!("got logs: {:?}", logs);
            }
        } else {
            let filter = futures::executor::block_on(web3.eth_filter().create_logs_filter(filter)).unwrap();
            futures::executor::block_on(filter.poll())?;
        }
        Ok(())
    }
}

pub fn topic_converter(topic_str: Option<&str>) -> Option<Vec<H256>> {
    if topic_str.is_none() {
        return None
    }
    let mut hasher = Sha3::keccak256();
    hasher.input_str(topic_str.unwrap());
    let  hasher_bytes = hasher.result_str();
    //  let hasher_slice = hasher_bytes.as_slice();
    let decoded = hexa::decode(hasher_bytes).expect("Decoding failed");
    let check:  [u8; 32]= decoded.try_into().expect("slice with incorrect length");
    Some(vec![check.into()])
}

