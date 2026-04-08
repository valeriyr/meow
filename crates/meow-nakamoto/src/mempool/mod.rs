pub mod error;

use std::collections::{BTreeSet, VecDeque};

use meow_types::{
    digest::Digest,
    object::object_ref::ObjectRef,
    transaction::{self, SignedTransaction, input::Input, transaction_type::TransactionType},
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

    /// Returns the pending transactions in the mempool.
    pub fn pending(&self) -> impl Iterator<Item = &SignedTransaction> {
        self.pending.iter()
    }

    /// Submits a transaction after validating:
    /// 1. Signature is valid
    /// 2. Not already in the queue
    /// 3. Referenced objects match the provided store snapshot
    pub fn submit(&mut self, tx: SignedTransaction, store: &Store) -> Result<()> {
        transaction::validator::validate_signed_transaction(&tx)?;

        let digest = tx.transaction().digest();
        if self.seen.contains(&digest) {
            return Err(MempoolError::DuplicateTransaction(digest));
        }

        validate_against_store(&tx, store)?;

        self.seen.insert(digest);
        self.pending.push_back(tx);
        Ok(())
    }

    /// Drops only transactions that are no longer valid against `store`.
    /// Called after a chain reorg so that transactions whose object
    /// references still hold on the new chain head are preserved.
    pub fn retain_valid(&mut self, store: &Store) {
        let mut valid: VecDeque<SignedTransaction> = VecDeque::new();
        let mut valid_seen: BTreeSet<Digest> = BTreeSet::new();

        for tx in self.pending.drain(..) {
            if validate_against_store(&tx, store).is_ok() {
                let digest = tx.transaction().digest();
                valid_seen.insert(digest);
                valid.push_back(tx);
            }
        }

        self.pending = valid;
        self.seen = valid_seen;
    }

    /// Drains up to `limit` transactions from the front of the queue.
    pub fn drain_batch(&mut self, limit: usize) -> Vec<SignedTransaction> {
        let count = limit.min(self.pending.len());
        let batch: Vec<_> = self.pending.drain(..count).collect();
        for tx in &batch {
            self.seen.remove(&tx.transaction().digest());
        }
        batch
    }
}

/// Validates that all object references in the transaction match the latest version and digest in the store.
fn validate_against_store(tx: &SignedTransaction, store: &Store) -> Result<()> {
    let gas_coin = tx.transaction().gas_coin();
    validate_object_ref(gas_coin, store)?;

    if let TransactionType::MeowCall(call) = tx.transaction().type_() {
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
        .ok_or(MempoolError::ObjectNotFound(*object_ref.address()))?;

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
