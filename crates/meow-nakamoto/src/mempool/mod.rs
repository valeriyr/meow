//! Pending transaction pool that feeds the miner; evicts spent or invalid transactions on reorg.

pub mod error;

use std::collections::{BTreeSet, VecDeque};

use meow_types::{
    digest::Digest,
    object::object_ref::ObjectRef,
    transaction::{
        SignedTransaction, Transaction, input::Input, transaction_type::TransactionType, validator,
    },
};

use crate::{mempool::error::MempoolError, store::Store};

/// The result type related to the mempool.
pub type Result<T> = std::result::Result<T, MempoolError>;

/// Pending transaction pool.
///
/// Transactions are validated on submission and drained in FIFO order
/// by the miner.
pub struct Mempool {
    /// Queue of pending transactions.
    pending: VecDeque<SignedTransaction>,
    /// Digests of transactions currently in the queue — used for dedup.
    seen: BTreeSet<Digest>,
}

impl Mempool {
    /// Creates an empty mempool.
    pub fn empty() -> Self {
        Self {
            pending: VecDeque::new(),
            seen: BTreeSet::new(),
        }
    }

    /// Returns `true` if the mempool is empty.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Returns the number of pending transactions in the mempool.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Returns the pending transactions in the mempool.
    pub fn pending(&self) -> impl Iterator<Item = &SignedTransaction> {
        self.pending.iter()
    }

    /// Submits a transaction after validating:
    /// 1. Signature is valid.
    /// 2. Not already in the queue.
    /// 3. Referenced objects match the provided store snapshot.
    pub fn submit(&mut self, signed_transaction: SignedTransaction, store: &Store) -> Result<()> {
        validator::validate_signed_transaction(&signed_transaction)?;

        let digest = signed_transaction.transaction().digest();

        if self.seen.contains(&digest) {
            return Err(MempoolError::DuplicateTransaction { digest });
        }

        validate_against_store(signed_transaction.transaction(), store)?;

        self.seen.insert(digest);
        self.pending.push_back(signed_transaction);
        Ok(())
    }

    /// Drops only transactions that are no longer valid against `store`.
    /// Called after a chain reorg so that transactions whose object
    /// references still hold on the new chain head are preserved.
    pub fn retain_valid(&mut self, store: &Store) {
        let mut valid: VecDeque<SignedTransaction> = VecDeque::new();
        let mut valid_seen: BTreeSet<Digest> = BTreeSet::new();

        for signed_transaction in self.pending.drain(..) {
            let transaction = signed_transaction.transaction();

            if validate_against_store(transaction, store).is_ok() {
                let digest = transaction.digest();

                valid_seen.insert(digest);
                valid.push_back(signed_transaction);
            }
        }

        self.pending = valid;
        self.seen = valid_seen;
    }

    /// Drains up to `limit` transactions from the front of the queue.
    /// Drained digests are removed from the seen set, so the same transaction
    /// may be re-submitted after it has been drained.
    pub fn drain_batch(&mut self, limit: usize) -> Vec<SignedTransaction> {
        let count = limit.min(self.pending.len());
        let batch: Vec<_> = self.pending.drain(..count).collect();
        for signed_transaction in &batch {
            let digest = signed_transaction.transaction().digest();
            self.seen.remove(&digest);
        }
        batch
    }
}

/// Validates that all object references in the transaction match the latest version and digest in the store.
pub(crate) fn validate_against_store(transaction: &Transaction, store: &Store) -> Result<()> {
    validate_object_ref(transaction.gas_coin(), store)?;

    if let TransactionType::MeowCall(call) = transaction.type_() {
        for argument in call.arguments() {
            if let Input::Object(object_ref) = argument {
                validate_object_ref(object_ref, store)?;
            }
        }
    }

    Ok(())
}

/// Validates that an object reference matches the latest version and digest in the store.
fn validate_object_ref(object_ref: &ObjectRef, store: &Store) -> Result<()> {
    let object = store
        .get_object(object_ref.address())
        .ok_or(MempoolError::ObjectNotFound {
            address: *object_ref.address(),
        })?;

    if object.version() != object_ref.version() {
        return Err(MempoolError::InvalidObjectVersion {
            address: *object_ref.address(),
            expected: *object_ref.version(),
            found: *object.version(),
        });
    }

    let found_digest = object.digest();
    if &found_digest != object_ref.digest() {
        return Err(MempoolError::InvalidObjectDigest {
            address: *object_ref.address(),
            expected: *object_ref.digest(),
            found: found_digest,
        });
    }

    Ok(())
}
