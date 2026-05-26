mod utils;

use meow_nakamoto::{
    chain::{ChainState, error::ChainError},
    roots,
    store::Store,
    system_transactions,
};
use meow_nakamoto_types::{
    block::Block, block_header::BlockHeader, state_snapshot::SNAPSHOT_DEPTH,
};
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{
        SignedTransaction, Transaction, execution_result::ExecutionResult,
        transaction_type::TransactionType,
    },
};
use rand::{SeedableRng, rngs::StdRng};

//
// ─── apply_block — accepted blocks ───
//

/// A valid block at height 1 must be accepted and advance the chain head.
#[test]
fn valid_block_advances_chain() {
    let (store, cs) = utils::coins(1);
    let mut chain = ChainState::new(store.clone(), 0);

    let (block, _) = make_valid_block(chain.head(), &store, 1, 1, cs[0].0, &cs[0].1);

    assert_eq!(chain.apply_block(block), Ok(true));

    assert_eq!(chain.head_height(), 1);
}

/// Submitting the same block a second time must be silently ignored.
#[test]
fn already_known_block_is_skipped() {
    let (store, cs) = utils::coins(1);
    let mut chain = ChainState::new(store.clone(), 0);

    let (block, _) = make_valid_block(chain.head(), &store, 1, 1, cs[0].0, &cs[0].1);

    assert_eq!(chain.apply_block(block.clone()), Ok(true));
    assert_eq!(chain.apply_block(block), Err(ChainError::AlreadyKnown));

    assert_eq!(chain.head_height(), 1);
}

/// A competing block at the same height as the current head must be stored but
/// must not displace the head — longest chain wins.
#[test]
fn block_on_equal_height_fork_does_not_change_head() {
    let (store, cs) = utils::coins(2);
    let mut chain = ChainState::new(store.clone(), 0);
    let genesis_hash = chain.head();

    let (block1, _) = make_valid_block(genesis_hash, &store, 1, 1, cs[0].0, &cs[0].1);
    assert_eq!(chain.apply_block(block1), Ok(true));
    let head_after_block1 = chain.head();

    // block2 uses a different coin (distinct sender) — produces a different block hash.
    let (block2, _) = make_valid_block(genesis_hash, &store, 1, 2, cs[1].0, &cs[1].1);
    assert_eq!(chain.apply_block(block2), Ok(false));

    assert_eq!(
        chain.head(),
        head_after_block1,
        "head must not change on equal-height fork"
    );
}

/// When a competing fork overtakes the current best chain in length the head
/// must switch to the longer chain (chain reorganization).
#[test]
fn chain_reorg_switches_head_to_longer_fork() {
    let (genesis_store, cs) = utils::coins(3);
    let mut chain = ChainState::new(genesis_store.clone(), 0);
    let genesis_hash = chain.head();

    // Block B1: first chain at height 1 (uses coin 0).
    let (b1, _) = make_valid_block(genesis_hash, &genesis_store, 1, 1, cs[0].0, &cs[0].1);
    assert_eq!(chain.apply_block(b1), Ok(true));
    assert_eq!(chain.head_height(), 1);

    // Block A1: competing chain at height 1 (uses coin 1 — different hash).
    let (a1, store_a1) = make_valid_block(genesis_hash, &genesis_store, 1, 2, cs[1].0, &cs[1].1);
    let a1_hash = a1.hash();
    assert_eq!(chain.apply_block(a1), Ok(false));
    assert_eq!(chain.head_height(), 1);

    // Block A2: extends A1 using coin 2 (untouched in a1's store).
    let (a2, _) = make_valid_block(a1_hash, &store_a1, 2, 3, cs[2].0, &cs[2].1);
    let a2_hash = a2.hash();
    assert_eq!(chain.apply_block(a2), Ok(true));

    assert_eq!(chain.head(), a2_hash, "head must be A2, not B1");
    assert_eq!(chain.head_height(), 2);
}

