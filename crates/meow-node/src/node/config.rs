//! Configuration for starting a MEOW node.

use std::net::SocketAddr;

use meow_gossip_types::config::GossipNetworkConfig;

/// Configuration for starting a MEOW node.
#[derive(Clone)]
pub struct NodeConfig {
    /// The address to bind the RPC server to (e.g., "127.0.0.1:8080").
    pub rpc_listen: SocketAddr,
    /// The gossip network configuration for the node.
    pub gossip_network_config: GossipNetworkConfig,
}

impl NodeConfig {
    /// Creates a new `NodeConfig` with the given parameters.
    pub fn new(rpc_listen: SocketAddr, gossip_network_config: GossipNetworkConfig) -> Self {
        Self {
            rpc_listen,
            gossip_network_config,
        }
    }
}
