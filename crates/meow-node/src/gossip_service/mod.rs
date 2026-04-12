pub mod error;

use std::{
    collections::{BTreeSet, HashMap},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
};

use meow_gossip_network::GossipNetwork;
use meow_gossip_types::{config::GossipNetworkConfig, event::NetworkEvent, peer_id::PeerId};
use meow_nakamoto::miner::Miner;
use meow_nakamoto_types::block::Block;
use meow_node_client::NodeClient;
use meow_types::{digest::Digest, transaction::SignedTransaction};
use tokio::sync::{Mutex, mpsc, watch};
use url::Url;

use crate::gossip_service::error::GossipServiceError;

/// The result type related to the gossip service.
pub type Result<T> = std::result::Result<T, GossipServiceError>;

/// Submitted transactions are published to the gossip network on this topic for other peers to validate and include in blocks.
const TOPIC_TRANSACTIONS: &str = "transactions";
/// Mined blocks are published on this topic for other peers to validate and extend.
const TOPIC_BLOCKS: &str = "blocks";
/// Peers exchange their HTTP RPC URLs on this topic to enable chain sync for late joiners.
const TOPIC_PEER_INFO: &str = "peer-info";

/// All gossip topics used by this node. Useful for subscribing and logging.
const TOPICS: [&str; 3] = [TOPIC_TRANSACTIONS, TOPIC_BLOCKS, TOPIC_PEER_INFO];

/// The state of the gossip service, used to coordinate chain syncing and normal operation.
enum GossipServiceState {
    Working,
    Syncing {
        buffered_blocks: Vec<Block>,
        buffered_hashes: BTreeSet<Digest>,
    },
}

/// The gossip service, responsible for handling network events and interacting with the miner.
pub struct GossipService {
    /// Shared miner, protected by a mutex for synchronization and interior mutability.
    miner: Arc<Mutex<Miner>>,
    /// Handle to receive transactions to be published to the gossip network.
    transactions_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    /// Handle to receive mined blocks to be published to the gossip network.
    blocks_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    /// The HTTP RPC URL of this node, broadcast to peers so they can sync the chain.
    node_rpc_url: Url,
    /// RPC URLs of known peers, collected from peer-info messages, keyed by peer ID.
    /// Used to fetch missing blocks when a gap is detected.
    known_peer_urls: HashMap<PeerId, Url>,
}

impl GossipService {
    /// Creates a new gossip service.
    pub fn new(
        miner: Arc<Mutex<Miner>>,
        transactions_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        blocks_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        node_rpc_bound_addr: SocketAddr,
    ) -> Self {
        Self {
            miner,
            transactions_rx,
            blocks_rx,
            node_rpc_url: Url::parse(&format!("http://{node_rpc_bound_addr}"))
                .expect("the RPC URL must be valid"),
            known_peer_urls: HashMap::new(),
        }
    }

