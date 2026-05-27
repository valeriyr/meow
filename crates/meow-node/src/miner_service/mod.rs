//! Miner service: runs the PoW miner in a background task and feeds produced blocks into the chain.

pub mod error;

use std::sync::Arc;

use meow_nakamoto::miner::Miner;
use tokio::sync::{Mutex, mpsc, watch};

use crate::miner_service::error::MinerServiceError;

/// The result type related to the miner service.
pub type Result<T> = std::result::Result<T, MinerServiceError>;

/// How long to wait between mempool polls when there are no pending transactions.
const MEMPOOL_EMPTY_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);

/// The miner service, responsible for preparing and mining blocks in a loop.
pub struct MinerService {
    /// Shared miner, protected by a mutex for synchronization and interior mutability.
    miner: Arc<Mutex<Miner>>,
    /// Handle to publish mined blocks to the gossip network.
    blocks_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl MinerService {
    /// Creates a new miner service.
    pub fn new(miner: Arc<Mutex<Miner>>, blocks_tx: mpsc::UnboundedSender<Vec<u8>>) -> Self {
        Self { miner, blocks_tx }
    }

    /// Runs the mining loop, preparing and committing blocks as they are mined.
    pub async fn run(self, mut shutdown_rx: watch::Receiver<()>) -> Result<()> {
        {
            let miner = self.miner.lock().await;
            tracing::info!(
                miner_address = %miner.miner_address(),
                reward_address = %miner.reward_address(),
                "miner service started"
            );
        }

        loop {
            tokio::select! {
                // Check for shutdown signal with higher priority to allow timely shutdown.
                biased;

                changed = shutdown_rx.changed() => {
                    match changed {
                        Ok(()) => {
                            tracing::info!("miner shutdown signal received");
                            break;
                        }
                        Err(_) => {
                            tracing::warn!("miner shutdown channel closed");
                            break;
                        }
                    }
                }
                maybe_result = async {
                    let work = match self.miner.lock().await.prepare_round() {
                        Some(work) => work,
                        None => {
                            // Mempool empty — wait before polling again.
                            tokio::time::sleep(MEMPOOL_EMPTY_POLL_INTERVAL).await;
                            return None;
                        }
                    };
                    // Grind nonce without holding the lock.
                    // grind() yields every YIELD_EVERY_N_NONCES nonces so select! can
                    // cancel this future promptly on shutdown.
                    work.grind().await
                } => {
                    if let Some((block, new_store)) = maybe_result {
                        let committed = self
                            .miner
                            .lock()
                            .await
                            .commit_mined(block.clone(), new_store);
                        if committed {
                            tracing::info!(
                                height = block.header.height,
                                block_hash = %block.hash(),
                                txs = block.results.len(),
                                nonce = block.header.nonce,
                                "block mined"
                            );
                            if let Ok(data) = bcs::to_bytes(&block)
                                && let Err(e) = self.blocks_tx.send(data)
                            {
                                tracing::warn!(height = block.header.height, block_hash = %block.hash(), error = %e, "failed to publish block");
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("miner service stopped");
        Ok(())
    }
}
