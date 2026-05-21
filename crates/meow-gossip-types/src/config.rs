//! Configuration for the gossip network node.

use std::time::Duration;

use crate::multiaddr::Multiaddr;

/// The default mDNS query interval in seconds.
pub const DEFAULT_MDNS_QUERY_INTERVAL_SECS: u64 = 300; // 5 minutes
/// The default number of heartbeat ticks until the connection to explicit peers are rechecked
/// and reconnected if necessary.
pub const DEFAULT_CHECK_EXPLICIT_PEERS_TICKS: u64 = 300;

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
    /// The number of heartbeat ticks until the connection to explicit peers are rechecked
    /// and reconnected if necessary.
    pub check_explicit_peers_ticks: u64,
}

impl GossipNetworkConfig {
    /// Creates a new `GossipNetworkConfig` with the given parameters.
    pub fn new(
        listen_address: Multiaddr,
        bootstrap_peers: Vec<Multiaddr>,
        mdns_query_interval: Duration,
        check_explicit_peers_ticks: u64,
    ) -> Self {
        Self {
            listen_address,
            bootstrap_peers,
            mdns_query_interval,
            check_explicit_peers_ticks,
        }
    }

    /// Creates a new `GossipNetworkConfig` with the given listen address and bootstrap peers,
    /// and default values for the other parameters.
    pub fn new_with_defaults(listen_address: Multiaddr, bootstrap_peers: Vec<Multiaddr>) -> Self {
        Self::new(
            listen_address,
            bootstrap_peers,
            Duration::from_secs(DEFAULT_MDNS_QUERY_INTERVAL_SECS),
            DEFAULT_CHECK_EXPLICIT_PEERS_TICKS,
        )
    }
}
