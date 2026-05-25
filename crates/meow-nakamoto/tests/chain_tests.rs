mod utils;

use std::{slice, sync::Arc};

use meow_nakamoto::{
    chain::ChainState, miner::mining_work::MiningWork, roots, store::Store, system_transactions,
};
use meow_nakamoto_types::{block::Block, block_header::BlockHeader};
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

/// A valid empty block at height 1 must be accepted and advance the chain head.
#[test]
fn valid_empty_block_advances_chain() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block = make_empty_block(chain.head(), &store, 1, 1);

    assert!(chain.apply_block(block));

    assert_eq!(chain.head_height(), 1);
}

/// Submitting the same block a second time must be silently ignored.
#[test]
fn already_known_block_is_skipped() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block = make_empty_block(chain.head(), &store, 1, 1);

    assert!(chain.apply_block(block.clone()));
    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 1);
}

/// A competing block at the same height as the current head must be stored but
/// must not displace the head — longest chain wins.
#[test]
fn block_on_equal_height_fork_does_not_change_head() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);
    let genesis_hash = chain.head();

    let block1 = make_empty_block(genesis_hash, &store, 1, 1);
    assert!(chain.apply_block(block1));
    let head_after_block1 = chain.head();

    // block2 has the same height but a different timestamp (→ different hash).
    let block2 = make_empty_block(genesis_hash, &store, 1, 2);
    assert!(!chain.apply_block(block2));

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
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);
    let genesis_hash = chain.head();

    // Block B1: first chain at height 1.
    let b1 = make_empty_block(genesis_hash, &store, 1, 1);
    assert!(chain.apply_block(b1));
    assert_eq!(chain.head_height(), 1);

    // Block A1: competing chain at height 1 (different timestamp → different hash).
    let a1 = make_empty_block(genesis_hash, &store, 1, 2);
    let a1_hash = a1.hash();
    assert!(!chain.apply_block(a1));
    assert_eq!(chain.head_height(), 1);

    // Block A2: extends A1 — the A chain is now longer; reorg must happen.
    let a2 = make_empty_block(a1_hash, &store, 2, 3);
    let a2_hash = a2.hash();
    assert!(chain.apply_block(a2));

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

    assert!(chain.apply_block(block));

    assert_eq!(chain.head(), block_hash);
    assert_eq!(chain.head_height(), 1);
}

//
// ─── apply_block — rejected blocks ───
//

/// An empty block whose `results` field is non-empty must be rejected because
/// re-executing produces no results, causing a mismatch.
#[test]
fn block_with_forged_results_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root: roots::compute_transactions_root(&[]),
            state_root: roots::compute_state_root(&store),
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![ExecutionResult::failure("forged", Digest::ZERO)],
        reward_transaction: None,
        reward_transaction_result: None,
    };

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
    assert!(chain.get_transaction_result(&Digest::ZERO).is_none());
}

/// A block whose `parent_hash` is not in the chain must be rejected.
#[test]
fn block_with_unknown_parent_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let unknown_parent = Digest::from([0xDE; 32]);
    let block = make_empty_block(unknown_parent, &store, 1, 1);

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block claiming height 2 when its parent is the genesis block (height 0)
/// must be rejected by the height-continuity check.
#[test]
fn block_with_wrong_height_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block = make_empty_block(chain.head(), &store, 2, 1);

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose timestamp equals (not strictly exceeds) its parent's timestamp
/// must be rejected to keep time monotonically increasing.
#[test]
fn block_with_non_monotonic_timestamp_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    // Genesis has timestamp 0; a block at height 1 with timestamp 0 violates the rule.
    let block = make_empty_block(chain.head(), &store, 1, 0);

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose timestamp is beyond `MAX_BLOCK_FUTURE_DRIFT_MS` ahead of the
/// local clock must be rejected to prevent miners from manipulating the clock.
#[test]
fn block_with_future_timestamp_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    // u64::MAX is so far in the future that no reasonable clock drift allows it.
    let block = make_empty_block(chain.head(), &store, 1, u64::MAX);

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose `transactions_root` field does not hash to the actual transaction
/// list must be rejected.
#[test]
fn block_with_wrong_transactions_root_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root: Digest::ZERO, // wrong: should be hash of []
            state_root: roots::compute_state_root(&store),
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![],
        reward_transaction: None,
        reward_transaction_result: None,
    };

    // compute_transactions_root(&[]) != Digest::ZERO, so this must be rejected.
    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block containing a transaction whose signature does not match its declared
