use std::time::Duration;

use meow_e2e_tests::{test_node::TestNode, test_utils};
use meow_genesis::Genesis;
use meow_types::{
    address::Address,
    identifier::Identifier,
    system_framework::meow_coin::{self, meow_coin_object},
    transaction::{
        SignedTransaction, Transaction, call::Call, execution_result::ExecutionStatus,
        input::Input, transaction_type::TransactionType,
    },
};
use serial_test::serial;

/// The wait after submitting an invalid transaction before asserting it did not propagate.
const GOSSIP_PROPAGATION_WAIT: Duration = Duration::from_millis(500);

//
// ─── Signature validation ───
//

#[tokio::test]
#[serial]
async fn invalid_signature_transaction_is_rejected_and_not_propagated_three_nodes() {
    let (_victim_keypair, victim, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let (node1, node2, node3) = test_utils::start_three_nodes_with_genesis(&genesis).await;

    let gas_coin_ref = test_utils::get_object_ref(node1.client(), &coin_addr).await;

    let tx = Transaction::new(
        victim,
        gas_coin_ref.clone(),
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let tx_digest = tx.digest();

    // Sign a different transaction and attach its signature to `tx` to forge an invalid signature.
    let other_tx = Transaction::new(
        victim,
        gas_coin_ref.clone(),
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    let attacker_keypair = test_utils::test_keypair_from_seed([7; 32]);
    let (other_signed, _) = other_tx.sign(&attacker_keypair);
    let invalid_signed = SignedTransaction::new(tx, other_signed.signature().clone());

    let err = test_utils::submit_and_reject(node1.client(), &invalid_signed).await;
    assert!(
        err.to_string().contains("invalid signature"),
        "unexpected error: {err}"
    );

    tokio::time::sleep(GOSSIP_PROPAGATION_WAIT).await;

    test_utils::assert_tx_not_found_on_nodes(&[&node1, &node2, &node3], &tx_digest).await;
}

#[tokio::test]
#[serial]
async fn forged_sender_transaction_is_rejected() {
    let (_victim_keypair, victim, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let (node1, node2, node3) = test_utils::start_three_nodes_with_genesis(&genesis).await;

    let gas_coin_ref = test_utils::get_object_ref(node1.client(), &coin_addr).await;
    let tx = Transaction::new(
        victim,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let tx_digest = tx.digest();

    // Attacker signs a transaction that claims to be from the victim.
    let attacker_keypair = test_utils::test_keypair_from_seed([9; 32]);
    let (forged_signed, _) = tx.sign(&attacker_keypair);

    let err = test_utils::submit_and_reject(node3.client(), &forged_signed).await;
    assert!(
        err.to_string().contains("invalid signature"),
        "unexpected error: {err}"
    );

    tokio::time::sleep(GOSSIP_PROPAGATION_WAIT).await;

    // The forged transaction must not have been committed on any node.
    test_utils::assert_tx_not_found_on_nodes(&[&node1, &node2, &node3], &tx_digest).await;

    let coin_after = test_utils::get_object(node1.client(), &coin_addr).await;
    let balance_after = meow_coin_object::balance_from_object(&coin_after).unwrap();
    assert_eq!(
        balance_after, 10_000,
        "victim balance must be unchanged after spoofed transaction rejection"
    );
}

//
// ─── Object reference validation ───
//

#[tokio::test]
#[serial]
async fn reusing_same_gas_coin_ref_is_forbidden() {
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let node = TestNode::start_with_genesis(&genesis).await;
    let client = node.client();

    let same_gas_ref = test_utils::get_object_ref(client, &coin_addr).await;

    let tx1 = Transaction::new(
        sender,
        same_gas_ref.clone(),
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let result1 = test_utils::sign_and_execute(client, &keypair, tx1).await;
    assert!(matches!(result1.status(), ExecutionStatus::Success));

    let tx2 = Transaction::new(
        sender,
        same_gas_ref,
        // Use different bytes so tx2 has a different digest from tx1.
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    let err = test_utils::sign_and_execute_expect_rejection(client, &keypair, tx2).await;

    assert!(
        err.contains("invalid object reference") && err.contains("invalid version"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
#[serial]
async fn reusing_same_owned_object_input_ref_is_forbidden() {
    // Genesis with two coins for sender: one used as gas, the other as the owned object.
    let keypair = test_utils::test_keypair();
    let sender = Address::from(&keypair);
    let genesis = Genesis::build(&[(sender, 10_000), (sender, 500)]).expect("genesis must build");
    let receiver = Address::suffixed(0xE1);

    let node = TestNode::start_with_genesis(&genesis).await;
    let client = node.client();

    // Identify the two coins: gas coin (first one found) and the owned coin (the other).
    let all_coin_addrs = test_utils::genesis_coin_addrs(&genesis, sender);
    let gas_coin_addr = all_coin_addrs[0];
    let owned_coin_addr = all_coin_addrs[1];

    let stale_owned_ref = test_utils::get_object_ref(client, &owned_coin_addr).await;

    // Transfer the minted coin to receiver (succeeds).
    let gas_for_transfer1 = test_utils::get_object_ref(client, &gas_coin_addr).await;
    let transfer1 = Call::new(
        meow_coin::MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("transfer").unwrap(),
        vec![
            Input::Object(stale_owned_ref.clone()),
            Input::raw(&receiver).unwrap(),
        ],
    );
    let transfer1_tx = Transaction::new(
        sender,
        gas_for_transfer1,
        TransactionType::MeowCall(transfer1),
    );
    let transfer1_result = test_utils::sign_and_execute(client, &keypair, transfer1_tx).await;
    assert!(matches!(
        transfer1_result.status(),
        ExecutionStatus::Success
    ));

    // Try to transfer the same coin again using the stale ref (must be rejected at submit time).
    let gas_for_transfer2 = test_utils::get_object_ref(client, &gas_coin_addr).await;
    let transfer2 = Call::new(
        meow_coin::MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("transfer").unwrap(),
        vec![Input::Object(stale_owned_ref), Input::raw(&sender).unwrap()],
    );
    let transfer2_tx = Transaction::new(
        sender,
        gas_for_transfer2,
        TransactionType::MeowCall(transfer2),
    );
    let err = test_utils::sign_and_execute_expect_rejection(client, &keypair, transfer2_tx).await;
    assert!(
        err.contains("invalid object reference") && err.contains("invalid version"),
        "unexpected error: {err}"
    );
}
