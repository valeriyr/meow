use std::net::SocketAddr;

use clap::Parser;
use meow_gossip_network::config::NetworkConfig;

use crate::node::Node;

/// The main command line commands.
#[derive(Parser)]
#[command(rename_all = "kebab-case")]
pub enum Command {
    /// Run the node.
    Run {
        /// Address to bind the RPC server on.
        #[arg(long, default_value = "127.0.0.1:8600")]
        rpc: SocketAddr,

        /// Address for the gossip network to listen on.
        #[arg(long, default_value = "/ip4/0.0.0.0/tcp/0")]
        listen: String,

        /// Bootstrap peer multiaddresses to connect to on startup (repeatable).
        /// Example: /ip4/1.2.3.4/tcp/30333/p2p/<peer-id>
        #[arg(long)]
        bootstrap: Vec<String>,

        /// PoW difficulty: number of leading zero bits required in a block hash.
        /// 8 = ~256 hashes per block (~fast). 20 = ~1M hashes (~slow).
        #[arg(long, default_value_t = 8)]
        difficulty: u32,
    },
}

impl Command {
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Run {
                rpc,
                listen,
                bootstrap,
                difficulty,
            } => {
                let gossip_config = NetworkConfig {
                    listen_addr: listen.parse()?,
                    bootstrap_peers: bootstrap
                        .iter()
                        .map(|s| s.parse())
                        .collect::<Result<Vec<_>, _>>()?,
                };
                Node::new(rpc, gossip_config, difficulty).run().await?;
            }
        }
        Ok(())
    }
}