/// A block mined with gas-consuming transactions must be accepted, advancing
/// the chain head. The valid reward transaction embedded in the block must pass
/// all chain-side validation checks.
#[test]
fn valid_block_with_reward_advances_chain() {
    let (mut chain, block) = chain_and_valid_gas_block();
    let block_hash = block.hash();

    assert_eq!(chain.apply_block(block), Ok(true));

    assert_eq!(chain.head(), block_hash);
    assert_eq!(chain.head_height(), 1);
}

/// Simulates the sync recovery path for a deep fork: blocks from the alternative
/// chain arrive in ascending height order, some below the local head.
///
/// `apply_block` must accept each fork block (storing it as a branch) even when
/// its height is at or below the current head. Once the fork tip arrives and makes
/// the alternative chain longer, a reorg must occur.
#[test]
fn deep_fork_resolves_when_ancestor_blocks_applied_before_tip() {
    let (genesis_store, cs) = utils::coins(7);
    let mut chain = ChainState::new(genesis_store.clone(), 0);
    let genesis_hash = chain.head();

    // Main chain: genesis → A1 → A2 → A3 (head at height 3).
    // Each block uses a fresh coin untouched by its ancestors in this chain.
    let (a1, store_a1) = make_valid_block(genesis_hash, &genesis_store, 1, 10, cs[0].0, &cs[0].1);
    let a1_hash = a1.hash();
    assert_eq!(chain.apply_block(a1), Ok(true));

    let (a2, store_a2) = make_valid_block(a1_hash, &store_a1, 2, 20, cs[1].0, &cs[1].1);
    let a2_hash = a2.hash();
    assert_eq!(chain.apply_block(a2), Ok(true));

    let (a3, _) = make_valid_block(a2_hash, &store_a2, 3, 30, cs[2].0, &cs[2].1);
    let a3_hash = a3.hash();
    assert_eq!(chain.apply_block(a3), Ok(true));
    assert_eq!(chain.head_height(), 3);
    assert_eq!(chain.head(), a3_hash);

    // Fork from genesis: B1–B3 use coins 3–5 (each untouched by ancestors in B chain).
    let (b1, store_b1) = make_valid_block(genesis_hash, &genesis_store, 1, 11, cs[3].0, &cs[3].1);
    let b1_hash = b1.hash();
    assert_eq!(
        chain.apply_block(b1),
        Ok(false),
        "head must not move on fork block below head"
    );
    assert_eq!(chain.head_height(), 3);
    assert_eq!(
        chain.head(),
        a3_hash,
        "head hash must not change on fork block below head"
    );

    let (b2, store_b2) = make_valid_block(b1_hash, &store_b1, 2, 21, cs[4].0, &cs[4].1);
    let b2_hash = b2.hash();
    assert_eq!(chain.apply_block(b2), Ok(false));
    assert_eq!(chain.head_height(), 3);
    assert_eq!(chain.head(), a3_hash);

    let (b3, store_b3) = make_valid_block(b2_hash, &store_b2, 3, 31, cs[5].0, &cs[5].1);
    let b3_hash = b3.hash();
    assert_eq!(chain.apply_block(b3), Ok(false));
    assert_eq!(chain.head_height(), 3);
    assert_eq!(chain.head(), a3_hash);

    // B4 makes the B fork one block longer than the A chain — reorg must happen.
    let (b4, _) = make_valid_block(b3_hash, &store_b3, 4, 41, cs[6].0, &cs[6].1);
    let b4_hash = b4.hash();
    assert_eq!(
        chain.apply_block(b4),
        Ok(true),
        "B4 must trigger a reorg to the longer fork"
    );

    assert_eq!(chain.head(), b4_hash);
    assert_eq!(chain.head_height(), 4);
}

//
// ─── apply_block — rejected blocks ───
//

/// A block whose `results` field does not match deterministic re-execution must
/// be rejected.
#[test]
fn block_with_forged_results_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    block.results = vec![ExecutionResult::failure("forged", Digest::ZERO)];

    assert_eq!(chain.apply_block(block), Err(ChainError::ResultsMismatch));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose `parent_hash` is not in the chain must be rejected.
