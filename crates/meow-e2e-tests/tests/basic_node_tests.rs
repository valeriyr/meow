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

#[tokio::test]
#[serial]
async fn get_unknown_object_returns_none() {
    let node = TestNode::start_empty().await;

    let result = node
        .client()
        .get_object(&Address::fill(0xAB))
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

    let module_addr = *publish_result.created_objects()[0].address();

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
