//! HTTP request handlers for each RPC endpoint.

pub mod error;

use std::sync::Arc;

use meow_nakamoto::miner::Miner;
use meow_nakamoto_types::{block::Block, state_snapshot::StateSnapshot};
use meow_types::{
    address::Address,
    digest::Digest,
    object::Object,
    transaction::{SignedTransaction, Transaction, execution_result::ExecutionResult},
};
use tokio::sync::{Mutex, mpsc};

use crate::rpc::rpc_handler::error::RpcHandlerError;

/// The result type related to the RPC handler.
pub type Result<T> = std::result::Result<T, RpcHandlerError>;

/// Business-logic service used by RPC handlers.
#[derive(Clone)]
pub struct RpcHandler {
    /// Shared miner, protected by a mutex for synchronization and interior mutability.
    miner: Arc<Mutex<Miner>>,
    /// Handle to publish accepted transactions to the gossip network.
    publish_transactions_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl RpcHandler {
    /// Creates a new RPC business-logic handler.
    pub fn new(
        miner: Arc<Mutex<Miner>>,
        publish_transactions_tx: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        Self {
            miner,
            publish_transactions_tx,
        }
    }

    /// Submits a transaction to local mempool and broadcasts it to gossip.
    pub async fn submit_transaction(&self, signed_transaction: SignedTransaction) -> Result<()> {
        let digest = signed_transaction.transaction().digest();
        let mut miner = self.miner.lock().await;

        if let Err(e) = miner.submit_transaction(signed_transaction.clone()) {
            tracing::warn!(%digest, error = %e, "RPC transaction rejected");
            return Err(e.into());
        }
        tracing::debug!(%digest, "RPC transaction accepted into mempool");

        // Serialize for gossip; local submission has already succeeded.
        match bcs::to_bytes(&signed_transaction) {
            Ok(data) => {
                if let Err(e) = self.publish_transactions_tx.send(data) {
                    tracing::warn!(%digest, error = %e, "failed to publish transaction to gossip");
                }
            }
            Err(e) => {
                tracing::warn!(%digest, error = %e, "failed to serialize transaction for gossip");
            }
        }

        Ok(())
    }

    /// Simulates a transaction locally without committing it.
    pub async fn simulate_transaction(&self, transaction: Transaction) -> Result<ExecutionResult> {
        let digest = transaction.digest();
        let mut miner = self.miner.lock().await;

        let result = miner.simulate_transaction(transaction)?;
        tracing::debug!(%digest, "simulated transaction");

        Ok(result)
    }

    /// Returns the latest live object at address.
    pub async fn get_object(&self, address: &Address) -> Option<Object> {
        let miner = self.miner.lock().await;

        miner.head_store().get_object(address).cloned()
    }

    /// Returns live objects for each address in the list, preserving order.
    /// Each entry is `None` if no live object exists at that address.
    pub async fn get_objects(&self, addresses: &[Address]) -> Vec<Option<Object>> {
        let miner = self.miner.lock().await;

        addresses
            .iter()
            .map(|addr| miner.head_store().get_object(addr).cloned())
            .collect()
    }

    /// Returns the latest live objects owned by the given address.
    pub async fn get_objects_owned(&self, owner: &Address) -> Vec<Object> {
        let miner = self.miner.lock().await;

        miner.head_store().get_objects(owner).cloned().collect()
    }

    /// Returns a committed transaction by digest.
    pub async fn get_transaction(&self, digest: &Digest) -> Option<SignedTransaction> {
        let miner = self.miner.lock().await;

        miner.get_transaction(digest).cloned()
    }

    /// Returns the execution result for a transaction digest if committed.
    pub async fn get_transaction_result(&self, digest: &Digest) -> Option<ExecutionResult> {
        let miner = self.miner.lock().await;

        miner.get_transaction_result(digest).cloned()
    }

    /// Returns a block by hash.
    pub async fn get_block(&self, digest: &Digest) -> Option<Block> {
        let miner = self.miner.lock().await;
        miner.get_block(digest).cloned()
    }

    /// Returns a full state snapshot at the given block hash.
    pub async fn get_block_snapshot(&self, digest: &Digest) -> Option<StateSnapshot> {
        let miner = self.miner.lock().await;
        miner.get_block_snapshot(digest)
    }

    /// Returns the digest of the current chain head.
    pub async fn get_chain_head(&self) -> Digest {
        let miner = self.miner.lock().await;
        miner.head()
    }

    /// Returns all blocks from the given height onwards (for chain synchronization).
    pub async fn get_blocks_since(&self, height: u64) -> Vec<Block> {
        let miner = self.miner.lock().await;

        miner.get_blocks_since(height)
    }

    /// Returns a full state snapshot at the current head.
    pub async fn get_state_snapshot(&self) -> StateSnapshot {
        let miner = self.miner.lock().await;

        miner.get_state_snapshot()
    }
}
