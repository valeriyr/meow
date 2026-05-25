mod utils;

use std::{slice, sync::Arc};

use meow_genesis::Genesis;
use meow_nakamoto::{
    miner::{Miner, mining_work::MiningWork},
    roots,
};
use meow_nakamoto_types::{block::Block, block_header::BlockHeader, miner_config::MinerConfig};
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{object_owner::ObjectOwner, object_ref::ObjectRef},
    transaction::{Transaction, transaction_type::TransactionType},
};
use rand::{SeedableRng, rngs::StdRng};

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
/// hash still matches the current head at commit time.
#[test]
fn commit_mined_advances_chain_when_head_matches() {
    let mut miner = Miner::empty(test_config());

    let (block, new_store) = empty_work(&miner, 1).grind();
    let block_hash = block.hash();

    assert!(miner.commit_mined(block, new_store));

    assert_eq!(miner.head(), block_hash);
    assert_eq!(miner.head_height(), 1);
}

/// `commit_mined` must discard the block and return `false` when the chain head
/// advanced between `prepare_round` and the commit (e.g. a peer block arrived).
#[test]
fn commit_mined_discards_stale_block() {
    let mut miner = Miner::empty(test_config());

    // Grind two height-1 blocks from the same genesis parent (different timestamps
    // produce different hashes so they can coexist in the chain).
    let (block_a, store_a) = empty_work(&miner, 1).grind();
    let (block_b, store_b) = empty_work(&miner, 2).grind();
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
    let work = MiningWork {
        header: BlockHeader {
            height: 1,
            parent_hash: Block::genesis().hash(),
            transactions_root: roots::compute_transactions_root(slice::from_ref(&signed_peer)),
            state_root: Digest::ZERO,
            timestamp: 1,
            nonce: 0,
        },
        batch: vec![signed_peer],
        parent_store,
        difficulty: 0,
        miner_keypair: Arc::new(utils::test_keypair()),
        miner_address,
        reward_address: miner_address,
    };
    let (peer_block, _) = work.grind();
    let peer_block_hash = peer_block.hash();

    // Apply the peer block — head advances, retain_valid is called on the mempool.
    assert!(miner.apply_block(peer_block));
    assert_eq!(miner.head(), peer_block_hash);
    assert_eq!(miner.head_height(), 1);

    // The mempool tx (referencing coin v1) must have been pruned — the coin is now v2.
    assert!(miner.prepare_round().is_none());
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

/// Build an empty `MiningWork` at height 1 with the given timestamp from the
/// miner's current head. The batch is empty so no gas is produced and the
/// keypair / reward address fields are unused by `grind`.
fn empty_work(miner: &Miner, timestamp: u64) -> MiningWork {
    let dummy_keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0xFF; 32]));
    let parent_store = miner.head_store().clone();
    MiningWork {
        header: BlockHeader {
            height: 1,
            parent_hash: Block::genesis().hash(),
            transactions_root: roots::compute_transactions_root(&[]),
            state_root: Digest::ZERO,
            timestamp,
            nonce: 0,
        },
        batch: vec![],
        parent_store,
        difficulty: DIFFICULTY,
        miner_keypair: Arc::new(dummy_keypair),
        miner_address: miner.miner_address(),
        reward_address: miner.reward_address(),
    }
}