#[test]
fn block_with_unknown_parent_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();

    block.header.parent_hash = Digest::from([0xDE; 32]); // unknown — one defect

    assert_eq!(chain.apply_block(block), Err(ChainError::UnknownParent));

    assert_eq!(chain.head_height(), 0);
}

/// A block claiming height 2 when its parent is the genesis block (height 0)
/// must be rejected by the height-continuity check.
#[test]
fn block_with_wrong_height_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();

    block.header.height = 2; // wrong — parent is genesis (height 0), so expected is 1

    assert_eq!(
        chain.apply_block(block),
        Err(ChainError::InvalidHeight {
            expected: 1,
            got: 2
        })
    );

    assert_eq!(chain.head_height(), 0);
}

/// A block whose timestamp equals (not strictly exceeds) its parent's timestamp
/// must be rejected to keep time monotonically increasing.
#[test]
fn block_with_non_monotonic_timestamp_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();

    // Genesis has timestamp 0; setting the block's timestamp to 0 violates monotonicity.
    block.header.timestamp = 0; // one defect

    assert_eq!(
        chain.apply_block(block),
        Err(ChainError::TimestampNotAdvancing)
    );

    assert_eq!(chain.head_height(), 0);
}

/// A block whose timestamp is beyond `MAX_BLOCK_FUTURE_DRIFT_MS` ahead of the
/// local clock must be rejected to prevent miners from manipulating the clock.
#[test]
fn block_with_future_timestamp_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();

    block.header.timestamp = u64::MAX; // far in the future — one defect

    assert_eq!(
        chain.apply_block(block),
        Err(ChainError::TimestampTooFarInFuture)
    );

    assert_eq!(chain.head_height(), 0);
}

/// A block with no transactions must be rejected — empty blocks carry no value
/// and are never produced by the local miner.
#[test]
fn empty_block_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    // make_empty_block is intentionally empty — this is the one test where that is the defect.
    let block = make_empty_block(chain.head(), &store, 1, 1);

    assert_eq!(chain.apply_block(block), Err(ChainError::EmptyBlock));

    assert_eq!(chain.head_height(), 0);
}

/// A block that lists the same transaction twice must be rejected before execution —
/// including duplicates is a structural defect, not a runtime failure.
#[test]
fn block_with_duplicate_transaction_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    let dup = block.transactions[0].clone();
    block.transactions.push(dup);
    block.header.transactions_root = roots::compute_transactions_root(&block.transactions);

    assert_eq!(
        chain.apply_block(block),
        Err(ChainError::DuplicateTransaction)
    );

    assert_eq!(chain.head_height(), 0);
}

/// A block containing a transaction whose gas coin does not exist in the parent
/// store must be rejected. `collect_inputs` silently omits the missing coin, so
/// `resolve_gas_coin_object` returns `GasCoinNotFound` — an executor-level `Err`
/// that is distinct from a transaction-level failure wrapped in `Ok`.
#[test]
fn block_with_missing_gas_coin_is_rejected_with_execution_failed() {
    let (mut chain, mut block) = chain_and_valid_gas_block();

    // Replace the transaction with one referencing a coin address that does not
    // exist in the chain's store — one defect.
    let keypair = utils::test_keypair();
    let nonexistent_coin =
        ObjectRef::new(Address::from([0xF9; 32]), ObjectVersion::ZERO, Digest::ZERO);
    let (signed, _) = Transaction::new(
        Address::from(&keypair),
        nonexistent_coin,
        TransactionType::MeowModulePublish(vec![]),
    )
    .sign(&keypair);
    block.transactions = vec![signed];
    block.header.transactions_root = roots::compute_transactions_root(&block.transactions);

    assert_eq!(chain.apply_block(block), Err(ChainError::ExecutionFailed));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose `transactions_root` field does not hash to the actual transaction
/// list must be rejected.
#[test]
fn block_with_wrong_transactions_root_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();

    block.header.transactions_root = Digest::ZERO; // wrong — does not match actual transactions

    assert_eq!(
        chain.apply_block(block),
        Err(ChainError::TransactionsRootMismatch)
    );

    assert_eq!(chain.head_height(), 0);
}

