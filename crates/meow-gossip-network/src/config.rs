use libp2p::Multiaddr;

const DEFAULT_LISTEN_ADDR: &str = "/ip4/0.0.0.0/tcp/0";

/// Configuration for the gossip network node.
pub struct NetworkConfig {
    /// Address to listen on (e.g. `/ip4/0.0.0.0/tcp/0`).
    pub listen_addr: Multiaddr,
    /// Known peers to connect to on startup.
    pub bootstrap_peers: Vec<Multiaddr>,
}

impl NetworkConfig {
    /// Creates a new `NetworkConfig` with the given parameters.
    pub fn new(listen_addr: Multiaddr, bootstrap_peers: Vec<Multiaddr>) -> Self {
        Self {
            listen_addr,
            bootstrap_peers,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_LISTEN_ADDR
                .parse()
                .expect("default listen addr must be valid"),
            bootstrap_peers: vec![],
        }
    }
}
