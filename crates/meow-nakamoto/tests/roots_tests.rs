mod utils;

use std::slice;

use meow_nakamoto::{roots, store::Store};
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{SignedTransaction, Transaction, transaction_type::TransactionType},
};
use rand::{SeedableRng, rngs::StdRng};

//
// ─── compute_state_root ───
//

/// The state root is deterministic: the same store always produces the same digest.
#[test]
fn compute_state_root_is_deterministic() {
    let store = Store::with_objects([utils::test_module_object(ADDRESS1)]);

    assert_eq!(
        roots::compute_state_root(&store),
        roots::compute_state_root(&store)
    );
}

/// A store with different objects must produce a different state root.
#[test]
fn compute_state_root_changes_when_objects_change() {
    let empty = Store::default();
    let non_empty = Store::with_objects([utils::test_module_object(ADDRESS1)]);

    assert_ne!(
        roots::compute_state_root(&empty),
        roots::compute_state_root(&non_empty)
    );
}

/// The state root must not depend on insertion order — the same set of objects
/// must hash identically regardless of the order they were added to the store.
/// This is critical: if two nodes insert objects in a different order they must
/// still agree on the state root and not fork.
#[test]
fn compute_state_root_is_insertion_order_independent() {
    let store_a = Store::with_objects([
        utils::test_module_object(ADDRESS1),
        utils::test_module_object(ADDRESS2),
    ]);
    let store_b = Store::with_objects([
        utils::test_module_object(ADDRESS2),
        utils::test_module_object(ADDRESS1),
    ]);

    assert_eq!(
        roots::compute_state_root(&store_a),
        roots::compute_state_root(&store_b)
    );
}

//
// ─── compute_transactions_root ───
//

/// An empty transaction list produces a deterministic non-ZERO digest.
#[test]
fn compute_transactions_root_is_deterministic_for_empty_list() {
    let root = roots::compute_transactions_root(&[]);

    assert_ne!(root, Digest::ZERO, "empty root must not be ZERO");
    assert_eq!(root, roots::compute_transactions_root(&[]));
}

/// The same transaction always produces the same root.
#[test]
fn compute_transactions_root_is_deterministic() {
    let transaction = make_signed_transaction(1);

    assert_eq!(
        roots::compute_transactions_root(slice::from_ref(&transaction)),
        roots::compute_transactions_root(&[transaction])
    );
}

/// Different transactions must produce different roots.
#[test]
fn compute_transactions_root_differs_for_different_transactions() {
    let transaction1 = make_signed_transaction(1);
    let transaction2 = make_signed_transaction(2);

    assert_ne!(
        roots::compute_transactions_root(&[transaction1]),
        roots::compute_transactions_root(&[transaction2])
    );
}

//
// ─── Utility functions ───
//

const ADDRESS1: Address = Address::suffixed(0xF1);
const ADDRESS2: Address = Address::suffixed(0xF2);

fn make_signed_transaction(seed: u8) -> SignedTransaction {
    let keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([seed; 32]));
    let sender = Address::from(&keypair);
    let transaction = Transaction::new(
        sender,
        ObjectRef::new(Address::ZERO, ObjectVersion::ZERO, Digest::ZERO),
        TransactionType::MeowModulePublish(vec![seed]),
    );
    let (signed, _) = transaction.sign(&keypair);
    signed
}
