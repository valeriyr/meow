#![allow(dead_code)]

use std::sync::Arc;

use meow_genesis::Genesis;
use meow_nakamoto::{
    chain::{ChainState, error::ChainError},
    miner::mining_work::MiningWork,
    roots,
    store::Store,
};
use meow_nakamoto_types::{block::Block, block_header::BlockHeader};
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{
        Object, object_owner::ObjectOwner, object_ref::ObjectRef, object_version::ObjectVersion,
    },
    transaction::{
        SignedTransaction, Transaction, execution_result::ExecutionResult,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::builder;
use rand::{SeedableRng, rngs::StdRng};

/// Snapshot depth used across all chain tests.
pub const SNAPSHOT_DEPTH: u64 = 64;
/// Batch size used in miner tests — one transaction per block is sufficient for testing.
/// Keeps tests fast and avoids the need for multiple independent coins per test.
pub const BATCH_SIZE: usize = 1;
/// Difficulty used in standard chain tests — zero means nonce=0 is always valid.
pub const DIFFICULTY: u32 = 0;
/// Non-trivial difficulty used in PoW rejection tests.
pub const POW_DIFFICULTY: u32 = 32;

/// Create a `ChainState` with zero difficulty and the standard test snapshot depth.
pub fn new_chain(store: Store) -> ChainState {
    ChainState::new(store, DIFFICULTY, SNAPSHOT_DEPTH)
}

/// Anchor a `ChainState` at a snapshot block with zero difficulty and the standard test snapshot depth.
pub fn chain_from_snapshot(block: Block, store: Store) -> Result<ChainState, ChainError> {
    ChainState::from_snapshot(0, block, store, DIFFICULTY, SNAPSHOT_DEPTH)
}

pub fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0x01; 32]))
}

pub fn noop_module_bytes() -> Vec<u8> {
    let module = builder::build(
        r#"
            mod noop;
            
            pub fn noop() {}
        "#,
        &[],
    )
    .expect("noop module must compile");
    bcs::to_bytes(&module).expect("module serialization is infallible")
}

/// Build a genesis that pre-allocates a coin to `owner`, returning the genesis
/// and the coin's `ObjectRef`.
pub fn genesis_with_coin(owner: Address) -> (Genesis, ObjectRef) {
    let genesis = Genesis::build(&[(owner, 10_000)]).expect("genesis must build");
    let coin_ref = genesis
        .objects()
        .iter()
        .find(|o| o.owner() == &ObjectOwner::Address(owner))
        .expect("allocation must produce a coin owned by the address")
        .object_ref();
    (genesis, coin_ref)
}

/// Build a genesis that pre-allocates a coin to `owner`, returning the store
/// and the coin's `ObjectRef` ready to be used as a gas coin.
pub fn genesis_store_with_coin(owner: Address) -> (Store, ObjectRef) {
    let (genesis, coin_ref) = genesis_with_coin(owner);
    (
        Store::with_objects(genesis.objects().iter().cloned()),
        coin_ref,
    )
}

/// Build a genesis that pre-allocates one coin per entry in `n`, using
/// deterministic keypairs seeded at `[0xC0 + i; 32]`. Returns the store
/// and a vec of `(keypair_seed, coin_ref)` pairs — one per coin.
///
/// Coins are independent: each belongs to a distinct address, so applying a
/// transaction that spends `coins[i]` never invalidates `coins[j]`.
pub fn coins(n: usize) -> (Store, Vec<([u8; 32], ObjectRef)>) {
    let seeds: Vec<[u8; 32]> = (0..n).map(|i| [0xC0u8.wrapping_add(i as u8); 32]).collect();
    let keypairs: Vec<KeyPair> = seeds
        .iter()
        .map(|&s| KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed(s)))
        .collect();
    let addresses: Vec<Address> = keypairs.iter().map(Address::from).collect();
    let allocations: Vec<(Address, u64)> = addresses.iter().copied().map(|a| (a, 10_000)).collect();
    let genesis = Genesis::build(&allocations).expect("genesis must build");
    let coin_refs: Vec<ObjectRef> = addresses
        .iter()
        .map(|addr| {
            genesis
                .objects()
                .iter()
                .find(|o| o.owner() == &ObjectOwner::Address(*addr))
                .expect("allocation produces a coin")
                .object_ref()
        })
        .collect();
    let store = Store::with_objects(genesis.objects().iter().cloned());
    (store, seeds.into_iter().zip(coin_refs).collect())
}

