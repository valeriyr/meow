mod utils;

use meow_nakamoto::{
    mempool::{MAX_MEMPOOL_SIZE, Mempool, error::MempoolError},
    store::Store,
};
use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_ref::ObjectRef,
        object_type::ObjectType, object_version::ObjectVersion,
    },
    transaction::{Transaction, call::Call, input::Input, transaction_type::TransactionType},
};
use rand::{SeedableRng, rngs::StdRng};

//
// ─── submit ───
//

/// A valid signed transaction referencing an existing coin must be accepted.
#[test]
fn submit_accepts_valid_transaction() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let store = Store::with_objects([gas_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &gas_coin).unwrap();

    assert_eq!(mempool.pending().count(), 1);
}

/// Submitting the same transaction digest a second time must return a duplicate error.
#[test]
fn submit_rejects_duplicate_transaction() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let store = Store::with_objects([gas_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &gas_coin).unwrap();

    let err = submit_transaction(&mut mempool, &store, &keypair, &gas_coin).unwrap_err();

    assert!(matches!(err, MempoolError::DuplicateTransaction { .. }));
}

/// Once the pool holds `MAX_MEMPOOL_SIZE` transactions the next submission must be rejected
/// with `MempoolFull`, regardless of how valid the transaction itself is.
#[test]
fn submit_rejects_when_mempool_is_full() {
    let keypair = utils::test_keypair();

    // Build MAX_MEMPOOL_SIZE + 1 unique coins so every submission has a distinct digest.
    let coins: Vec<Object> = (0..=MAX_MEMPOOL_SIZE as u16)
        .map(|i| make_object(Address::suffixed(i), ObjectVersion::ONE))
        .collect();
    let store = Store::with_objects(coins.clone());

    let mut mempool = Mempool::empty();

    for coin in &coins[..MAX_MEMPOOL_SIZE] {
        submit_transaction(&mut mempool, &store, &keypair, coin).unwrap();
    }
    assert_eq!(mempool.len(), MAX_MEMPOOL_SIZE);

    let err =
        submit_transaction(&mut mempool, &store, &keypair, &coins[MAX_MEMPOOL_SIZE]).unwrap_err();
    assert!(matches!(
        err,
        MempoolError::MempoolFull { capacity } if capacity == MAX_MEMPOOL_SIZE
    ));
}

/// A `MeowCall` transaction whose object argument is absent from the store must
/// be rejected — `validate_against_store` checks call args as well as the gas coin.
#[test]
fn submit_rejects_meow_call_with_missing_object_argument() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let arg_obj = make_object(Address::suffixed(0xF1), ObjectVersion::ONE);
    // Store only has the gas coin — the call argument object is absent.
    let store = Store::with_objects([gas_coin.clone()]);

    let transaction = Transaction::new(
        keypair.public().into(),
        gas_coin.object_ref(),
        TransactionType::MeowCall(Call::new(
            Address::suffixed(0xFD),
            Identifier::new("fn").unwrap(),
            vec![Input::Object(arg_obj.object_ref())],
        )),
    );
    let (signed, _) = transaction.sign(&keypair);

    let mut mempool = Mempool::empty();
    let err = mempool.submit(signed, &store).unwrap_err();

    assert!(matches!(err, MempoolError::ObjectNotFound { .. }));
}

/// A transaction whose gas coin address does not exist in the store must be rejected.
#[test]
fn submit_rejects_transaction_for_missing_gas_coin() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();

    let mut mempool = Mempool::empty();
    let err = submit_transaction(&mut mempool, &Store::default(), &keypair, &gas_coin).unwrap_err();

    assert!(matches!(err, MempoolError::ObjectNotFound { .. }));
}

/// A transaction signed by a key that does not match the declared sender must be rejected.
#[test]
fn submit_rejects_transaction_with_invalid_signature() {
    let sender_keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([1u8; 32]));
    let signer_keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([2u8; 32]));
    let sender: Address = sender_keypair.public().into();

    let gas_coin = make_gas_coin();
    let store = Store::with_objects([gas_coin.clone()]);

    let transaction = Transaction::new(
        sender,
        gas_coin.object_ref(),
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    // Sign with the wrong keypair — signature will not match `sender`.
    let (signed, _) = transaction.sign(&signer_keypair);

    let mut mempool = Mempool::empty();
    let err = mempool.submit(signed, &store).unwrap_err();

    assert!(matches!(err, MempoolError::TransactionValidationError(_)));
}

