struct Config {
    chains: Vec<RawChainConfig>,
    keystore_path: Vec<u8>,
}

struct RawChainConfig {
    // Name of chain
    name: Vec<u8>,
    // Type of chain (EVM, Substrate, Cosmos, etc.)
    chain_type: Vec<u8>,
    // Chain ID
    id: Vec<u8>,
    // Url for rpc endpoint
    endpoint: Vec<u8>,
    // Address of key to use
    from: Vec<u8>,
}
