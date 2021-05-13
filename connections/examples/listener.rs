use hex_literal::hex;
use web3::{contract::{Contract, Options}};
use std::fs::File;
use secp256k1::SecretKey;
use connections::ethereum::event_listener::{ConfigEvent,Deployer};




#[tokio::main]
async fn main() -> web3::contract::Result<()> {
    let url = "http://127.0.0.1:7545";

    let web3 = web3::Web3::new(web3::transports::Http::new(url)?);
    let accounts = web3.eth().accounts().await?;
    let contract = Contract::deploy(web3.eth(), include_bytes!("res/SimpleEvent.abi"))?
        .confirmations(0)
        .options(Options::with(|opt| opt.gas = Some(3_000_000.into())))
        .execute(include_str!("res/SimpleEvent.bin"), (), accounts[0])
        .await?;

    println!("contract deployed at: {}", contract.address());
    let private_key = SecretKey::from_slice(&hex!(
            "4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
        ))
        .unwrap();
    let deployer = Deployer::new(1,
                                 accounts[0],
                                 "hello",
                                 contract.clone(),
                                 //H160::from_low_u64_be(123)
                                 private_key    ,
                                 url
    );

    let config = ConfigEvent::new(1,
                                  contract.address(),
                                  10,
                                  [None,None,None,None],
                                  false,
                                  url);
    let json_file = File::create("events.json").expect("cannot create event file");

    let future_deployer = Deployer::call_function_in_smart_contract(deployer);
    let future_listener = ConfigEvent::event_listener(config,json_file);
    futures::try_join!(future_listener,future_deployer);
    Ok(())
}