/// A transaction referencing a gas coin with the wrong digest must be rejected.
#[test]
fn submit_rejects_transaction_with_mismatched_gas_coin_digest() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let store = Store::with_objects([gas_coin.clone()]);

    // gas_coin.digest() is Digest::ZERO; use a different digest in the ObjectRef.
    let wrong_ref = ObjectRef::new(
        *gas_coin.address(),
        *gas_coin.version(),
        Digest::from([0xFF; 32]),
    );
    let transaction = Transaction::new(
        keypair.public().into(),
        wrong_ref,
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );
    let (signed, _) = transaction.sign(&keypair);

    let mut mempool = Mempool::empty();
    let err = mempool.submit(signed, &store).unwrap_err();

    assert!(matches!(err, MempoolError::InvalidObjectDigest { .. }));
}

/// A transaction referencing a gas coin at a stale version must be rejected.
#[test]
fn submit_rejects_transaction_with_stale_gas_coin_version() {
    let keypair = utils::test_keypair();
    let coin_v1 = make_gas_coin();
    let coin_v2 = make_object(*coin_v1.address(), coin_v1.version().next().unwrap());
    let store_v2 = Store::with_objects([coin_v2]);

    let mut mempool = Mempool::empty();
    // store has v2, transaction references v1
    let err = submit_transaction(&mut mempool, &store_v2, &keypair, &coin_v1).unwrap_err();

    assert!(matches!(err, MempoolError::InvalidObjectVersion { .. }));
}

//
// ─── drain_batch ───
//

