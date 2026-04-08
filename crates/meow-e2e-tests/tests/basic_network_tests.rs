use meow_e2e_tests::{test_node::TestNode, test_utils};
use meow_types::{
    object::object_type::ObjectType,
    transaction::{Transaction, transaction_type::TransactionType},
};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn second_node_successfully_connected_and_synchronized() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let node1 = TestNode::start_with_genesis(&genesis).await;
    let client1 = node1.client();

    let gas_coin_ref = test_utils::get_object_ref(client1, &coin_addr).await;
    let transaction = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    // Execute on node1 and capture the result to get the newly created module address.
    let result = test_utils::sign_and_execute(client1, &keypair, transaction).await;
    let module_addr = test_utils::published_module_addr(&result);

    // Start node2 after node1 has already committed a block. Mine one more block on
    // node1 so node2 receives a gossip block with height > its local height (0),
    // triggering gap detection and a pull-sync of the missing blocks.
    let node2 =
        TestNode::start_with_bootstrap(&genesis, vec![node1.gossip_bootstrap_address().clone()])
            .await;
    let client2 = node2.client();

    let gas_coin_ref_2 = test_utils::get_object_ref(client1, &coin_addr).await;
    let tx2 = Transaction::new(
        sender,
        gas_coin_ref_2,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    test_utils::sign_and_execute(client1, &keypair, tx2).await;

    // The module object must only be visible on node2 after chain sync completes.
    let fetched = test_utils::wait_for_object(client2, &module_addr).await;

    assert_eq!(*fetched.address(), module_addr);
    assert_eq!(*fetched.type_(), ObjectType::Module);
}

#[tokio::test]
#[serial]
async fn late_joiner_syncs_multiple_committed_blocks() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let node1 = TestNode::start_with_genesis(&genesis).await;
    let client1 = node1.client();

    let gas_coin_ref_1 = test_utils::get_object_ref(client1, &coin_addr).await;
    let tx1 = Transaction::new(
        sender,
        gas_coin_ref_1,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let result1 = test_utils::sign_and_execute(client1, &keypair, tx1).await;
    let module_addr_1 = test_utils::published_module_addr(&result1);

    let gas_coin_ref_2 = test_utils::get_object_ref(client1, &coin_addr).await;
    let tx2 = Transaction::new(
        sender,
        gas_coin_ref_2,
        TransactionType::MeowModulePublish(test_utils::module_math()),
    );
    let result2 = test_utils::sign_and_execute(client1, &keypair, tx2).await;
    let module_addr_2 = test_utils::published_module_addr(&result2);

    // Node2 joins late and should pull both historical blocks. Mine one more block on
    // node1 so node2 receives a gossip block with height > its local height (0),
    // triggering gap detection and a pull-sync of the missing blocks.
    let node2 =
        TestNode::start_with_bootstrap(&genesis, vec![node1.gossip_bootstrap_address().clone()])
            .await;
    let client2 = node2.client();

    let gas_coin_ref_3 = test_utils::get_object_ref(client1, &coin_addr).await;
    let tx3 = Transaction::new(
        sender,
        gas_coin_ref_3,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    test_utils::sign_and_execute(client1, &keypair, tx3).await;

    let fetched1 = test_utils::wait_for_object(client2, &module_addr_1).await;
    let fetched2 = test_utils::wait_for_object(client2, &module_addr_2).await;

    assert_eq!(*fetched1.address(), module_addr_1);
    assert_eq!(*fetched2.address(), module_addr_2);
    assert_eq!(*fetched1.type_(), ObjectType::Module);
    assert_eq!(*fetched2.type_(), ObjectType::Module);
}

#[tokio::test]
#[serial]
async fn late_joiner_syncs_blocks_mined_during_sync() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let node1 = TestNode::start_with_genesis(&genesis).await;
    let client1 = node1.client();

    // Commit one block before node2 joins.
    let gas_coin_ref_1 = test_utils::get_object_ref(client1, &coin_addr).await;
    let tx1 = Transaction::new(
        sender,
        gas_coin_ref_1,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let result1 = test_utils::sign_and_execute(client1, &keypair, tx1).await;
    let module_addr_1 = test_utils::published_module_addr(&result1);

    // Start node2 after node1 has already committed block 1. Node2 has no blocks yet
    // and will sync once it receives a gossip block with a height gap.
    let node2 =
        TestNode::start_with_bootstrap(&genesis, vec![node1.gossip_bootstrap_address().clone()])
            .await;
    let client2 = node2.client();

    // Mine blocks 2 and 3 on node1. When node2 receives the gossip message for block 2
    // (height 2 > local height 0), it detects the gap and pulls blocks 1-2 from node1.
    // Block 3 arrives via gossip while the pull is in flight and is buffered and applied after.
    let gas_coin_ref_2 = test_utils::get_object_ref(client1, &coin_addr).await;
    let tx2 = Transaction::new(
        sender,
        gas_coin_ref_2,
        TransactionType::MeowModulePublish(test_utils::module_math()),
    );
    let result2 = test_utils::sign_and_execute(client1, &keypair, tx2).await;
    let module_addr_2 = test_utils::published_module_addr(&result2);

    let gas_coin_ref_3 = test_utils::get_object_ref(client1, &coin_addr).await;
    let tx3 = Transaction::new(
        sender,
        gas_coin_ref_3,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let result3 = test_utils::sign_and_execute(client1, &keypair, tx3).await;
    let module_addr_3 = test_utils::published_module_addr(&result3);

    let fetched1 = test_utils::wait_for_object(client2, &module_addr_1).await;
    let fetched2 = test_utils::wait_for_object(client2, &module_addr_2).await;
    let fetched3 = test_utils::wait_for_object(client2, &module_addr_3).await;

    assert_eq!(*fetched1.address(), module_addr_1);
    assert_eq!(*fetched2.address(), module_addr_2);
    assert_eq!(*fetched3.address(), module_addr_3);
}
