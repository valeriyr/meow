//! Merkle-style root hashes committed to by block headers.

use meow_types::{digest::Digest, object::Object, transaction::SignedTransaction};

use crate::store::Store;

/// Deterministic hash of the object store's current state.
/// Used as `state_root` in block headers.
pub fn compute_state_root(store: &Store) -> Digest {
    let objects: Vec<&Object> = store.objects().collect();
    Digest::compute(&objects).expect("state root serialization is infallible")
}

/// Hash over all transaction digests in order.
/// Used as `transactions_root` in block headers.
pub fn compute_transactions_root(transactions: &[SignedTransaction]) -> Digest {
    let digests: Vec<Digest> = transactions
        .iter()
        .map(|t| t.transaction().digest())
        .collect();
    Digest::compute(&digests).expect("transactions root serialization is infallible")
}
