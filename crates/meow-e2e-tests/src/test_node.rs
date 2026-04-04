use std::net::SocketAddr;

use meow_genesis::Genesis;
use meow_gossip_types::config::GossipNetworkConfig;

use meow_node::node::Node;
use meow_node_client::NodeClient;
use tokio::sync::oneshot;

/// Default RPC address for test nodes (port 0 means "pick a random free port").
const DEFAULT_RPC_ADDR: &str = "127.0.0.1:0";
// difficulty 0: instant mining
const DEFAULT_DIFFICULTY: u32 = 0;

/// A running MEOW node for use in tests.
pub struct TestNode {
    client: NodeClient,
    task: tokio::task::JoinHandle<()>,
}

impl TestNode {
    /// Start a node with an empty store (no genesis objects).
    pub async fn start_empty() -> Self {
        Self::start(Node::empty(
            DEFAULT_RPC_ADDR.parse().unwrap(),
            GossipNetworkConfig::default(),
            DEFAULT_DIFFICULTY,
        ))
        .await
    }

    /// Start a node pre-seeded with the given genesis.
    pub async fn start_with_genesis(genesis: &Genesis) -> Self {
        Self::start(Node::with_genesis(
            DEFAULT_RPC_ADDR.parse().unwrap(),
            GossipNetworkConfig::default(),
            DEFAULT_DIFFICULTY,
            genesis,
        ))
        .await
    }

    /// Start the node and return a handle to it.
    /// The node will be automatically stopped when the handle is dropped.
    async fn start(node: Node) -> Self {
        let (tcp_listener_ready_tx, tcp_listener_ready_rx) = oneshot::channel::<SocketAddr>();

        let task = tokio::spawn(async move {
            node.run_notifying(tcp_listener_ready_tx).await.ok();
        });
        let addr = tcp_listener_ready_rx
            .await
            .expect("node must send bound addr");

        let client = NodeClient::with_address(addr);

        Self { client, task }
    }

    /// Returns a reference to the node's RPC client.
    pub fn client(&self) -> &NodeClient {
        &self.client
    }
}

impl Drop for TestNode {
    fn drop(&mut self) {
        self.task.abort();
    }
}
