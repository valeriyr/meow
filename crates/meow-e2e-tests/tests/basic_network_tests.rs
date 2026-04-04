use meow_e2e_tests::{test_node::TestNode, test_utils};
use meow_types::transaction::{Transaction, transaction_type::TransactionType};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn second_node_successfully_connected_and_synchronized() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let node1 = TestNode::start_with_genesis(&genesis).await;
    let client1 = node1.client();

    // Give mDNS a short moment to discover peers before broadcasting a block.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let gas_coin_ref = test_utils::get_object_ref(client1, &coin_addr).await;
    let transaction = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let _ = test_utils::sign_and_execute(client1, &keypair, transaction).await;

    let node2 = TestNode::start_with_genesis(&genesis).await;
    let client2 = node2.client();

    let fetched = test_utils::wait_for_object(client2, &coin_addr).await;

    assert_eq!(*fetched.address(), coin_addr);
    assert_eq!(fetched.owner().address(), Some(&sender));
}