/// A block containing a transaction whose signature does not match its declared
/// sender must be rejected before any execution happens.
#[test]
fn block_with_invalid_transaction_signature_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();

    // Replace the transaction with one whose signer does not match the declared sender — one defect.
    block.transactions = vec![make_mismatched_signature_transaction()];
    block.header.transactions_root = roots::compute_transactions_root(&block.transactions);

    assert_eq!(chain.apply_block(block), Err(ChainError::InvalidSignature));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose `state_root` does not match the store produced by re-executing
/// its transactions must be rejected.
#[test]
fn block_with_wrong_state_root_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    block.header.state_root = Digest::from([0xFF; 32]);

    assert_eq!(chain.apply_block(block), Err(ChainError::StateRootMismatch));

    assert_eq!(chain.head_height(), 0);
}

/// A block that does not meet the configured PoW difficulty must be rejected.
#[test]
fn block_failing_pow_difficulty_is_rejected() {
    // make_valid_gas_block uses difficulty=0 so the block's nonce=0. Apply it to a
    // chain that requires 32 leading zero bits — the hash won't meet that bar.
    let (block, parent_store) = make_valid_gas_block();
    let mut chain = ChainState::new(parent_store, 32);

    assert_eq!(chain.apply_block(block), Err(ChainError::PowCheckFailed));

    assert_eq!(chain.head_height(), 0);
}

/// A block with gas-consuming transactions that omits the reward transaction
/// must be rejected — the reward is mandatory when gas > 0.
#[test]
fn block_with_missing_reward_on_nonzero_gas_block_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    block.header.reward_root = None;
    block.reward_transaction = None;
    block.reward_transaction_result = None;

    assert_eq!(chain.apply_block(block), Err(ChainError::InvalidReward));

    assert_eq!(chain.head_height(), 0);
}

/// A block where `reward_transaction` is present but `reward_transaction_result` is absent
/// (or vice versa) must be rejected by the structural consistency check before any execution.
#[test]
fn block_with_inconsistent_reward_fields_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    block.reward_transaction_result = None; // present + absent → inconsistent

    assert_eq!(
        chain.apply_block(block),
        Err(ChainError::InconsistentReward)
    );

    assert_eq!(chain.head_height(), 0);
}

/// A block whose reward transaction carries an invalid signature must be rejected — the
/// signature is checked before the reward amount or result.
#[test]
fn block_with_invalid_reward_signature_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    // Substitute a transaction whose signer does not match the declared sender address —
    // one structural defect, reward_root updated to match the new digest.
    let bad_reward = make_mismatched_signature_transaction();
    block.header.reward_root = Some(bad_reward.transaction().digest());
    block.reward_transaction = Some(bad_reward);

    assert_eq!(chain.apply_block(block), Err(ChainError::InvalidReward));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose reward transaction claims an amount that does not match the total gas
/// consumed by its transactions must be rejected.
#[test]
fn block_with_wrong_reward_amount_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    let mining_hash = block.header.mining_hash();
    let miner_address = Address::from(&utils::test_keypair());
    // Build a correctly-structured reward transaction but with amount 1 — the actual
    // gas consumed by a module-publish is always greater than 1.
    let wrong_reward_tx =
        system_transactions::make_reward_transaction(miner_address, miner_address, 1, mining_hash);
    let (signed_wrong, _) = wrong_reward_tx.sign(&utils::test_keypair());
    block.header.reward_root = Some(signed_wrong.transaction().digest());
    block.reward_transaction = Some(signed_wrong);

    assert_eq!(chain.apply_block(block), Err(ChainError::InvalidReward));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose `reward_transaction_result` does not match the result produced by
