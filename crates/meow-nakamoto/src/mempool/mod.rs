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

        let gas_coin = tx.transaction().gas_coin();
        if !store.contains(gas_coin.address()) {
            return Err(MempoolError::GasCoinNotFound(*gas_coin.address()));
        }

        validate_object_ref(gas_coin, store)?;

        if let TransactionType::MeowCall(call) = tx.transaction().type_() {
            for argument in call.arguments() {
                if let Input::Object(object_ref) = argument {
                    validate_object_ref(object_ref, store)?;
                }
            }
        }

        self.seen.insert(digest);
        self.pending.push_back(tx);
        Ok(())
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
