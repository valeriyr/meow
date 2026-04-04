use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use meow_genesis::Genesis;
use meow_gossip_types::{config::GossipNetworkConfig, multiaddr::Multiaddr};

use crate::node::Node;

/// The default node RPC URL.
pub const DEFAULT_NODE_URL: &str = "127.0.0.1:8600";
/// The default gossip listen address.
pub const DEFAULT_GOSSIP_LISTEN_ADDRESS: &str = "/ip4/0.0.0.0/tcp/0";

/// The main command line commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case", verbatim_doc_comment)]
pub enum Command {
    /// Run the node.
    Run {
        /// RPC listen address for this node.
        /// Example: 127.0.0.1:8600
        #[arg(long, default_value = DEFAULT_NODE_URL, verbatim_doc_comment)]
        rpc_listen: SocketAddr,
        /// Gossip listen multiaddr.
        /// Example: /ip4/0.0.0.0/tcp/30333
        #[arg(long, default_value = DEFAULT_GOSSIP_LISTEN_ADDRESS, verbatim_doc_comment)]
        listen_address: Multiaddr,
        /// Bootstrap peer addresses to connect to on startup (repeatable).
        /// Example: /ip4/1.2.3.4/tcp/30333/p2p/<peer-id>
        #[arg(long, verbatim_doc_comment)]
        bootstrap_peers: Vec<Multiaddr>,
        /// Path to a BCS-serialized Genesis file.
        /// If omitted, the node starts with an empty state.
        #[arg(long, verbatim_doc_comment)]
        genesis: Option<PathBuf>,
        /// PoW difficulty: number of leading zero bits required in a block hash.
        /// Dev guidance: 8 = ~256 hashes per block (fast), 20 = ~1M hashes (slow).
        #[arg(
            long,
            default_value_t = 8,
            value_parser = clap::value_parser!(u32).range(1..=32),
            verbatim_doc_comment
        )]
        difficulty: u32,
    },
}

impl Command {
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Run {
                rpc_listen,
                listen_address,
                bootstrap_peers,
                genesis,
                difficulty,
            } => {
                let gossip_network_config = GossipNetworkConfig {
                    listen_address,
                    bootstrap_peers,
                };

                let node = if let Some(genesis_path) = genesis {
                    let genesis_bytes = std::fs::read(&genesis_path)?;
                    let genesis = bcs::from_bytes::<Genesis>(&genesis_bytes)?;

                    Node::with_genesis(rpc_listen, gossip_network_config, difficulty, &genesis)
                } else {
                    Node::empty(rpc_listen, gossip_network_config, difficulty)
                };

                node.run().await?;
            }
        }
        Ok(())
    }
}