/// Build a `MiningWork` ready to grind. `miner_keypair` is always `test_keypair()`;
/// `miner_address` is derived from it.
pub fn mining_work(
    batch: Vec<SignedTransaction>,
    parent_store: Store,
    height: u64,
    parent_hash: Digest,
    timestamp: u64,
    difficulty: u32,
    reward_address: Address,
) -> MiningWork {
    let keypair = test_keypair();
    let miner_address = Address::from(&keypair);
    let transactions_root = roots::compute_transactions_root(&batch);
    MiningWork {
        header: BlockHeader {
            height,
            parent_hash,
            transactions_root,
            reward_root: None,
            state_root: Digest::ZERO,
            timestamp,
            nonce: 0,
        },
        batch,
        parent_store,
        difficulty,
        miner_keypair: Arc::new(keypair),
        miner_address,
        reward_address,
    }
}

/// Build and sign a `MeowModulePublish` transaction using `keypair` and `coin_ref` as gas.
/// Pass distinct `content` bytes across calls to produce distinct digests so multiple
/// transactions referencing the same coin can coexist in the mempool.
pub fn make_signed_transaction(
    keypair: &KeyPair,
    coin_ref: ObjectRef,
    content: Vec<u8>,
) -> SignedTransaction {
    let (signed, _) = Transaction::new(
        Address::from(keypair),
        coin_ref,
        TransactionType::MeowModulePublish(content),
    )
    .sign(keypair);
    signed
}

/// Build a validly signed transaction for use with `commit` and structural tests (no gas coin required).
/// Returns `(signed_transaction, transaction_digest)`.
pub fn dummy_signed_transaction() -> (SignedTransaction, Digest) {
    let keypair = test_keypair();
    let signed = make_signed_transaction(
        &keypair,
        ObjectRef::new(Address::ZERO, ObjectVersion::ZERO, Digest::ZERO),
        vec![1],
    );
    let digest = signed.transaction().digest();
    (signed, digest)
}

/// Build a module `Object` at the given address with empty content, suitable for store and root tests.
pub fn test_module_object(addr: Address) -> Object {
    Object::fresh_module(addr, Digest::ZERO, vec![])
}

/// Build a structurally valid block with one signed transaction, a matching placeholder
/// failure result, and no reward. `transactions_root` is computed from the transaction;
/// `reward_root` and `nonce` are `None`/`0`. To inject an intentionally wrong field,
/// reassign it after construction (e.g. `block.header.transactions_root = Digest::ZERO`).
pub fn make_block(
    height: u64,
    parent_hash: Digest,
    timestamp: u64,
    state_root: Digest,
    signed: SignedTransaction,
) -> Block {
    make_block_with_transactions(height, parent_hash, timestamp, state_root, vec![signed])
}

/// Build a structurally valid block with an arbitrary transaction list. Each transaction
/// gets a placeholder failure result. `transactions_root` is computed from the full list.
pub fn make_block_with_transactions(
    height: u64,
    parent_hash: Digest,
    timestamp: u64,
    state_root: Digest,
    transactions: Vec<SignedTransaction>,
) -> Block {
    let results = transactions
        .iter()
        .map(|tx| ExecutionResult::failure("", tx.transaction().digest()))
        .collect();
    Block {
        header: BlockHeader {
            height,
            parent_hash,
            transactions_root: roots::compute_transactions_root(&transactions),
            reward_root: None,
            state_root,
            timestamp,
            nonce: 0,
        },
        transactions,
        results,
        reward_transaction: None,
        reward_transaction_result: None,
    }
}
