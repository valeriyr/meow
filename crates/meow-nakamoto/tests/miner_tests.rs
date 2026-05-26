mod utils;

use meow_genesis::Genesis;
use meow_nakamoto::{
    chain::error::ChainError,
    miner::{Miner, error::MinerError},
    roots,
    store::Store,
};
use meow_nakamoto_types::state_snapshot::StateSnapshot;
use meow_nakamoto_types::{block::Block, miner_config::MinerConfig};
use meow_types::{
    address::Address,
    digest::Digest,
    object::{object_owner::ObjectOwner, object_ref::ObjectRef},
    transaction::{Transaction, transaction_type::TransactionType},
};

//
// ─── prepare_round ───
//

/// `prepare_round` must return `None` when the mempool has no pending transactions.
#[test]
fn prepare_round_returns_none_when_mempool_is_empty() {
    let mut miner = Miner::empty(test_config());

    assert!(miner.prepare_round().is_none());
}

/// `prepare_round` must drain the mempool into the work batch, set the correct
/// height and parent hash on the header, and leave the mempool empty so that
/// a second call returns `None`.
#[test]
fn prepare_round_drains_mempool_and_returns_correct_header() {
    let miner_address = test_miner_address();
    let (genesis, coin_ref) = genesis_with_coin(miner_address);
    let mut miner = Miner::with_genesis(&genesis, test_config());

    let transaction = Transaction::new(
        miner_address,
        coin_ref,
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    let (signed, _) = transaction.sign(&utils::test_keypair());
    miner.submit_transaction(signed).unwrap();

    let expected_parent_hash = Block::genesis().hash();
    let work = miner
        .prepare_round()
        .expect("mempool has one pending transaction");

    assert_eq!(work.header.height, 1);
    assert_eq!(work.header.parent_hash, expected_parent_hash);
    assert_eq!(work.batch.len(), 1);

    // Mempool must be empty after the batch was drained.
    assert!(miner.prepare_round().is_none());
}

//
// ─── commit_mined ───
//

/// `commit_mined` must advance the chain head when the mined block's parent
/// hash still matches the current head at commit time, and the new store must
/// be accessible via `head_store`.
#[test]
fn commit_mined_advances_chain_when_head_matches() {
    let miner_address = test_miner_address();
    let (genesis, coin_ref) = genesis_with_coin(miner_address);
    let mut miner = Miner::with_genesis(&genesis, test_config());

    let (block, new_store) = make_block_with_dummy_transaction(&miner, 1);
    let block_hash = block.hash();

    assert!(miner.commit_mined(block, new_store));

    assert_eq!(miner.head(), block_hash);
    assert_eq!(miner.head_height(), 1);
    assert!(
        miner.head_store().get_object(coin_ref.address()).is_some(),
        "committed store must be accessible via head_store"
    );
}

/// `commit_mined` must discard the block and return `false` when the chain head
/// advanced between `prepare_round` and the commit (e.g. a peer block arrived).
#[test]
fn commit_mined_discards_stale_block() {
    let mut miner = Miner::empty(test_config());

    // Build two height-1 blocks from the same genesis parent (different timestamps
    // produce different hashes so they can coexist in the chain).
    let (block_a, store_a) = make_block_with_dummy_transaction(&miner, 1);
    let (block_b, store_b) = make_block_with_dummy_transaction(&miner, 2);
    let block_a_hash = block_a.hash();

    // Commit block_a first — head is now block_a's hash.
    assert!(miner.commit_mined(block_a, store_a));
    assert_eq!(miner.head(), block_a_hash);
    assert_eq!(miner.head_height(), 1);

    // block_b was ground against genesis; its parent is now stale.
    assert!(!miner.commit_mined(block_b, store_b));
    assert_eq!(
        miner.head(),
        block_a_hash,
        "head must not change after stale commit"
    );
    assert_eq!(miner.head_height(), 1);
}

//
// ─── apply_block ───
//

/// When `apply_block` advances the head, the miner must call `retain_valid` on the
/// mempool. A pending transaction whose gas-coin version was bumped by the new block
/// must be pruned before it can enter a future mining round.
#[test]
fn apply_block_on_reorg_prunes_stale_mempool_transactions() {
    let miner_address = test_miner_address();
    let (genesis, coin_ref) = genesis_with_coin(miner_address);
    let mut miner = Miner::with_genesis(&genesis, test_config());

    // Submit a transaction referencing the coin at its current version (v1).
    let tx_mempool = Transaction::new(
        miner_address,
        coin_ref.clone(),
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    let (signed_mempool, _) = tx_mempool.sign(&utils::test_keypair());
    miner.submit_transaction(signed_mempool).unwrap();

    // Build a peer block that executes a different transaction using the same coin as
    // gas. Executing it bumps the coin's version in the new store, making the
    // mempool transaction stale.
    let tx_peer = Transaction::new(
        miner_address,
        coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed_peer, _) = tx_peer.sign(&utils::test_keypair());
    let parent_store = miner.head_store().clone();
    let (peer_block, _) = utils::mining_work(
        vec![signed_peer],
        parent_store,
        1,
        Block::genesis().hash(),
        1,
        0,
        miner_address,
    )
    .grind()
    .expect("batch has one transaction");
    let peer_block_hash = peer_block.hash();

    // Apply the peer block — head advances, retain_valid is called on the mempool.
    assert_eq!(miner.apply_block(peer_block), Ok(true));
    assert_eq!(miner.head(), peer_block_hash);
    assert_eq!(miner.head_height(), 1);

    // The mempool tx (referencing coin v1) must have been pruned — the coin is now v2.
    assert!(miner.prepare_round().is_none());
}

/// `apply_block` must return `Ok(false)` for a valid block that does not extend
/// the current head (equal-height fork), leaving the mempool intact — `retain_valid`
/// is only called when the head actually changes.
#[test]
fn apply_block_on_fork_preserves_mempool() {
    let miner_address = test_miner_address();
    let (genesis, coin_ref) = genesis_with_coin(miner_address);
    let mut miner = Miner::with_genesis(&genesis, test_config());

    // block_a: directly constructed — commit_mined does not validate.
    let (block_a, store_a) = make_block_with_dummy_transaction(&miner, 1);

    // block_fork: valid height-1 block from genesis; must have a real transaction
    // because it goes through apply_block which enforces the no-empty-block rule.
    let genesis_store = miner.head_store().clone();
    let genesis_hash = miner.head();
    let fork_tx = Transaction::new(
        miner_address,
        coin_ref.clone(),
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed_fork, _) = fork_tx.sign(&utils::test_keypair());
    let (block_fork, _) = utils::mining_work(
        vec![signed_fork],
        genesis_store,
        1,
        genesis_hash,
        2,
        0,
        miner_address,
    )
    .grind()
    .expect("valid transaction must succeed");

    // Commit block_a — head advances to height 1.
    assert!(miner.commit_mined(block_a, store_a));
    assert_eq!(miner.head_height(), 1);

    // Queue a transaction so the mempool is non-empty.
    let transaction = Transaction::new(
        miner_address,
        coin_ref,
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    let (signed, _) = transaction.sign(&utils::test_keypair());
    miner.submit_transaction(signed).unwrap();

    // Apply the fork block (height 1, same as head — valid but does not extend chain).
    assert_eq!(miner.apply_block(block_fork), Ok(false));

    // Mempool must be intact — retain_valid is only called on Ok(true).
    assert!(
        miner.prepare_round().is_some(),
        "mempool must not be pruned on a non-advancing fork block"
    );
}

/// `apply_block` must ignore a block whose parent hash is not in the chain,
/// leaving the head unchanged.
#[test]
fn apply_block_ignores_block_with_unknown_parent() {
    let miner_address = test_miner_address();
    let (genesis, coin_ref) = genesis_with_coin(miner_address);
    let mut miner = Miner::with_genesis(&genesis, test_config());
    let original_head = miner.head();

    // Build a fully valid block, then change only the parent_hash — one defect.
    let transaction = Transaction::new(
        miner_address,
        coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed, _) = transaction.sign(&utils::test_keypair());
    let parent_store = miner.head_store().clone();
    let (mut block, _) = utils::mining_work(
        vec![signed],
        parent_store,
        1,
        miner.head(),
        1,
        0,
        miner_address,
    )
    .grind()
    .expect("batch has one transaction");

    block.header.parent_hash = Digest::new([0xFF; 32]); // unknown — one defect

    assert_eq!(
        miner.apply_block(block),
        Err(MinerError::ChainError(ChainError::UnknownParent))
    );
    assert_eq!(miner.head(), original_head);
}

//
// ─── simulate_transaction ───
//

/// `simulate_transaction` must execute the transaction and return a result
/// without committing any state change to the chain.
#[test]
fn simulate_transaction_returns_result_for_valid_transaction() {
    let miner_address = test_miner_address();
    let (genesis, coin_ref) = genesis_with_coin(miner_address);
    let mut miner = Miner::with_genesis(&genesis, test_config());

    let transaction = Transaction::new(
        miner_address,
        coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );

    assert!(miner.simulate_transaction(transaction).is_ok());

    // Head must not advance — simulation does not commit.
    assert_eq!(miner.head_height(), 0);
}

/// `simulate_transaction` must return `MinerError::SimulationError` when
/// `executor::execute` itself returns `Err`. The coin exists in the store and
/// passes `validate_against_store`, but the executor's ownership check rejects
/// it because the sender does not own the coin.
#[test]
fn simulate_transaction_returns_simulation_error_when_executor_fails() {
    let (genesis, coin_ref) = genesis_with_coin(test_miner_address());
    let mut miner = Miner::with_genesis(&genesis, test_config());

    // Wrong sender: coin address/version/digest match the store, so validate_against_store
    // passes, but resolve_gas_coin_object rejects it with InvalidGasCoinOwner.
    let wrong_sender = Address::from([0xE1; 32]);
    let transaction = Transaction::new(
        wrong_sender,
        coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );

    assert!(matches!(
        miner.simulate_transaction(transaction),
        Err(MinerError::SimulationError(_))
    ));
}

//
// ─── replace_from_snapshot ───
//

/// After a successful snapshot replacement the mempool must be empty,
/// since all previously queued transactions reference stale object versions.
#[test]
fn replace_from_snapshot_wipes_mempool() {
    let miner_address = test_miner_address();
    let (genesis, coin_ref) = genesis_with_coin(miner_address);
    let mut miner = Miner::with_genesis(&genesis, test_config());

    // Queue a transaction so the mempool is non-empty before the snapshot.
    let transaction = Transaction::new(
        miner_address,
        coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed, _) = transaction.sign(&utils::test_keypair());
    miner.submit_transaction(signed).unwrap();

    // Build a valid snapshot at height 1 (ahead of our genesis head at height 0).
    let peer_store = Store::with_objects(genesis.objects().iter().cloned());
    let state_root = roots::compute_state_root(&peer_store);
    let (signed, _) = utils::dummy_signed_transaction();
    let snap_block = utils::make_block(1, Block::genesis().hash(), 9999, state_root, signed);
    let snapshot = StateSnapshot {
        head: snap_block,
        objects: genesis.objects().to_vec(),
    };
    assert!(miner.replace_from_snapshot(snapshot).is_ok());

    assert!(
        miner.prepare_round().is_none(),
        "mempool must be empty after snapshot replacement"
    );
}

//
// ─── Utility functions ───
//

const DIFFICULTY: u32 = 0;

fn test_miner_address() -> Address {
    Address::from(&utils::test_keypair())
}

fn test_config() -> MinerConfig {
    let keypair = utils::test_keypair();
    let reward_address = Address::from(&keypair);
    MinerConfig::new(DIFFICULTY, keypair, reward_address)
}

/// Build a genesis that pre-allocates a coin to `owner` and return the genesis
/// together with the coin's `ObjectRef`.
fn genesis_with_coin(owner: Address) -> (Genesis, ObjectRef) {
    let genesis = Genesis::build(&[(owner, 10_000)]).expect("genesis must build");
    let coin = genesis
        .objects()
        .iter()
        .find(|o| o.owner() == &ObjectOwner::Address(owner))
        .expect("allocation must produce a coin owned by the address");
    let coin_ref = coin.object_ref();
    (genesis, coin_ref)
}

/// Build a valid empty block at the next height from the miner's current head,
/// together with the unchanged parent store. Bypasses grinding — suitable for
/// `commit_mined` and `apply_block` tests where the focus is not on mining itself.
fn make_block_with_dummy_transaction(miner: &Miner, timestamp: u64) -> (Block, Store) {
    let parent_store = miner.head_store().clone();
    let (signed, _) = utils::dummy_signed_transaction();
    let block = utils::make_block(
        miner.head_height() + 1,
        miner.head(),
        timestamp,
        roots::compute_state_root(&parent_store),
        signed,
    );
    (block, parent_store)
}
