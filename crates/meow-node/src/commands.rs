//! CLI command definitions and dispatch logic for the MEOW node binary.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use clap::Parser;
use meow_genesis::Genesis;
use meow_gossip_types::{
    config::{
        DEFAULT_CHECK_EXPLICIT_PEERS_TICKS, DEFAULT_MDNS_QUERY_INTERVAL_SECS, GossipNetworkConfig,
    },
    multiaddr::Multiaddr,
};
use meow_nakamoto_types::miner_config::MinerConfig;
use meow_types::{
    address::Address,
    config,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    keystore::Keystore,
};
use rand::thread_rng;

use crate::node::{Node, config::NodeConfig};

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
        /// mDNS re-query interval in seconds.
        /// Controls how often the node re-broadcasts discovery queries when no peers are found.
        #[arg(long, default_value_t = DEFAULT_MDNS_QUERY_INTERVAL_SECS, verbatim_doc_comment)]
        mdns_query_interval: u64,
        /// The number of heartbeat ticks until the connection to explicit peers are rechecked
        /// and reconnected if necessary.
        #[arg(long, default_value_t = DEFAULT_CHECK_EXPLICIT_PEERS_TICKS, verbatim_doc_comment)]
        check_explicit_peers_ticks: u64,
        /// Path to a BCS-serialized Genesis file.
        #[arg(long, verbatim_doc_comment)]
        genesis: PathBuf,
        /// PoW difficulty: number of leading zero bits required in a block hash.
        /// Dev guidance: 8 = ~256 hashes per block (fast), 20 = ~1M hashes (slow).
        #[arg(
            long,
            default_value_t = 8,
            value_parser = clap::value_parser!(u32).range(1..=32),
            verbatim_doc_comment
        )]
        difficulty: u32,
        /// Address of the miner key to load from the keystore used to sign system transactions.
        /// If omitted, an ephemeral random keypair is used and rewards are lost on restart.
        #[arg(long, verbatim_doc_comment)]
        miner_address: Option<Address>,
        /// Address that receives the minted block reward coins.
        /// If omitted, defaults to the miner's own address.
        /// Use this to direct earnings to a cold wallet or a separate account.
        #[arg(long, verbatim_doc_comment)]
        miner_reward_address: Option<Address>,
        /// Path to the keystore file used to sign system transactions.
        /// If omitted, the default keystore path is used. Requires --miner-address.
        #[arg(long, requires = "miner_address", verbatim_doc_comment)]
        keystore_path: Option<PathBuf>,
    },
}

impl Command {
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Command::Run {
                rpc_listen,
                listen_address,
                bootstrap_peers,
                mdns_query_interval,
                check_explicit_peers_ticks,
                genesis,
                difficulty,
                miner_address,
                miner_reward_address,
                keystore_path,
            } => {
                print_startup_banner();

                let miner_keypair = resolve_miner_keypair(keystore_path, miner_address)?;
                let miner_reward_address =
                    miner_reward_address.unwrap_or_else(|| Address::from(&miner_keypair));

                let gossip_network_config = GossipNetworkConfig::new(
                    listen_address,
                    bootstrap_peers,
                    Duration::from_secs(mdns_query_interval),
                    check_explicit_peers_ticks,
                );
                let node_config = NodeConfig::new(rpc_listen, gossip_network_config);
                let miner_config =
                    MinerConfig::new(difficulty, miner_keypair, miner_reward_address);

                let genesis_bytes = std::fs::read(&genesis)?;
                let genesis = bcs::from_bytes::<Genesis>(&genesis_bytes)?;
                let node = Node::with_genesis(node_config, miner_config, &genesis);

                node.run().await?;
            }
        }
        Ok(())
    }
}

/// Prints the ASCII art banner on node startup.
fn print_startup_banner() {
    println!(
        r#"
 __  __ _____  ___  _       __   _   _  ___  ____  _____
|  \/  | ____|/ _ \| |     / /  | \ | |/ _ \|  _ \| ____|
| |\/| |  _| | | | | | /| / /   |  \| | | | | | | |  _|
| |  | | |___| |_| | |/ |/ /    | |\  | |_| | |_| | |___
|_|  |_|_____|\___/|__/|__/     |_| \_|\___/|____/|_____|

version {}
"#,
        env!("CARGO_PKG_VERSION")
    );
}

/// Resolve the miner keypair from the keystore or generate an ephemeral fallback.
///
/// - `miner_address` given: load that key from the keystore at `keystore_path`
///   (or the default path if `keystore_path` is `None`).
/// - If `miner_address` is not given: generate a random keypair and warn. The resulting miner
///   address is not stable across restarts and all block rewards will be lost.
fn resolve_miner_keypair(
    keystore_path: Option<PathBuf>,
    miner_address: Option<Address>,
) -> anyhow::Result<KeyPair> {
    if let Some(address) = miner_address {
        let keystore_path = keystore_path.unwrap_or(config::meow_keystore_path()?);
        let keystore = Keystore::file_based(&keystore_path)?;

        let keypair = keystore.get_key(&address).ok_or_else(|| {
            anyhow::anyhow!("address {address} not found in keystore {keystore_path:?}")
        })?;

        return KeyPair::from_bytes(&keypair.to_bytes()).map_err(Into::into);
    }

    tracing::warn!(
        "no miner address provided — generating a random miner keypair; \
         block rewards will be lost on restart"
    );

    Ok(KeyPair::random(SignatureScheme::Ed25519, thread_rng()))
}
