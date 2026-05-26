mod utils;

use meow_nakamoto::{miner::mining_work::MiningWork, roots, store::Store};
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{SignedTransaction, Transaction, transaction_type::TransactionType},
};
use rand::{SeedableRng, rngs::StdRng};

//
// ─── grind ───
//

/// With zero difficulty every hash qualifies, so the nonce must stay at its
/// initial value of zero — the grind loop exits on the very first candidate.
#[test]
fn grind_with_zero_difficulty_sets_nonce_zero() {
    let mut work = make_work_with_publish_transaction();
    work.difficulty = 0;
    let (block, _) = work.grind().expect("batch has one transaction");

    assert_eq!(block.header.nonce, 0);
}

/// The grinder must produce a block whose nonce satisfies `DIFFICULTY`.
#[test]
fn grind_produces_block_with_nonce_meeting_difficulty() {
    let (block, _) = make_work_with_publish_transaction()
        .grind()
        .expect("batch has one transaction");

    assert!(block.header.meets_difficulty(DIFFICULTY));
}

/// The state root embedded in the returned block header must equal
/// `compute_state_root` applied to the returned store — they are always derived
/// from the same store snapshot, so any divergence is a bug.
#[test]
fn grind_state_root_matches_returned_store() {
    let (block, store) = make_work_with_publish_transaction()
        .grind()
        .expect("batch has one transaction");

    assert_eq!(block.header.state_root, roots::compute_state_root(&store));
}

/// The transactions root embedded in the returned block header must equal
/// `compute_transactions_root` applied to the block's own transaction list —
/// the header must always commit to the actual executed set.
#[test]
fn grind_transactions_root_matches_block_transactions() {
    let (block, _) = make_work_with_publish_transaction()
        .grind()
        .expect("batch has one transaction");

    assert_eq!(
        block.header.transactions_root,
        roots::compute_transactions_root(&block.transactions),
    );
}

/// `grind` must return `None` when the batch is empty — grinding an empty block
/// is not allowed.
#[test]
fn grind_returns_none_for_empty_batch() {
    assert!(make_work(vec![]).grind().is_none());
}

/// When all transactions in the batch fail (e.g. gas coin absent from store),
/// every transaction is dropped and `grind` must return `None` instead of
/// committing an empty block.
#[test]
fn grind_returns_none_when_all_transactions_fail() {
    let batch = vec![make_failing_transaction(0xA1)];
    assert!(make_work(batch).grind().is_none());
}

/// When a batch contains both valid and failing transactions, `grind` must drop
/// only the failing ones, re-grind with the reduced set, and commit a block whose
/// `transactions_root` reflects the actual executed transactions — not the original
/// batch.
#[test]
fn grind_drops_failed_transactions_and_updates_transactions_root() {
    let keypair = utils::test_keypair();
    let miner_address = Address::from(&keypair);
    let (parent_store, gas_coin_ref) = utils::genesis_store_with_coin(miner_address);

    let valid_tx = Transaction::new(
        miner_address,
        gas_coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (valid_signed, _) = valid_tx.sign(&keypair);

    let batch = vec![valid_signed, make_failing_transaction(0xA1)];
    let work = utils::mining_work(
        batch,
        parent_store,
        1,
        Digest::ZERO,
        0,
        DIFFICULTY,
        REWARD_ADDRESS,
    );

    let (block, _) = work.grind().expect("valid transaction survives");

    assert_eq!(block.transactions.len(), 1);
    assert_eq!(
        block.header.transactions_root,
        roots::compute_transactions_root(&block.transactions),
        "transactions_root must be recomputed after dropping failed transactions"
    );
}

/// `results` must be in 1-to-1 correspondence with `transactions`.
/// The successful transaction must appear in both with matching digest.
/// Because the transaction spends gas, `reward_transaction` and
/// `reward_transaction_result` must both be present.
#[test]
fn grind_results_count_matches_transactions_count() {
    let (block, _) = make_work_with_publish_transaction()
        .grind()
        .expect("batch has one transaction");

    assert_eq!(block.transactions.len(), 1);
    assert_eq!(block.results.len(), 1);
    assert_eq!(
        block.results[0].transaction_digest(),
        &block.transactions[0].transaction().digest(),
    );
    assert!(block.reward_transaction.is_some());
    assert!(block.reward_transaction_result.is_some());
}

//
// ─── Utility functions ───
//

const DIFFICULTY: u32 = 4;
const REWARD_ADDRESS: Address = Address::suffixed(0xE1);

fn make_work(batch: Vec<SignedTransaction>) -> MiningWork {
    utils::mining_work(
        batch,
        Store::default(),
        1,
        Digest::ZERO,
        0,
        DIFFICULTY,
        REWARD_ADDRESS,
    )
}

/// Build `MiningWork` containing one valid `MeowModulePublish` transaction that
/// will execute successfully and produce gas, using a genesis store pre-seeded
/// with a coin for the miner.
fn make_work_with_publish_transaction() -> MiningWork {
    let keypair = utils::test_keypair();
    let miner_address = Address::from(&keypair);
    let (parent_store, gas_coin_ref) = utils::genesis_store_with_coin(miner_address);
    let transaction = Transaction::new(
        miner_address,
        gas_coin_ref,
        TransactionType::MeowModulePublish(utils::noop_module_bytes()),
    );
    let (signed_transaction, _) = transaction.sign(&keypair);
    utils::mining_work(
        vec![signed_transaction],
        parent_store,
        1,
        Digest::ZERO,
        0,
        DIFFICULTY,
        REWARD_ADDRESS,
    )
}

/// Build a signed transaction that always causes `executor::execute` to return
/// `Err`: the gas-coin address (`0xF9`) is never present in `Store::default()`,
/// so `resolve_gas_coin_object` fails before any execution happens.
fn make_failing_transaction(seed: u8) -> SignedTransaction {
    let keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([seed; 32]));
    let sender = Address::from(&keypair);
    let transaction = Transaction::new(
        sender,
        ObjectRef::new(Address::suffixed(0xF9), ObjectVersion::ZERO, Digest::ZERO),
        TransactionType::MeowModulePublish(vec![1]),
    );
    let (signed, _) = transaction.sign(&keypair);
    signed
}
