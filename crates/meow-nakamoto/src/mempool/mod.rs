pub mod error;

use std::collections::{BTreeSet, VecDeque};

use meow_types::{digest::Digest, transaction::SignedTransaction};

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
    /// 3. Gas coin exists in the provided store snapshot
    pub fn submit(&mut self, tx: SignedTransaction, store: &Store) -> Result<()> {
        tx.verify().map_err(|_| MempoolError::InvalidSignature)?;

        let digest = tx.transaction().digest();
        if self.seen.contains(&digest) {
            return Err(MempoolError::DuplicateTransaction(digest));
        }

        let gas_coin = tx.transaction().gas_coin();
        if !store.contains(gas_coin) {
            return Err(MempoolError::GasCoinNotFound(*gas_coin));
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