/// re-executing the reward transaction must be rejected.
#[test]
fn block_with_tampered_reward_result_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    let reward_digest = block
        .reward_transaction
        .as_ref()
        .unwrap()
        .transaction()
        .digest();
    block.reward_transaction_result = Some(ExecutionResult::failure("forged", reward_digest));

    assert_eq!(chain.apply_block(block), Err(ChainError::InvalidReward));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose parent has been pruned (header and snapshot removed together once it
/// falls more than `SNAPSHOT_DEPTH` blocks behind the head) must be rejected with
/// `UnknownParent` — the parent no longer exists in the chain.
#[test]
fn block_extending_pruned_parent_is_rejected_with_unknown_parent() {
    let (store, cs) = utils::coins(1);
    let mut chain = ChainState::new(store.clone(), 0);
    let genesis_hash = chain.head();

    // Build a valid height-1 block before advancing the chain. All structural
    // checks pass; the only defect is that the parent (genesis) will be pruned.
    // timestamp=2: distinct from the committed height-1 block (timestamp=1).
    let (valid_block, _) = make_valid_block(genesis_hash, &store, 1, 2, cs[0].0, &cs[0].1);

    // Advance the head past SNAPSHOT_DEPTH so genesis is pruned (header + snapshot).
    advance_head_via_commit(&mut chain, &store, SNAPSHOT_DEPTH + 1);

    assert!(chain.head_height() > SNAPSHOT_DEPTH);

    assert_eq!(
        chain.apply_block(valid_block),
        Err(ChainError::UnknownParent)
    );

    assert_eq!(chain.head_height(), SNAPSHOT_DEPTH + 1);
}

//
// ─── get_blocks_since ───
//

/// `get_blocks_since(h)` must return every block whose height is at least `h`,
/// including the genesis block when `h == 0`.
#[test]
fn get_blocks_since_returns_blocks_from_height() {
    let (store, cs) = utils::coins(2);
    let mut chain = ChainState::new(store.clone(), 0);

    let (block1, _) = make_valid_block(chain.head(), &store, 1, 1, cs[0].0, &cs[0].1);
    assert!(chain.apply_block(block1).is_ok());

    // block2 extends block1 using coin 1 (untouched by block1 in the updated store).
    let (block2, _) = make_valid_block(chain.head(), chain.head_store(), 2, 2, cs[1].0, &cs[1].1);
    assert!(chain.apply_block(block2).is_ok());

    let heights = |blocks: Vec<Block>| -> Vec<u64> {
        let mut hs: Vec<u64> = blocks.iter().map(|b| b.header.height).collect();
        hs.sort_unstable();
        hs
    };

    assert_eq!(heights(chain.get_blocks_since(0)), vec![0, 1, 2]);
    assert_eq!(heights(chain.get_blocks_since(1)), vec![1, 2]);
    assert_eq!(heights(chain.get_blocks_since(2)), vec![2]);
    assert!(chain.get_blocks_since(3).is_empty());
}

//
// ─── prune_finalized_blocks ───
//

/// Once the chain grows past `SNAPSHOT_DEPTH`, blocks below the finality horizon must
/// be absent from `get_blocks_since` — both their headers and snapshots are removed.
#[test]
fn finalized_blocks_are_absent_from_get_blocks_since() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    // Advance to height SNAPSHOT_DEPTH + 1.
    // cutoff = (SNAPSHOT_DEPTH + 1) - SNAPSHOT_DEPTH = 1 → genesis (height 0) is pruned.
    advance_head_via_commit(&mut chain, &store, SNAPSHOT_DEPTH + 1);

    let blocks = chain.get_blocks_since(0);

    // Genesis must be gone; only heights 1..=SNAPSHOT_DEPTH+1 are kept.
    assert_eq!(blocks.len(), SNAPSHOT_DEPTH as usize + 1);
    assert!(
        blocks.iter().all(|b| b.header.height >= 1),
        "no block below the finality horizon must appear in get_blocks_since"
    );
}

//
// ─── get_transaction ───
//

