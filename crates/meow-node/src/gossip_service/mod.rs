pub mod error;

use std::sync::Arc;

use meow_gossip_network::GossipNetwork;
use meow_gossip_types::{config::GossipNetworkConfig, event::NetworkEvent};
use meow_nakamoto::{block::Block, miner::Miner};
use meow_types::transaction::SignedTransaction;
use tokio::sync::{Mutex, mpsc, watch};

use crate::gossip_service::error::GossipServiceError;

/// The result type related to the gossip service.
pub type Result<T> = std::result::Result<T, GossipServiceError>;

const TOPIC_TXS: &str = "txns";
const TOPIC_BLOCKS: &str = "blocks";

/// The gossip service, responsible for handling network events and interacting with the miner.
pub struct GossipService {
    /// Shared miner, protected by a mutex for synchronization and interior mutability.
    miner: Arc<Mutex<Miner>>,
    /// Handle to receive transactions to be published to the gossip network.
    transactions_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    /// Handle to receive mined blocks to be published to the gossip network.
    blocks_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl GossipService {
    /// Creates a new gossip service.
    pub fn new(
        miner: Arc<Mutex<Miner>>,
        transactions_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        blocks_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Self {
        Self {
            miner,
            transactions_rx,
            blocks_rx,
        }
    }

    /// Runs the gossip loop, handling network events and interacting with the miner.
    pub async fn run(
        mut self,
        config: GossipNetworkConfig,
        mut shutdown_rx: watch::Receiver<()>,
    ) -> Result<()> {
        tracing::info!("starting gossip service");

        let mut gossip: GossipNetwork = GossipNetwork::new(config).await?;
        gossip.subscribe(TOPIC_TXS)?;
        gossip.subscribe(TOPIC_BLOCKS)?;
        tracing::info!(topics = "txns,blocks", "gossip subscriptions ready");

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    match changed {
                        Ok(()) => {
                            tracing::info!("gossip shutdown signal received");
                            break;
                        }
                        Err(_) => {
                            tracing::warn!("gossip shutdown channel closed");
                            break;
                        }
                    }
                }
                event = gossip.next_event() => {
                    match event {
                        Some(message) => {
                            match message {
                                NetworkEvent::Message { topic, data, .. } if topic == TOPIC_TXS => {
                                    match bcs::from_bytes::<SignedTransaction>(&data) {
                                        Ok(tx) => {
                                            if let Err(e) = self.miner.lock().await.submit_tx(tx) {
                                                tracing::debug!("incoming tx rejected: {e}");
                                            }
                                        }
                                        Err(e) => tracing::debug!("gossip: failed to decode tx: {e}"),
                                    }
                                }
                                NetworkEvent::Message { topic, data, .. } if topic == TOPIC_BLOCKS => {
                                    match bcs::from_bytes::<Block>(&data) {
                                        Ok(block) => {
                                            let switched = self.miner.lock().await.on_block_received(block);
                                            if switched {
                                                tracing::info!("reorged to peer's longer chain");
                                            }
                                        }
                                        Err(e) => tracing::debug!("gossip: failed to decode block: {e}"),
                                    }
                                }
                                NetworkEvent::PeerConnected(peer) => {
                                    tracing::info!(%peer, "peer connected");
                                }
                                NetworkEvent::PeerDisconnected(peer) => {
                                    tracing::info!(%peer, "peer disconnected");
                                }
                                NetworkEvent::Message { topic, .. } => {
                                    tracing::debug!(%topic, "gossip: unknown topic — ignoring");
                                }
                            }
                        }
                        None => {
                            tracing::warn!("gossip event stream ended");
                            break;
                        }
                    }
                }
                tx = self.transactions_rx.recv() => {
                    match tx {
                        Some(tx) => {
                            if let Err(e) = gossip.publish(TOPIC_TXS, tx) {
                                tracing::warn!("gossip publish error: {e}");
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
                block = self.blocks_rx.recv() => {
                    match block {
                        Some(block) => {
                            if let Err(e) = gossip.publish(TOPIC_BLOCKS, block) {
                                tracing::warn!("gossip publish error: {e}");
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }

        tracing::info!("gossip service stopped");
        Ok(())
    }
}
