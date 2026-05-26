//! In-process MEOW node wrapper for use in end-to-end tests.

use std::net::SocketAddr;

use meow_genesis::Genesis;
use meow_gossip_types::{config::GossipNetworkConfig, multiaddr::Multiaddr};

use meow_nakamoto_types::miner_config::MinerConfig;
use meow_node::node::{Node, config::NodeConfig};
use meow_node_client::NodeClient;
use meow_types::{
    address::Address,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
};
use rand::thread_rng;
use tokio::sync::oneshot;

/// Default RPC address for test nodes (port 0 means "pick a random free port").
const DEFAULT_RPC_ADDR: &str = "127.0.0.1:0";
// difficulty 0: instant mining
const DEFAULT_DIFFICULTY: u32 = 0;

/// A running MEOW node for use in tests.
pub struct TestNode {
    client: NodeClient,
    gossip_bootstrap_address: Multiaddr,
    task: tokio::task::JoinHandle<()>,
}

impl TestNode {
    /// Start a node with a minimal single-account genesis for tests that need a running node
    /// but don't interact with any specific on-chain state (e.g. querying unknown addresses).
    pub async fn start_minimal() -> Self {
        let keypair = KeyPair::random(SignatureScheme::Ed25519, thread_rng());
        let address = Address::from(&keypair);
        let genesis = Genesis::build(&[(address, 1)]).expect("throwaway genesis must build");

        Self::start_with_genesis(&genesis).await
    }

    /// Start a node pre-seeded with the given genesis.
    pub async fn start_with_genesis(genesis: &Genesis) -> Self {
        Self::start_with_bootstrap(genesis, vec![]).await
    }

    /// Start a node pre-seeded with the given genesis and an explicit miner configuration.
    pub async fn start_with_genesis_and_miner_config(
        genesis: &Genesis,
        miner_config: MinerConfig,
    ) -> Self {
        let (listen_addr, bootstrap_addr) = random_gossip_listen_addr();
        let gossip_config = GossipNetworkConfig::new_with_defaults(listen_addr, vec![]);
        let node_config = NodeConfig::new(DEFAULT_RPC_ADDR.parse().unwrap(), gossip_config);
        let node = Node::with_genesis(node_config, miner_config, genesis);

        Self::start(node, bootstrap_addr).await
    }

    /// Start a node pre-seeded with the given genesis and explicit bootstrap peers.
    pub async fn start_with_bootstrap(genesis: &Genesis, bootstrap_peers: Vec<Multiaddr>) -> Self {
        let (listen_addr, bootstrap_addr) = random_gossip_listen_addr();

        let gossip_config = GossipNetworkConfig::new_with_defaults(listen_addr, bootstrap_peers);
        let node_config = NodeConfig::new(DEFAULT_RPC_ADDR.parse().unwrap(), gossip_config);
        let miner_keypair = test_miner_keypair();
        let miner_reward_address = Address::from(&miner_keypair);
        let miner_config =
            MinerConfig::new(DEFAULT_DIFFICULTY, miner_keypair, miner_reward_address);
        let node = Node::with_genesis(node_config, miner_config, genesis);

        Self::start(node, bootstrap_addr).await
    }

    /// Start the node and return a handle to it.
    /// The node will be automatically stopped when the handle is dropped.
    async fn start(node: Node, gossip_bootstrap_address: Multiaddr) -> Self {
        let (tcp_listener_ready_tx, tcp_listener_ready_rx) = oneshot::channel::<SocketAddr>();

        let task = tokio::spawn(async move {
            node.run_notifying(tcp_listener_ready_tx).await.ok();
        });
        let addr = tcp_listener_ready_rx
            .await
            .expect("node must send bound addr");

        let client = NodeClient::with_address(addr);

        Self {
            client,
            gossip_bootstrap_address,
            task,
        }
    }

    /// Returns a reference to the node's RPC client.
    pub fn client(&self) -> &NodeClient {
        &self.client
    }

    /// Returns the node's gossip address, suitable for use as a bootstrap peer by other nodes.
    pub fn gossip_bootstrap_address(&self) -> &Multiaddr {
        &self.gossip_bootstrap_address
    }
}

impl Drop for TestNode {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Returns `(listen_addr, bootstrap_addr)` for a fresh test gossip endpoint.
/// - `listen_addr` binds to `0.0.0.0` so gossipsub uses the real interface.
/// - `bootstrap_addr` uses `127.0.0.1` and is safe to pass to other test nodes as a dial target.
fn random_gossip_listen_addr() -> (Multiaddr, Multiaddr) {
    let listener = std::net::TcpListener::bind(DEFAULT_RPC_ADDR)
        .expect("must bind an ephemeral gossip port for tests");
    let port = listener
        .local_addr()
        .expect("ephemeral listener must have local address")
        .port();
    drop(listener);

    let listen_addr = format!("/ip4/0.0.0.0/tcp/{port}")
        .parse()
        .expect("generated gossip listen address must be valid");
    let bootstrap_addr = format!("/ip4/127.0.0.1/tcp/{port}")
        .parse()
        .expect("generated gossip bootstrap address must be valid");

    (listen_addr, bootstrap_addr)
}

/// Returns a fresh miner keypair for a test node.
/// Each call produces a distinct key so nodes in a multi-node test have unique miner addresses.
fn test_miner_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, thread_rng())
}