/// `get_transaction` must find a transaction that was committed to the chain and
/// return `None` for an unknown digest.
#[test]
fn get_transaction_returns_committed_transaction() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let (signed, tx_digest) = utils::dummy_signed_transaction();

    let block = utils::make_block(
        1,
        chain.head(),
        1,
        roots::compute_state_root(&store),
        signed,
    );
    chain.commit(block, store.clone());

    assert!(chain.get_transaction(&tx_digest).is_some());
    assert!(chain.get_transaction(&Digest::from([0xAA; 32])).is_none());
}

//
// ─── commit ───
//

/// Results recorded in a committed block must be retrievable by transaction digest.
#[test]
fn committed_transaction_result_is_queryable() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let (signed, tx_digest) = utils::dummy_signed_transaction();
    let block = utils::make_block(
        1,
        chain.head(),
        1,
        roots::compute_state_root(&store),
        signed,
    );
    chain.commit(block, store.clone());

    assert!(chain.get_transaction_result(&tx_digest).is_some());
}

/// `commit` must panic when given an empty block — an empty block is a programmer error.
#[test]
#[should_panic(expected = "commit called with structurally invalid block")]
fn commit_panics_on_empty_block() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);
    chain.commit(make_empty_block(chain.head(), &store, 1, 1), store);
}

//
// ─── from_snapshot ───
//

/// `from_snapshot` must anchor the chain at the given block, making it the head.
#[test]
fn from_snapshot_anchors_chain_at_given_block() {
    let store = Store::default();
    let state_root = roots::compute_state_root(&store);
    let (signed, _) = utils::dummy_signed_transaction();
    let block = utils::make_block(42, Digest::from([0xAA; 32]), 100, state_root, signed);
    let block_hash = block.hash();

    let chain = ChainState::from_snapshot(0, block, store, 0).expect("valid snapshot");

    assert_eq!(chain.head(), block_hash);
    assert_eq!(chain.head_height(), 42);
}

/// `from_snapshot` must reject a snapshot whose height does not exceed the current head.
#[test]
fn from_snapshot_rejects_non_advancing_height() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    let block = utils::make_block(
        0,
        Digest::ZERO,
        1,
        roots::compute_state_root(&store),
        signed,
    ); // height 0: same as current_head_height=0 → not advancing

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::SnapshotNotAdvancing {
            snap_height: 0,
            head_height: 0
        }
    );
}

/// `from_snapshot` must reject a snapshot whose head block has no transactions.
#[test]
fn from_snapshot_rejects_empty_block() {
    let store = Store::default();
    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: Block::genesis().hash(),
            transactions_root: roots::compute_transactions_root(&[]),
            reward_root: None,
            state_root: roots::compute_state_root(&store),
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![],
        reward_transaction: None,
        reward_transaction_result: None,
    };

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::EmptyBlock
    );
}

/// `from_snapshot` must reject a snapshot whose head block contains duplicate transactions.
#[test]
fn from_snapshot_rejects_duplicate_transaction() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    let dup = signed.clone();
    let mut block = utils::make_block(
        1,
        Block::genesis().hash(),
        1,
        roots::compute_state_root(&store),
        signed,
    );
    block.transactions.push(dup.clone());
    block.results.push(block.results[0].clone());
    block.header.transactions_root = roots::compute_transactions_root(&block.transactions);

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::DuplicateTransaction
    );
}

/// `from_snapshot` must reject a snapshot whose head block timestamp is too far in the future.
#[test]
fn from_snapshot_rejects_future_timestamp() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    let block = utils::make_block(
        1,
        Block::genesis().hash(),
        u64::MAX,
        roots::compute_state_root(&store),
        signed,
    ); // u64::MAX: absurdly far in the future

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::TimestampTooFarInFuture
    );
}

/// `from_snapshot` must reject a block whose results count differs from its transaction count.
#[test]
fn from_snapshot_rejects_results_count_mismatch() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    let mut block = utils::make_block(
        1,
        Block::genesis().hash(),
        1,
        roots::compute_state_root(&store),
        signed,
    );
    block.results.clear(); // mismatch: 1 transaction, 0 results

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::ResultsCountMismatch
    );
}

