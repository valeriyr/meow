//! Shared helpers and polling utilities for end-to-end tests.

use std::time::Duration;

use crate::test_node::TestNode;
use meow_genesis::Genesis;
use meow_nakamoto_types::miner_config::MinerConfig;
use meow_node_client::NodeClient;
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{Object, object_ref::ObjectRef, object_type::ObjectType},
    transaction::{SignedTransaction, Transaction, execution_result::ExecutionResult},
};
use meow_vm_adapter::builder;
use rand::{SeedableRng, rngs::StdRng};

/// The timeout for waiting for a result to be available.
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
/// The interval between polls when waiting for a result to be available.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// The wait after connecting bootstrap peers before asserting gossip state.
pub const GOSSIP_PEER_CONNECT_WAIT: Duration = Duration::from_millis(300);

// difficulty 0: instant mining.
const DEFAULT_DIFFICULTY: u32 = 0;
// batch_size 1: mine as soon as any transaction arrives.
const DEFAULT_BATCH_SIZE: usize = 1;
// snapshot_depth 64: retain 64 snapshots behind the head (max reorg depth).
const DEFAULT_SNAPSHOT_DEPTH: u64 = 64;

/// Returns a `MinerConfig` with test defaults.
pub fn test_miner_config(keypair: KeyPair, reward_address: Address) -> MinerConfig {
    MinerConfig::new(
        DEFAULT_DIFFICULTY,
        keypair,
        reward_address,
        DEFAULT_BATCH_SIZE,
        DEFAULT_SNAPSHOT_DEPTH,
    )
}

/// Returns a `MinerConfig` with a custom snapshot depth.
pub fn test_miner_config_with_snapshot_depth(
    keypair: KeyPair,
    reward_address: Address,
    snapshot_depth: u64,
) -> MinerConfig {
    MinerConfig::new(
        DEFAULT_DIFFICULTY,
        keypair,
        reward_address,
        DEFAULT_BATCH_SIZE,
        snapshot_depth,
    )
}

/// Returns a deterministic test keypair.
pub fn test_keypair() -> KeyPair {
    test_keypair_from_seed([42; 32])
}

/// Returns a test keypair generated from the given seed.
pub fn test_keypair_from_seed(seed: [u8; 32]) -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed(seed))
}

/// Find the genesis coin address for a given sender.
pub fn genesis_coin_addr(genesis: &Genesis, sender: Address) -> Address {
    *genesis
        .objects()
        .iter()
        .find(|o| o.owner().address() == Some(&sender))
        .expect("genesis must contain a coin for sender")
        .address()
}

/// Return all coin addresses owned by `sender` in genesis.
pub fn genesis_coin_addrs(genesis: &Genesis, sender: Address) -> Vec<Address> {
    genesis
        .objects()
        .iter()
        .filter(|o| o.owner().address() == Some(&sender))
        .map(|o| *o.address())
        .collect()
}

/// Extract the published module address from a successful execution result.
pub fn published_module_addr(result: &ExecutionResult) -> Address {
    *result
        .created_objects()
        .iter()
        .find(|o| o.type_() == &ObjectType::Module)
        .expect("module publish must create a module object")
        .address()
}

/// Build a deterministic single-account genesis and return the key test values.
pub fn single_account_genesis(balance: u64) -> (KeyPair, Address, Genesis, Address) {
    let keypair = test_keypair();
    let sender = Address::from(&keypair);
    let genesis = Genesis::build(&[(sender, balance)]).expect("genesis must build");
    let coin_addr = genesis_coin_addr(&genesis, sender);
    (keypair, sender, genesis, coin_addr)
}

/// Fetch a live object from the node or panic if it is missing.
pub async fn get_object(client: &NodeClient, address: &Address) -> Object {
    client
        .get_object(address)
        .await
        .expect("client call must not fail")
        .expect("object must be present in node")
}

/// Fetch a live object reference from the node.
pub async fn get_object_ref(client: &NodeClient, address: &Address) -> ObjectRef {
    get_object(client, address).await.object_ref()
}

