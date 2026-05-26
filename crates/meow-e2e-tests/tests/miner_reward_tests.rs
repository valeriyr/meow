use meow_e2e_tests::{test_node::TestNode, test_utils};
use meow_types::{
    address::Address,
    system_framework::meow_coin::meow_coin_object,
    transaction::{
        Transaction, execution_result::ExecutionStatus, transaction_type::TransactionType,
    },
};
use serial_test::serial;

//
// ─── Block reward ───
//

#[tokio::test]
#[serial]
async fn miner_receives_reward_coin_after_block_with_fees() {
    // The reward coin must be minted to reward_address with balance equal to
    // the total gas collected from user transactions in the block.
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let miner_keypair = test_utils::test_keypair_from_seed([99; 32]);
    let miner_reward_address = Address::from(&miner_keypair);
    let miner_config = test_utils::test_miner_config(miner_keypair, miner_reward_address);
    let node = TestNode::start_with_genesis_and_miner_config(&genesis, miner_config).await;
    let client = node.client();

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let transaction = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let result = test_utils::sign_and_execute(client, &keypair, transaction).await;
    assert_eq!(*result.status(), ExecutionStatus::Success);

    let reward_coins = test_utils::wait_for_objects_owned(client, &miner_reward_address).await;

    assert_eq!(reward_coins.len(), 1);
    let reward_coin_balance = meow_coin_object::balance_from_object(&reward_coins[0]).unwrap();
    assert_eq!(reward_coin_balance, result.gas_used());
}

#[tokio::test]
#[serial]
async fn reward_is_minted_to_reward_address_not_signer() {
    // When reward_address differs from the miner's signing address, the minted
    // coin goes only to reward_address — the signer receives nothing.
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(10_000);

    let miner_keypair = test_utils::test_keypair_from_seed([99; 32]);
    let miner_address = Address::from(&miner_keypair);
    let miner_reward_address = Address::suffixed(0xF1);
    assert_ne!(miner_address, miner_reward_address);

    let miner_config = test_utils::test_miner_config(miner_keypair, miner_reward_address);
    let node = TestNode::start_with_genesis_and_miner_config(&genesis, miner_config).await;
    let client = node.client();

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let transaction = Transaction::new(
        sender,
        gas_coin_ref,
        TransactionType::MeowModulePublish(test_utils::module_noop()),
    );
    let result = test_utils::sign_and_execute(client, &keypair, transaction).await;
    assert_eq!(*result.status(), ExecutionStatus::Success);

    let reward_coins = test_utils::wait_for_objects_owned(client, &miner_reward_address).await;
    assert_eq!(reward_coins.len(), 1);
    let reward_coin_balance = meow_coin_object::balance_from_object(&reward_coins[0]).unwrap();
    assert_eq!(reward_coin_balance, result.gas_used());

    let signer_coins = client.get_objects_owned(&miner_address).await.unwrap();
    assert!(
        signer_coins.is_empty(),
        "miner signing address must not receive the reward coin"
    );
}

#[tokio::test]
#[serial]
async fn total_reward_equals_sum_of_fees_across_multiple_transactions() {
    // Each block that contains transactions produces one reward coin. The total
    // balance across all reward coins must equal the total gas paid across all
    // transactions, regardless of how they are batched into blocks.
    let (keypair, sender, genesis, coin_addr) = test_utils::single_account_genesis(50_000);

    let miner_keypair = test_utils::test_keypair_from_seed([99; 32]);
    let miner_reward_address = Address::from(&miner_keypair);
    let miner_config = test_utils::test_miner_config(miner_keypair, miner_reward_address);
    let node = TestNode::start_with_genesis_and_miner_config(&genesis, miner_config).await;
    let client = node.client();

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let result1 = test_utils::sign_and_execute(
        client,
        &keypair,
        Transaction::new(
            sender,
            gas_coin_ref,
            TransactionType::MeowModulePublish(test_utils::module_noop()),
        ),
    )
    .await;
    assert_eq!(*result1.status(), ExecutionStatus::Success);

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let result2 = test_utils::sign_and_execute(
        client,
        &keypair,
        Transaction::new(
            sender,
            gas_coin_ref,
            TransactionType::MeowModulePublish(test_utils::module_math()),
        ),
    )
    .await;
    assert_eq!(*result2.status(), ExecutionStatus::Success);

    let gas_coin_ref = test_utils::get_object_ref(client, &coin_addr).await;
    let result3 = test_utils::sign_and_execute(
        client,
        &keypair,
        Transaction::new(
            sender,
            gas_coin_ref,
            TransactionType::MeowModulePublish(test_utils::module_noop()),
        ),
    )
    .await;
    assert_eq!(*result3.status(), ExecutionStatus::Success);

    let total_gas_paid = result1.gas_used() + result2.gas_used() + result3.gas_used();

    // Each sign_and_execute waits for confirmation before the next transaction is
    // submitted, so the mempool is empty at the start of each mining round — each
    // transaction lands in its own block with its own reward.
    let blocks = client.get_blocks_since(1).await.unwrap();
    assert_eq!(
        blocks.len(),
        3,
        "expected exactly 3 blocks (one per transaction)"
    );
    assert!(
        blocks.iter().all(|b| b.reward_transaction.is_some()),
        "every block must carry a reward transaction"
    );

    let reward_coins = test_utils::wait_for_objects_owned(client, &miner_reward_address).await;
    assert_eq!(reward_coins.len(), 3, "expected one reward coin per block");

    let total_reward: u64 = reward_coins
        .iter()
        .map(|c| meow_coin_object::balance_from_object(c).unwrap())
        .sum();

    assert_eq!(total_reward, total_gas_paid);
}