/// `from_snapshot` must reject a block that has `reward_transaction` but no
/// `reward_transaction_result`, or vice versa.
#[test]
fn from_snapshot_rejects_inconsistent_reward() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    let (reward_signed, _) = utils::dummy_signed_transaction();
    let mut block = utils::make_block(
        1,
        Block::genesis().hash(),
        1,
        roots::compute_state_root(&store),
        signed,
    );
    block.reward_transaction = Some(reward_signed); // present without matching result

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::InconsistentReward
    );
}

/// `from_snapshot` must reject a snapshot whose `reward_root` does not match the reward transaction.
#[test]
fn from_snapshot_rejects_reward_root_mismatch() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    let mut block = utils::make_block(
        1,
        Block::genesis().hash(),
        1,
        roots::compute_state_root(&store),
        signed,
    );
    block.header.reward_root = Some(Digest::from([0xFF; 32])); // wrong: body has no reward transaction

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::RewardRootMismatch
    );
}

/// `from_snapshot` must reject a snapshot whose transactions root does not match its transaction list.
#[test]
fn from_snapshot_rejects_transactions_root_mismatch() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    let mut block = utils::make_block(
        1,
        Block::genesis().hash(),
        1,
        roots::compute_state_root(&store),
        signed,
    );
    block.header.transactions_root = Digest::ZERO; // wrong: does not match `transactions`

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::TransactionsRootMismatch
    );
}

/// `from_snapshot` must reject a snapshot whose head block does not meet PoW difficulty.
#[test]
fn from_snapshot_rejects_pow_failure() {
    let store = Store::default();
    let (signed, _) = utils::dummy_signed_transaction();
    // nonce=0: not mined; won't satisfy difficulty=32
    let block = utils::make_block(
        1,
        Block::genesis().hash(),
        1,
        roots::compute_state_root(&store),
        signed,
    );

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 32)
            .err()
            .expect("expected rejection"),
        ChainError::PowCheckFailed
    );
}

/// `from_snapshot` must reject a snapshot whose objects do not match the claimed state root.
#[test]
fn from_snapshot_rejects_state_root_mismatch() {
    let (store, _) = utils::coins(1);
    let (signed, _) = utils::dummy_signed_transaction();
    let block = utils::make_block(1, Block::genesis().hash(), 1, Digest::ZERO, signed); // state_root=ZERO: wrong, actual root of non-empty store != ZERO

    assert_eq!(
        ChainState::from_snapshot(0, block, store, 0)
            .err()
            .expect("expected rejection"),
        ChainError::StateRootMismatch
    );
}

/// A block that extends the snapshot head must be accepted by `apply_block`.
#[test]
fn from_snapshot_accepts_block_extending_snapshot() {
    let (store, cs) = utils::coins(1);
    let state_root = roots::compute_state_root(&store);
    let (signed, _) = utils::dummy_signed_transaction();
    let snap_block = utils::make_block(5, Digest::from([0xBB; 32]), 50, state_root, signed);
    let snap_hash = snap_block.hash();

    let mut chain =
        ChainState::from_snapshot(0, snap_block, store.clone(), 0).expect("valid snapshot");

    let (next, _) = make_valid_block(snap_hash, &store, 6, 60, cs[0].0, &cs[0].1);
    assert_eq!(chain.apply_block(next), Ok(true));
    assert_eq!(chain.head_height(), 6);
}

//
// ─── sync ───
//

/// `sync_from_height` must return 0 while the chain head is within the first
/// `SNAPSHOT_DEPTH` blocks — there is nothing to look back past.
#[test]
fn sync_from_height_is_zero_below_snapshot_depth() {
    let store = Store::default();
    let chain = ChainState::new(store.clone(), 0);
    assert_eq!(chain.sync_from_height(), 0);
}

/// `sync_from_height` must return `head_height - SNAPSHOT_DEPTH` once the chain
/// grows past the snapshot window.
#[test]
fn sync_from_height_is_head_minus_snapshot_depth_when_beyond() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    advance_head_via_commit(&mut chain, &store, SNAPSHOT_DEPTH + 10);

    assert_eq!(chain.sync_from_height(), 10);
}

