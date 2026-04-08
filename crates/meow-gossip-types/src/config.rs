use std::time::Duration;

use crate::multiaddr::Multiaddr;

/// Configuration for the gossip network node.
#[derive(Clone)]
pub struct GossipNetworkConfig {
    /// Address to listen on (e.g. `/ip4/0.0.0.0/tcp/0`).
    pub listen_address: Multiaddr,
    /// Known peers to connect to on startup.
    pub bootstrap_peers: Vec<Multiaddr>,
    /// How often mDNS re-broadcasts discovery queries.
    /// Shorter values mean faster peer discovery at the cost of more multicast traffic.
    pub mdns_query_interval: Duration,
}

impl GossipNetworkConfig {
    /// Creates a new `GossipNetworkConfig` with the given parameters.
    pub fn new(
        listen_address: Multiaddr,
        bootstrap_peers: Vec<Multiaddr>,
        mdns_query_interval: Duration,
    ) -> Self {
        Self {
            listen_address,
            bootstrap_peers,
            mdns_query_interval,
        }
    }
}