/// `drain_batch` with a limit larger than the pool must return all pending transactions.
#[test]
fn drain_batch_returns_all_when_limit_exceeds_pool_size() {
    let keypair = utils::test_keypair();
    let coin_a = make_object(Address::suffixed(0xF1), ObjectVersion::ONE);
    let coin_b = make_object(Address::suffixed(0xF2), ObjectVersion::ONE);
    let store = Store::with_objects([coin_a.clone(), coin_b.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &coin_a).unwrap();
    submit_transaction(&mut mempool, &store, &keypair, &coin_b).unwrap();

    let batch = mempool.drain_batch(10);
    assert_eq!(batch.len(), 2);
    assert_eq!(mempool.pending().count(), 0);
}

/// `drain_batch` must honour its limit and leave the remainder in the pool.
#[test]
fn drain_batch_respects_limit_and_leaves_remainder() {
    let keypair = utils::test_keypair();
    let coin_a = make_object(Address::suffixed(0xF1), ObjectVersion::ONE);
    let coin_b = make_object(Address::suffixed(0xF2), ObjectVersion::ONE);
    let coin_c = make_object(Address::suffixed(0xF3), ObjectVersion::ONE);
    let store = Store::with_objects([coin_a.clone(), coin_b.clone(), coin_c.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &coin_a).unwrap();
    submit_transaction(&mut mempool, &store, &keypair, &coin_b).unwrap();
    submit_transaction(&mut mempool, &store, &keypair, &coin_c).unwrap();

    let batch = mempool.drain_batch(2);
    assert_eq!(batch.len(), 2);
    assert_eq!(mempool.pending().count(), 1);
}

/// `drain_batch` must return transactions in the same order they were submitted (FIFO).
#[test]
fn drain_batch_preserves_fifo_order() {
    let keypair = utils::test_keypair();
    let coin_a = make_object(Address::suffixed(0xF1), ObjectVersion::ONE);
    let coin_b = make_object(Address::suffixed(0xF2), ObjectVersion::ONE);
    let coin_c = make_object(Address::suffixed(0xF3), ObjectVersion::ONE);
    let store = Store::with_objects([coin_a.clone(), coin_b.clone(), coin_c.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &coin_a).unwrap();
    submit_transaction(&mut mempool, &store, &keypair, &coin_b).unwrap();
    submit_transaction(&mut mempool, &store, &keypair, &coin_c).unwrap();

    let batch = mempool.drain_batch(3);
    assert_eq!(
        *batch[0].transaction().gas_coin().address(),
        *coin_a.address()
    );
    assert_eq!(
        *batch[1].transaction().gas_coin().address(),
        *coin_b.address()
    );
    assert_eq!(
        *batch[2].transaction().gas_coin().address(),
        *coin_c.address()
    );
}

/// After a transaction is drained its digest is removed from the seen set, so
/// the same transaction can be re-submitted — drained ≠ permanently rejected.
#[test]
fn drain_batch_allows_resubmission_after_drain() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let store = Store::with_objects([gas_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &gas_coin).unwrap();

    let batch = mempool.drain_batch(1);
    assert_eq!(batch.len(), 1);

    // Re-submitting the identical transaction must succeed.
    mempool
        .submit(batch.into_iter().next().unwrap(), &store)
        .unwrap();
    assert_eq!(mempool.pending().count(), 1);
}

//
// ─── retain_valid ───
//

/// A transaction whose gas coin still exists in the store with the same
/// version and digest must be kept after `retain_valid`.
#[test]
fn retain_valid_keeps_transaction_with_unchanged_object() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let store = Store::with_objects([gas_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &gas_coin).unwrap();

    // Store unchanged — transaction must survive retain_valid.
    mempool.retain_valid(&store);

    let surviving: Vec<_> = mempool.pending().collect();
    assert_eq!(surviving.len(), 1);
    assert_eq!(
        *surviving[0].transaction().gas_coin().address(),
        *gas_coin.address()
    );
}

/// A transaction whose gas coin no longer exists in the store must be dropped.
#[test]
fn retain_valid_drops_transaction_for_missing_object() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let store = Store::with_objects([gas_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &gas_coin).unwrap();

    // Empty store simulates the coin being consumed on a reorged branch.
    mempool.retain_valid(&Store::default());

    assert_eq!(mempool.pending().count(), 0);
}

/// A transaction whose gas coin exists but has a different version (it was
/// mutated on the reorged branch) must be dropped.
#[test]
fn retain_valid_drops_transaction_for_stale_object_version() {
    let keypair = utils::test_keypair();
    let coin_v1 = make_gas_coin();
    let store_v1 = Store::with_objects([coin_v1.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store_v1, &keypair, &coin_v1).unwrap();

    // After reorg the coin is at version 2 — reference in the transaction is now stale.
    let coin_v2 = make_object(*coin_v1.address(), ObjectVersion::ONE.next().unwrap());
    let store_v2 = Store::with_objects([coin_v2]);

    mempool.retain_valid(&store_v2);

    assert_eq!(mempool.pending().count(), 0);
}

/// A transaction whose gas coin has the correct version but a different digest
/// must be dropped by `retain_valid` — the coin's content changed.
#[test]
fn retain_valid_drops_transaction_for_mismatched_gas_coin_digest() {
    let keypair = utils::test_keypair();
    let gas_coin = make_gas_coin();
    let gas_coin_addr = *gas_coin.address();
    let store = Store::with_objects([gas_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &gas_coin).unwrap();

    // Same address, version, and type but a different transaction digest — content was mutated.
    let coin_new_digest = Object::new(
        gas_coin_addr,
        ObjectOwner::Address(gas_coin_addr),
        Digest::from([0xFF; 32]),
        ObjectVersion::ONE,
        ObjectType::Object(coin_decl_ref()),
        vec![],
    );
    mempool.retain_valid(&Store::with_objects([coin_new_digest]));

    assert_eq!(mempool.pending().count(), 0);
}

/// With two transactions in the pool, only the one whose coin is stale is dropped;
/// the surviving transaction must reference the good coin specifically.
#[test]
fn retain_valid_is_selective() {
    let keypair = utils::test_keypair();
    let good_addr = Address::suffixed(0xF1);
    let stale_addr = Address::suffixed(0xF2);

    let good_coin = make_object(good_addr, ObjectVersion::ONE);
    let stale_coin = make_object(stale_addr, ObjectVersion::ONE);
    let store = Store::with_objects([good_coin.clone(), stale_coin.clone()]);

    let mut mempool = Mempool::empty();
    submit_transaction(&mut mempool, &store, &keypair, &good_coin).unwrap();
    submit_transaction(&mut mempool, &store, &keypair, &stale_coin).unwrap();

    // New store has good_coin unchanged but stale_coin at a bumped version.
    let stale_coin_v2 = make_object(stale_addr, ObjectVersion::ONE.next().unwrap());
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

fn make_object(addr: Address, version: ObjectVersion) -> Object {
    Object::new(
        addr,
        ObjectOwner::Address(addr),
        Digest::ZERO,
        version,
        ObjectType::Object(coin_decl_ref()),
        vec![],
    )
}

fn make_gas_coin() -> Object {
    make_object(Address::suffixed(0xF9), ObjectVersion::ONE)
}

fn coin_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(Address::suffixed(0xFD), Identifier::new("Coin").unwrap())
}

fn submit_transaction(
    mempool: &mut Mempool,
    store: &Store,
    keypair: &KeyPair,
    gas_coin: &Object,
) -> Result<(), MempoolError> {
    mempool.submit(
        utils::make_signed_transaction(keypair, gas_coin.object_ref(), vec![1, 2, 3]),
        store,
    )
}