//
// ─── Utility functions ───
//

/// Build a block with one `MeowModulePublish` transaction using the given coin,
/// grind nonce (difficulty 0 so nonce = 0), and return the block and new store.
fn make_valid_block(
    parent_hash: Digest,
    parent_store: &Store,
    height: u64,
    timestamp: u64,
    keypair_seed: [u8; 32],
    coin_ref: &ObjectRef,
) -> (Block, Store) {
    let keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed(keypair_seed));
    let miner_address = Address::from(&keypair);
    let transaction = Transaction::new(
        miner_address,
        coin_ref.clone(),
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed, _) = transaction.sign(&keypair);
    utils::mining_work(
        vec![signed],
        parent_store.clone(),
        height,
        parent_hash,
        timestamp,
        0,
        miner_address,
    )
    .grind()
    .expect("valid transaction must succeed")
}

/// Build a block with no transactions. Intentionally invalid — used only in tests that
/// verify the empty-block rejection path.
fn make_empty_block(
    parent_hash: Digest,
    parent_store: &Store,
    height: u64,
    timestamp: u64,
) -> Block {
    Block {
        header: BlockHeader {
            height,
            parent_hash,
            transactions_root: roots::compute_transactions_root(&[]),
            reward_root: None,
            state_root: roots::compute_state_root(parent_store),
            timestamp,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![],
        reward_transaction: None,
        reward_transaction_result: None,
    }
}

/// Build a signed transaction whose sender address does not correspond to the
/// signing keypair, making the signature cryptographically invalid
/// (`SignerMismatch` from `validate_signed_transaction`).
fn make_mismatched_signature_transaction() -> SignedTransaction {
    let keypair_a = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0xA1; 32]));
    let keypair_b = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0xB2; 32]));
    let transaction = Transaction::new(
        Address::from(&keypair_a),
        ObjectRef::new(Address::ZERO, ObjectVersion::ZERO, Digest::ZERO),
        TransactionType::MeowModulePublish(vec![1]),
    );
    let (signed, _) = transaction.sign(&keypair_b); // wrong keypair → SignerMismatch
    signed
}

/// Mine a valid height-1 block against a fresh genesis store. Returns the block
/// and its parent store so callers can construct a matching chain or chain variant.
///
/// The block uses difficulty=0 so nonce=0. Tests that need a specific chain difficulty
/// can pass the returned store to `ChainState::new(store, difficulty)`.
fn make_valid_gas_block() -> (Block, Store) {
    let keypair = utils::test_keypair();
    let miner_address = Address::from(&keypair);
    let (parent_store, gas_coin_ref) = utils::genesis_store_with_coin(miner_address);
    let transaction = Transaction::new(
        miner_address,
        gas_coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed, _) = transaction.sign(&keypair);
    let (block, _) = utils::mining_work(
        vec![signed],
        parent_store.clone(),
        1,
        Block::genesis().hash(),
        1,
        0,
        miner_address,
    )
    .grind()
    .expect("batch has one transaction");
    (block, parent_store)
}

/// Build a `ChainState` at genesis and a valid mined block at height 1 that
/// includes a gas-consuming transaction and a correct reward transaction.
fn chain_and_valid_gas_block() -> (ChainState, Block) {
    let (block, parent_store) = make_valid_gas_block();
    (ChainState::new(parent_store, 0), block)
}

/// Advance the chain head by `n` blocks using `commit()`, bypassing all validation.
/// Each block carries a height and timestamp equal to its sequence number so
/// consecutive blocks have distinct hashes without requiring PoW.
fn advance_head_via_commit(chain: &mut ChainState, store: &Store, n: u64) {
    let mut parent_hash = chain.head();
    let start = chain.head_height() + 1;
    for h in start..=start + n - 1 {
        let (signed, _) = utils::dummy_signed_transaction();
        let block = utils::make_block(h, parent_hash, h, roots::compute_state_root(store), signed);
        parent_hash = block.hash();
        chain.commit(block, store.clone());
    }
}
