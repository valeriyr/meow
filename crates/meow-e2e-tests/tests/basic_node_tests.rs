use meow_e2e_tests::{test_node::TestNode, test_utils};
use meow_types::{
    address::Address,
    identifier::Identifier,
    transaction::{
        Transaction, call::Call, execution_result::ExecutionStatus, input::Input,
        transaction_type::TransactionType,
    },
};
use serial_test::serial;

//
// ─── get_object ───
//

#[tokio::test]
#[serial]
async fn get_unknown_object_returns_none() {
    let node = TestNode::start_minimal().await;

    let result = node
        .client()
        .get_object(&Address::suffixed(0xF1))
        .await
        .unwrap();

    assert!(result.is_none());
}

#[tokio::test]
#[serial]
async fn genesis_coin_object_is_queryable() {
    let (_, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;

    let fetched = test_utils::get_object(node.client(), &coin_addr).await;

    assert_eq!(fetched.address(), &coin_addr);
    assert_eq!(fetched.owner().address(), Some(&sender));
}

//
// ─── Transactions ───
//

#[tokio::test]
#[serial]
async fn published_module_transaction_is_mined() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;
    let client = node.client();

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let transaction = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_math()),
    );
    let result = test_utils::sign_and_execute(client, &keypair, transaction).await;

    assert_eq!(*result.status(), ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "one module object created"
    );
}

#[tokio::test]
#[serial]
async fn meow_call_transaction_is_mined_and_succeeds() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;
    let client = node.client();

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let publish_tx = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_math()),
    );
    let publish_result = test_utils::sign_and_execute(client, &keypair, publish_tx).await;
    assert_eq!(*publish_result.status(), ExecutionStatus::Success);

    let module_addr = test_utils::published_module_addr(&publish_result);

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let call = Call::new(
        module_addr,
        Identifier::new("add").unwrap(),
        vec![Input::raw(&3u64).unwrap(), Input::raw(&5u64).unwrap()],
    );
    let call_tx = Transaction::new(sender, gas_coin_ref, TransactionType::MeowCall(call));
    let call_result = test_utils::sign_and_execute(client, &keypair, call_tx).await;
    assert_eq!(*call_result.status(), ExecutionStatus::Success);
}

//
// ─── get_objects_owned ───
//

#[tokio::test]
#[serial]
async fn get_objects_owned_returns_empty_for_unknown_owner() {
    let node = TestNode::start_minimal().await;

    let objects = node
        .client()
        .get_objects_owned(&Address::suffixed(0xE1))
        .await
        .unwrap();

    assert!(objects.is_empty());
}

#[tokio::test]
#[serial]
async fn get_objects_owned_returns_genesis_coin_for_sender() {
    let (_, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;

    let objects = node.client().get_objects_owned(&sender).await.unwrap();

    assert_eq!(
        objects.len(),
        1,
        "sender should own exactly their genesis coin"
    );
    assert_eq!(objects[0].address(), &coin_addr);
    assert_eq!(objects[0].owner().address(), Some(&sender));
}

#[tokio::test]
#[serial]
async fn published_module_is_immutable_and_not_owned_by_sender() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;
    let client = node.client();

    // Publish a module
    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let publish_tx = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_math()),
    );
    let publish_result = test_utils::sign_and_execute(client, &keypair, publish_tx).await;
    assert_eq!(*publish_result.status(), ExecutionStatus::Success);

    // Modules are immutable objects and are not owned by the sender,
    // so even though the sender published it, it won't appear in get_objects_owned.
    // The sender still owns their genesis coin.
    let objects = client.get_objects_owned(&sender).await.unwrap();

    assert_eq!(
        objects.len(),
        1,
        "sender should own only their genesis coin"
    );
    assert_eq!(objects[0].address(), &coin_addr);
}

//
// ─── get_objects ───
//

#[tokio::test]
#[serial]
async fn get_objects_returns_found_objects_and_none_for_unknown() {
    let (_, _, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;

    let unknown = Address::suffixed(0xF1);
    let results = node
        .client()
        .get_objects(&[coin_addr, unknown])
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert!(results[0].is_some(), "known address should return Some");
    assert_eq!(results[0].as_ref().unwrap().address(), &coin_addr);
    assert!(results[1].is_none(), "unknown address should return None");
}

#[tokio::test]
#[serial]
async fn get_objects_empty_list_returns_empty() {
    let node = TestNode::start_minimal().await;

    let results = node.client().get_objects(&[]).await.unwrap();

    assert!(results.is_empty());
}

#[tokio::test]
#[serial]
async fn get_objects_returns_all_none_for_unknown_addresses() {
    let node = TestNode::start_minimal().await;

    let unknown1 = Address::suffixed(0xF1);
    let unknown2 = Address::suffixed(0xF2);
    let results = node
        .client()
        .get_objects(&[unknown1, unknown2])
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert!(results[0].is_none());
    assert!(results[1].is_none());
}

#[tokio::test]
#[serial]
async fn get_objects_at_limit_succeeds() {
    let node = TestNode::start_minimal().await;
    let addresses: Vec<Address> = (0..100).map(Address::fill).collect(); // exactly 100

    let results = node.client().get_objects(&addresses).await.unwrap();

    assert_eq!(results.len(), 100);
    assert!(results.iter().all(|o| o.is_none()));
}

#[tokio::test]
#[serial]
async fn get_objects_too_many_addresses_returns_400() {
    use meow_node_client::error::NodeClientError;

    let node = TestNode::start_minimal().await;
    let addresses: Vec<Address> = (0..=100).map(Address::fill).collect(); // 101 addresses

    let result = node.client().get_objects(&addresses).await;

    assert!(
        matches!(result, Err(NodeClientError::NodeError { status, .. }) if status == 400),
        "expected 400 NodeError, got: {result:?}"
    );
}

//
// ─── get_transaction ───
//

#[tokio::test]
#[serial]
async fn get_unknown_transaction_returns_none() {
    let node = TestNode::start_minimal().await;

    let result = node
        .client()
        .get_transaction(&meow_types::digest::Digest::ZERO)
        .await
        .unwrap();

    assert!(result.is_none());
}

#[tokio::test]
#[serial]
async fn committed_transaction_is_queryable_by_digest() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;
    let client = node.client();

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let transaction = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_math()),
    );
    let digest = transaction.digest();

    let result = test_utils::sign_and_execute(client, &keypair, transaction).await;
    assert_eq!(*result.status(), ExecutionStatus::Success);

    let fetched = client.get_transaction(&digest).await.unwrap();
    let fetched = fetched.expect("transaction should be queryable by digest once committed");

    assert_eq!(fetched.transaction().digest(), digest);
    assert_eq!(fetched.transaction().sender(), &sender);
    assert!(matches!(
        fetched.transaction().type_(),
        TransactionType::MeowModulePublish(_)
    ));
}

//
// ─── get_state_snapshot ───
//

/// `GET /state-snapshot` must return the current head block and all live objects.
#[tokio::test]
#[serial]
async fn get_state_snapshot_returns_head_and_all_live_objects() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);
    let node = TestNode::start_with_genesis(&genesis).await;
    let client = node.client();

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let transaction = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    test_utils::sign_and_execute(client, &keypair, transaction).await;

    let snapshot = client
        .get_state_snapshot()
        .await
        .expect("state snapshot request must succeed");

    assert_eq!(snapshot.head.header.height, 1);
    assert!(
        snapshot.objects.iter().any(|o| o.address() == &coin_addr),
        "genesis coin must be present in the snapshot"
    );
}
