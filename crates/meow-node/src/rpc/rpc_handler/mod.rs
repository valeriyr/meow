pub mod error;

use std::sync::Arc;

use meow_nakamoto::miner::Miner;
use meow_types::{
    address::Address,
    object::Object,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
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
    pub async fn submit_tx(&self, tx: SignedTransaction) -> Result<()> {
        let tx_digest = tx.transaction().digest();
        let mut miner = self.miner.lock().await;

        miner.submit_tx(tx.clone())?;
        tracing::debug!(%tx_digest, "accepted tx in local mempool");

        // Serialize for gossip; local submission has already succeeded.
        match bcs::to_bytes(&tx) {
            Ok(data) => {
                if let Err(e) = self.publish_transactions_tx.send(data) {
                    tracing::warn!(%tx_digest, "failed to publish accepted tx to gossip: {e}");
                }
            }
            Err(e) => {
                tracing::warn!(%tx_digest, "failed to serialize accepted tx for gossip: {e}");
            }
        }

        Ok(())
    }

    /// Returns the latest live object at address.
    pub async fn get_object(&self, addr: &Address) -> Result<Option<Object>> {
        let miner = self.miner.lock().await;

        Ok(miner.head_store().get_object(addr).cloned())
    }

    /// Returns the latest live objects owned by the given address.
    pub async fn get_objects(&self, owner: &Address) -> Result<Vec<Object>> {
        let miner = self.miner.lock().await;

        Ok(miner.head_store().get_objects(owner).cloned().collect())
    }

    /// Returns a committed transaction by digest.
    pub async fn get_transaction(
        &self,
        digest: &meow_types::digest::Digest,
    ) -> Result<Option<SignedTransaction>> {
        let miner = self.miner.lock().await;

        Ok(miner.get_transaction(digest).cloned())
    }

    /// Returns the execution result for a transaction digest if committed.
    pub async fn get_transaction_result(
        &self,
        digest: &meow_types::digest::Digest,
    ) -> Result<Option<ExecutionResult>> {
        let miner = self.miner.lock().await;

        Ok(miner.get_transaction_result(digest).cloned())
    }
}