/// sender must be rejected before any execution happens.
#[test]
fn block_with_invalid_transaction_signature_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let signed = make_mismatched_signature_transaction();
    let transactions_root = roots::compute_transactions_root(slice::from_ref(&signed));

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root,
            state_root: Digest::ZERO, // unreachable — rejected at signature check
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![signed],
        results: vec![],
        reward_transaction: None,
        reward_transaction_result: None,
    };

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block whose `state_root` does not match the store produced by re-executing
/// its transactions must be rejected.
#[test]
fn block_with_wrong_state_root_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root: roots::compute_transactions_root(&[]),
            state_root: Digest::from([0xFF; 32]), // wrong
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![],
        reward_transaction: None,
        reward_transaction_result: None,
    };

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block that does not meet the configured PoW difficulty must be rejected.
#[test]
fn block_failing_pow_difficulty_is_rejected() {
    let store = Store::default();
    // Require 32 leading zero bits — effectively impossible for nonce = 0.
    let mut chain = ChainState::new(store.clone(), 32);

    let block = make_empty_block(chain.head(), &store, 1, 1);

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// An empty block (zero gas) that nonetheless carries a `reward_transaction`
/// must be rejected — a reward is only valid when gas was consumed.
#[test]
fn block_with_reward_on_zero_gas_block_is_rejected() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let keypair = utils::test_keypair();
    let miner_address = Address::from(&keypair);
    let reward_transaction = system_transactions::make_reward_transaction(
        miner_address,
        miner_address,
        1_000,
        chain.head(),
    );
    let (signed_reward, _) = reward_transaction.sign(&keypair);

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root: roots::compute_transactions_root(&[]),
            state_root: roots::compute_state_root(&store),
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![],
        reward_transaction: Some(signed_reward),
        reward_transaction_result: None,
    };

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

/// A block with gas-consuming transactions that omits the reward transaction
/// must be rejected — the reward is mandatory when gas > 0.
#[test]
fn block_with_missing_reward_on_nonzero_gas_block_is_rejected() {
    let (mut chain, mut block) = chain_and_valid_gas_block();
    block.reward_transaction = None;
    block.reward_transaction_result = None;

    assert!(!chain.apply_block(block));

    assert_eq!(chain.head_height(), 0);
}

//
// ─── get_blocks_since ───
//

/// `get_blocks_since(h)` must return every block whose height is at least `h`,
/// including the genesis block when `h == 0`.
#[test]
fn get_blocks_since_returns_blocks_from_height() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let block1 = make_empty_block(chain.head(), &store, 1, 1);
    assert!(chain.apply_block(block1));

    let block2 = make_empty_block(chain.head(), &store, 2, 2);
    assert!(chain.apply_block(block2));

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
// ─── get_transaction ───
//

/// `get_transaction` must find a transaction that was committed to the chain and
/// return `None` for an unknown digest.
#[test]
fn get_transaction_returns_committed_transaction() {
    let store = Store::default();
    let mut chain = ChainState::new(store.clone(), 0);

    let (signed, tx_digest) = make_dummy_signed_transaction();

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root: roots::compute_transactions_root(slice::from_ref(&signed)),
            state_root: roots::compute_state_root(&store),
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![signed],
        results: vec![],
        reward_transaction: None,
        reward_transaction_result: None,
    };
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

    let tx_digest = Digest::from([0xAB; 32]);
    let result = ExecutionResult::failure("test", tx_digest);

    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root: roots::compute_transactions_root(&[]),
            state_root: roots::compute_state_root(&store),
            timestamp: 1,
            nonce: 0,
        },
        transactions: vec![],
        results: vec![result.clone()],
        reward_transaction: None,
        reward_transaction_result: None,
    };
    chain.commit(block, store.clone());

    assert_eq!(chain.get_transaction_result(&tx_digest), Some(&result));
}

//
// ─── Utility functions ───
//

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

/// Build a validly signed transaction for use with `commit` (no gas coin required).
fn make_dummy_signed_transaction() -> (SignedTransaction, Digest) {
    let keypair = utils::test_keypair();
    let transaction = Transaction::new(
        Address::from(&keypair),
        ObjectRef::new(Address::ZERO, ObjectVersion::ZERO, Digest::ZERO),
        TransactionType::MeowModulePublish(vec![1]),
    );
    let (signed, _) = transaction.sign(&keypair);
    let digest = signed.transaction().digest();
    (signed, digest)
}

/// Build a `ChainState` at genesis and a valid mined block at height 1 that
/// includes a gas-consuming transaction and a correct reward transaction.
/// Both share the same genesis store as their base.
fn chain_and_valid_gas_block() -> (ChainState, Block) {
    let keypair = utils::test_keypair();
    let miner_address = Address::from(&keypair);
    let (parent_store, gas_coin_ref) = utils::genesis_store_with_coin(miner_address);
    let chain = ChainState::new(parent_store.clone(), 0);

    let transaction = Transaction::new(
        miner_address,
        gas_coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed, _) = transaction.sign(&keypair);
    let transactions_root = roots::compute_transactions_root(slice::from_ref(&signed));

    let work = MiningWork {
        header: BlockHeader {
            height: 1,
            parent_hash: chain.head(),
            transactions_root,
            state_root: Digest::ZERO,
            timestamp: 1,
            nonce: 0,
        },
        batch: vec![signed],
        parent_store,
        difficulty: 0,
        miner_keypair: Arc::new(keypair),
        miner_address,
        reward_address: miner_address,
    };

    let (block, _) = work.grind();

    (chain, block)
}