/// Poll until the transaction result is available, or panic after 5s.
pub async fn wait_for_object(client: &NodeClient, address: &Address) -> Object {
    let deadline = tokio::time::Instant::now() + DEFAULT_WAIT_TIMEOUT;
    loop {
        if let Some(object) = client.get_object(address).await.unwrap() {
            return object;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for object {address} to be available"
        );
        tokio::time::sleep(DEFAULT_POLL_INTERVAL).await;
    }
}

/// Poll until at least one object owned by `address` is available, or panic after 5s.
pub async fn wait_for_objects_owned(client: &NodeClient, address: &Address) -> Vec<Object> {
    let deadline = tokio::time::Instant::now() + DEFAULT_WAIT_TIMEOUT;
    loop {
        let objects = client.get_objects_owned(address).await.unwrap();
        if !objects.is_empty() {
            return objects;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for objects owned by {address}"
        );
        tokio::time::sleep(DEFAULT_POLL_INTERVAL).await;
    }
}

/// Poll until the transaction result is available, or panic after 5s.
pub async fn wait_for_result(client: &NodeClient, digest: &Digest) -> ExecutionResult {
    let deadline = tokio::time::Instant::now() + DEFAULT_WAIT_TIMEOUT;
    loop {
        if let Some(result) = client.get_transaction_result(digest).await.unwrap() {
            return result;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for transaction {digest} to be mined"
        );
        tokio::time::sleep(DEFAULT_POLL_INTERVAL).await;
    }
}

/// Submit a signed transaction and wait for its execution result.
pub async fn submit_and_wait(
    client: &NodeClient,
    transaction: &SignedTransaction,
    digest: &Digest,
) -> ExecutionResult {
    client
        .submit_transaction(transaction)
        .await
        .expect("submit must succeed");
    wait_for_result(client, digest).await
}

/// Submit a signed transaction and return the rejection error string.
pub async fn submit_and_reject(client: &NodeClient, transaction: &SignedTransaction) -> String {
    client
        .submit_transaction(transaction)
        .await
        .expect_err("submit must fail")
        .to_string()
}

/// A simple math module that can be published in tests.
pub fn module_math() -> Vec<u8> {
    let module = builder::build(
        r#"
            mod math;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
        &[],
    )
    .unwrap();
    bcs::to_bytes(&module).unwrap()
}

/// A simple noop module that can be published in tests.
pub fn module_noop() -> Vec<u8> {
    let module = builder::build(
        r#"
            mod noop;

            pub fn noop() {}
        "#,
        &[],
    )
    .unwrap();
    bcs::to_bytes(&module).unwrap()
}

/// Start three nodes with the same genesis in a deterministic connected topology.
pub async fn start_three_nodes_with_genesis(genesis: &Genesis) -> (TestNode, TestNode, TestNode) {
    let node1 = TestNode::start_with_genesis(genesis).await;
    let node2 =
        TestNode::start_with_bootstrap(genesis, vec![node1.gossip_bootstrap_address().clone()])
            .await;
    let node3 = TestNode::start_with_bootstrap(
        genesis,
        vec![
            node1.gossip_bootstrap_address().clone(),
            node2.gossip_bootstrap_address().clone(),
        ],
    )
    .await;
    // Give peers a short moment to establish outgoing bootstrap connections.
    tokio::time::sleep(GOSSIP_PEER_CONNECT_WAIT).await;
    (node1, node2, node3)
}

/// Assert that a transaction is not found on any of the given nodes.
pub async fn assert_tx_not_found_on_nodes(nodes: &[&TestNode], digest: &Digest) {
    for node in nodes {
        assert!(
            node.client()
                .get_transaction_result(digest)
                .await
                .unwrap()
                .is_none(),
            "transaction {digest} must not be found on node"
        );
    }
}

/// Sign and execute a transaction, waiting for its execution result.
pub async fn sign_and_execute(
    client: &NodeClient,
    keypair: &KeyPair,
    transaction: Transaction,
) -> ExecutionResult {
    let (signed, digest) = transaction.sign(keypair);
    submit_and_wait(client, &signed, &digest).await
}

/// Sign and submit a transaction, expecting rejection.
pub async fn sign_and_execute_expect_rejection(
    client: &NodeClient,
    keypair: &KeyPair,
    transaction: Transaction,
) -> String {
    let (signed, _) = transaction.sign(keypair);
    submit_and_reject(client, &signed).await
}
