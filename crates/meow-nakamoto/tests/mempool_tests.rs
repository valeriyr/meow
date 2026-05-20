use meow_nakamoto::{mempool::Mempool, store::Store};
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{
        Object, object_owner::ObjectOwner, object_type::ObjectType, object_version::ObjectVersion,
    },
    transaction::{Transaction, transaction_type::TransactionType},
};
use rand::{SeedableRng, rngs::StdRng};

//
// ─── retain_valid ───
//

/// A transaction whose gas coin still exists in the store with the same
/// version and digest must be kept after `retain_valid`.
#[test]
fn retain_valid_keeps_transaction_with_unchanged_object() {
    let keypair = test_keypair();
    let coin = make_module_object(Address::fill(0xFD), ObjectVersion::ONE);
    let store = Store::with_objects([coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_publish_tx(&mut mempool, &keypair, &coin, &store);

    // Store unchanged — transaction must survive retain_valid.
    mempool.retain_valid(&store);

    let surviving: Vec<_> = mempool.pending().collect();
    assert_eq!(surviving.len(), 1);
    assert_eq!(
        *surviving[0].transaction().gas_coin().address(),
        *coin.address()
    );
}

/// A transaction whose gas coin no longer exists in the store must be dropped.
#[test]
fn retain_valid_drops_transaction_for_missing_object() {
    let keypair = test_keypair();
    let coin = make_module_object(Address::fill(0xFD), ObjectVersion::ONE);
    let store = Store::with_objects([coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_publish_tx(&mut mempool, &keypair, &coin, &store);

    // Empty store simulates the coin being consumed on a reorged branch.
    mempool.retain_valid(&Store::default());

    assert_eq!(mempool.pending().count(), 0);
}

/// A transaction whose gas coin exists but has a different version (it was
/// mutated on the reorged branch) must be dropped.
#[test]
fn retain_valid_drops_transaction_for_stale_object_version() {
    let keypair = test_keypair();
    let coin_addr = Address::fill(0xFD);
    let coin_v1 = make_module_object(coin_addr, ObjectVersion::ONE);
    let store_v1 = Store::with_objects([coin_v1.clone()]);

    let mut mempool = Mempool::empty();
    submit_publish_tx(&mut mempool, &keypair, &coin_v1, &store_v1);

    // After reorg the coin is at version 2 — reference in the tx is now stale.
    let coin_v2 = make_module_object(coin_addr, ObjectVersion::ONE.next().unwrap());
    let store_v2 = Store::with_objects([coin_v2]);

    mempool.retain_valid(&store_v2);

    assert_eq!(mempool.pending().count(), 0);
}

/// With two transactions in the pool, only the one whose coin is stale is dropped;
/// the surviving transaction must reference the good coin specifically.
#[test]
fn retain_valid_is_selective() {
    let keypair = test_keypair();
    let good_addr = Address::fill(0xFA);
    let stale_addr = Address::fill(0xFB);

    let good_coin = make_module_object(good_addr, ObjectVersion::ONE);
    let stale_coin = make_module_object(stale_addr, ObjectVersion::ONE);
    let store = Store::with_objects([good_coin.clone(), stale_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_publish_tx(&mut mempool, &keypair, &good_coin, &store);
    submit_publish_tx(&mut mempool, &keypair, &stale_coin, &store);

    // New store has good_coin unchanged but stale_coin at a bumped version.
    let stale_coin_v2 = make_module_object(stale_addr, ObjectVersion::ONE.next().unwrap());
    let new_store = Store::with_objects([good_coin.clone(), stale_coin_v2]);
    mempool.retain_valid(&new_store);

    let surviving: Vec<_> = mempool.pending().collect();
    assert_eq!(surviving.len(), 1);
    assert_eq!(
        *surviving[0].transaction().gas_coin().address(),
        good_addr,
        "only the transaction referencing the good coin should survive"
    );
}

//
// ─── Utility functions ───
//

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([1u8; 32]))
}

fn make_module_object(addr: Address, version: ObjectVersion) -> Object {
    Object::new(
        addr,
        ObjectOwner::Immutable,
        Digest::ZERO,
        version,
        ObjectType::Module,
        vec![],
    )
}

fn submit_publish_tx(mempool: &mut Mempool, keypair: &KeyPair, gas_coin: &Object, store: &Store) {
    let sender: Address = keypair.public().into();
    let tx = Transaction::new(
        sender,
        gas_coin.object_ref(),
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    let (signed, _) = tx.sign(keypair);
    mempool.submit(signed, store).unwrap();
}