    /// Runs the gossip loop, handling network events and interacting with the miner.
    pub async fn run(
        mut self,
        config: GossipNetworkConfig,
        mut shutdown_rx: watch::Receiver<()>,
    ) -> Result<()> {
        tracing::info!("starting gossip service");

        let mut gossip: GossipNetwork = GossipNetwork::new(config)?;
        let local_peer_id = gossip.local_peer_id();

        tracing::info!(%local_peer_id, "network started");

        gossip.subscribe(TOPIC_TRANSACTIONS)?;
        gossip.subscribe(TOPIC_BLOCKS)?;
        gossip.subscribe(TOPIC_PEER_INFO)?;

        tracing::info!(topics = ?TOPICS, "subscriptions ready");

        let mut sync_fut: Pin<Box<dyn Future<Output = Vec<Block>> + Send>> =
            Box::pin(std::future::pending());
        let mut service_state = GossipServiceState::Working;

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    match changed {
                        Ok(()) => {
                            tracing::info!("shutdown signal received");
                            break;
                        }
                        Err(_) => {
                            tracing::warn!("shutdown channel closed");
                            break;
                        }
                    }
                }
                event = gossip.next_event() => {
                    match event {
                        Some(message) => {
                            match message {
                                NetworkEvent::Message { topic, data, .. } if topic == TOPIC_TRANSACTIONS => {
                                    match bcs::from_bytes::<SignedTransaction>(&data) {
                                        Ok(tx) => {
                                            if let Err(e) = self.miner.lock().await.submit_tx(tx) {
                                                tracing::debug!(error = %e, "incoming transaction rejected");
                                            }
                                        }
                                        Err(e) => tracing::debug!(error = %e, "failed to decode transaction"),
                                    }
                                }
                                NetworkEvent::Message { topic, data, from, .. } if topic == TOPIC_BLOCKS => {
                                    match bcs::from_bytes::<Block>(&data) {
                                        Ok(block) => {
                                            if let GossipServiceState::Syncing { buffered_blocks, buffered_hashes } = &mut service_state {
                                                let hash = block.hash();
                                                if buffered_hashes.insert(hash) {
                                                    buffered_blocks.push(block);
                                                }
                                            } else {
                                                let height = block.header.height;
                                                let local_height = self.miner.lock().await.head_height();

                                                if height > local_height + 1 {
                                                    // Gap: we are missing blocks between local_height+1 and height-1.
                                                    // Pull from the peer that sent this block; fall back to any known peer.
                                                    tracing::info!(local_height, height, "block gap detected, syncing missing blocks from peer");

                                                    let block_hash = block.hash();

                                                    service_state = GossipServiceState::Syncing {
                                                        buffered_blocks: vec![block],
                                                        buffered_hashes: [block_hash].into_iter().collect(),
                                                    };

                                                    let peer_url = from
                                                        .as_ref()
                                                        .and_then(|id| self.known_peer_urls.get(id))
                                                        .or_else(|| self.known_peer_urls.values().next())
                                                        .cloned();

                                                    if let Some(peer_url) = peer_url {
                                                        sync_fut = Box::pin(async move {
                                                            let start_height = local_height + 1;
                                                            pull_blocks_from_peer(peer_url, start_height).await
                                                        });
                                                    } else {
                                                        tracing::warn!(local_height, height, "block gap detected but no known peer URL yet");
                                                    }
                                                } else {
                                                    let switched = self.miner.lock().await.apply_block(block);
                                                    if switched {
                                                        tracing::info!(height, "reorged to peer's longer chain");
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => tracing::debug!(error = %e, "failed to decode block"),
                                    }
                                }
                                NetworkEvent::Message { topic, data, from, .. } if topic == TOPIC_PEER_INFO => {
                                    match String::from_utf8(data) {
                                        Ok(peer_rpc_url) => {
                                            let peer_rpc_url = match Url::parse(&peer_rpc_url) {
                                                Ok(url) => url,
                                                Err(e) => {
                                                    tracing::warn!(error = %e, "received invalid peer RPC URL");
                                                    continue;
                                                }
                                            };
                                            if let Some(peer_id) = from {
                                                self.known_peer_urls.entry(peer_id).or_insert_with(|| {
                                                    tracing::info!(rpc_url = %peer_rpc_url, "new peer discovered");
                                                    peer_rpc_url
                                                });
                                            }
                                        }
                                        Err(e) => tracing::warn!(error = %e, "received invalid peer info"),
                                    }
                                }
                                NetworkEvent::Message { topic, .. } => {
                                    tracing::debug!(name = %topic, "message on unknown topic — ignoring");
                                }
                                NetworkEvent::Listening { addr } => {
                                    tracing::info!(%addr, %local_peer_id, "gossip listening");
                                }
                                NetworkEvent::PeerConnected(peer) => {
                                    tracing::info!(peer_id = %peer, "peer connected");
                                }
                                NetworkEvent::PeerDisconnected(peer) => {
                                    tracing::info!(peer_id = %peer, "peer disconnected");
                                }
                                NetworkEvent::PeerSubscribedToTopic { peer, topic } if topic == TOPIC_PEER_INFO => {
                                    tracing::debug!(peer_id = %peer, "peer subscribed to peer-info, broadcasting our RPC URL");
                                    // The peer is now ready to receive on this topic — safe to send.
                                    let data = self.node_rpc_url.as_str().as_bytes().to_vec();
                                    if let Err(e) = gossip.publish(TOPIC_PEER_INFO, data) {
                                        tracing::warn!(error = %e, "failed to publish peer info");
                                    }
                                }
                                NetworkEvent::PeerSubscribedToTopic { topic, .. } => {
                                    tracing::debug!(name = %topic, "unknown subscribed topic — ignoring");
                                }
                            }
                        }
                        None => {
                            tracing::warn!("event stream ended");
                            break;
                        }
                    }
                }
                tx = self.transactions_rx.recv() => {
                    match tx {
                        Some(tx) => {
                            if let Err(e) = gossip.publish(TOPIC_TRANSACTIONS, tx) {
                                tracing::warn!(error = %e, "failed to publish transaction");
                            }
                        }
                        None => {
                            tracing::warn!("transactions channel closed unexpectedly");
                            break;
                        }
                    }
                }
                block = self.blocks_rx.recv() => {
                    match block {
                        Some(block) => {
                            if let Err(e) = gossip.publish(TOPIC_BLOCKS, block) {
                                tracing::warn!(error = %e, "failed to publish block");
                            }
                        }
                        None => {
                            tracing::warn!("blocks channel closed unexpectedly");
                            break;
                        }
                    }
                }
                mut pulled_blocks = &mut sync_fut => {
                    sync_fut = Box::pin(std::future::pending());

                    let pulled_blocks_count = pulled_blocks.len();

                    // Apply pulled blocks first, then buffered gossip blocks.
                    let mut seen: BTreeSet<Digest> = BTreeSet::new();

                    pulled_blocks.sort_unstable_by_key(|b| b.header.height);

                    let mut miner = self.miner.lock().await;
                    for block in pulled_blocks {
                        if seen.insert(block.hash()) {
                            miner.apply_block(block);
                        }
                    }

                    let buffered_blocks_count = if let GossipServiceState::Syncing { mut buffered_blocks, .. } = service_state {
                        let buffered_blocks_count = buffered_blocks.len();

                        buffered_blocks.sort_unstable_by_key(|b| b.header.height);
                        buffered_blocks.into_iter().for_each(|block| {
                            if seen.insert(block.hash()) {
                                miner.apply_block(block);
                            }
                        });

                        buffered_blocks_count
                    } else {
                        debug_assert!(false, "GossipServiceState should be Syncing when receiving pulled blocks");
                        0
                    };

                    tracing::info!(
                        pulled_blocks_count,
                        buffered_blocks_count,
                        new_height = miner.head_height(),
                        "chain sync complete"
                    );

                    service_state = GossipServiceState::Working;
                }
            }
        }

        tracing::info!("gossip service stopped");
        Ok(())
    }
}

/// Pulls all blocks from `peer_rpc_url` starting at `from_height` in a single request.
/// Blocks that arrive during the sync are buffered by the caller and applied afterwards.
async fn pull_blocks_from_peer(peer_rpc_url: Url, from_height: u64) -> Vec<Block> {
    let peer_client = NodeClient::with_url(peer_rpc_url.clone());

    match peer_client.get_blocks_since(from_height).await {
        Ok(blocks) => {
            if !blocks.is_empty() {
                tracing::debug!(count = blocks.len(), %peer_rpc_url, "pulled blocks from peer");
            }
            blocks
        }
        Err(e) => {
            tracing::warn!(%peer_rpc_url, error = %e, "chain sync: request failed");
            Vec::new()
        }
    }
}
