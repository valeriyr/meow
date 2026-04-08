pub mod error;

use std::{sync::Arc, time::Duration};

use meow_nakamoto::miner::Miner;
use tokio::sync::{Mutex, mpsc, watch};

use crate::miner_service::error::MinerServiceError;

/// The result type related to the miner service.
pub type Result<T> = std::result::Result<T, MinerServiceError>;

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
        tracing::info!("starting miner service");

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
                maybe_work = async {
                    match self.miner.lock().await.prepare_round() {
                        Some(work) => Some(work),
                        None => {
                            // Mempool empty — wait before polling again.
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            None
                        }
                    }
                } => {
                    if let Some(work) = maybe_work {
                        // Grind nonce without holding the lock.
                        let (block, new_store) = work.grind();

                        let committed = self
                            .miner
                            .lock()
                            .await
                            .commit_mined(block.clone(), new_store);
                        if committed {
                            tracing::info!(
                                height = block.header.height,
                                txs = block.results.len(),
                                nonce = block.header.nonce,
                                "block mined"
                            );
                            if let Ok(data) = bcs::to_bytes(&block)
                                && let Err(e) = self.blocks_tx.send(data)
                            {
                                tracing::warn!(error = %e, "failed to publish block");
                            }
                        }
                        // Yield between rounds so other tasks can run.
                        tokio::task::yield_now().await;
                    }
                }
            }
        }

        tracing::info!("miner service stopped");
        Ok(())
    }
}
