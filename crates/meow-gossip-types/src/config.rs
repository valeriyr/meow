use crate::multiaddr::Multiaddr;

const DEFAULT_LISTEN_ADDR: &str = "/ip4/0.0.0.0/tcp/0";

/// Configuration for the gossip network node.
pub struct GossipNetworkConfig {
    /// Address to listen on (e.g. `/ip4/0.0.0.0/tcp/0`).
    pub listen_address: Multiaddr,
    /// Known peers to connect to on startup.
    pub bootstrap_peers: Vec<Multiaddr>,
}

impl GossipNetworkConfig {
    /// Creates a new `GossipNetworkConfig` with the given parameters.
    pub fn new(listen_address: Multiaddr, bootstrap_peers: Vec<Multiaddr>) -> Self {
        Self {
            listen_address,
            bootstrap_peers,
        }
    }
}

impl Default for GossipNetworkConfig {
    fn default() -> Self {
        Self {
            listen_address: DEFAULT_LISTEN_ADDR
                .parse()
                .expect("default listen addr must be valid"),
            bootstrap_peers: vec![],
        }
    }
}
