//! Gossip service: bridges the libp2p network with the chain, handles topic routing and catch-up sync.

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
use meow_nakamoto_types::{block::Block, state_snapshot::StateSnapshot};
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

/// Maximum number of blocks buffered while a sync is in progress. Without a cap, a
/// peer could flood distinct blocks during a slow/stalled sync and exhaust memory.
/// Sized generously above a full sync window (`snapshot_depth`) so legitimate sync
/// is never starved; excess blocks are dropped (and will be re-gossiped or pulled).
const MAX_BUFFERED_SYNC_BLOCKS: usize = 100_000;

/// The state of the gossip service, used to coordinate chain syncing and normal operation.
enum GossipServiceState {
    Working,
    /// Block sync in progress: gap ≤ snapshot_depth, pulling missing blocks from peer.
    Syncing {
        buffered_blocks: Vec<Block>,
        buffered_hashes: BTreeSet<Digest>,
    },
    /// State sync in progress: gap > snapshot_depth, fetching a full snapshot from peer.
    StateSyncing {
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
        let mut state_sync_fut: Pin<Box<dyn Future<Output = Option<StateSnapshot>> + Send>> =
            Box::pin(std::future::pending());
        let mut service_state = GossipServiceState::Working;

        loop {
            tokio::select! {
                // Check for shutdown signal with higher priority to allow timely shutdown.
                biased;

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
                // State-sync and block-sync futures complete before new gossip events are
                // processed. This prevents a gossip storm from starving an in-flight
                // snapshot or block-pull fetch that has already resolved.
                snapshot = &mut state_sync_fut => {
                    state_sync_fut = Box::pin(std::future::pending());

                    if let Some(snapshot) = snapshot {
                        let snap_height = snapshot.head.header.height;

                        let mut miner = self.miner.lock().await;
                        match miner.replace_from_snapshot(snapshot) {
                            Ok(()) => {
                                tracing::info!(snap_height, new_height = miner.head_height(), state_root = %miner.head_block().header.state_root, "state sync complete");

                                if let GossipServiceState::StateSyncing { mut buffered_blocks, .. } = service_state {
                                    buffered_blocks.sort_unstable_by_key(|b| b.header.height);

                                    let mut seen: BTreeSet<Digest> = BTreeSet::new();

                                    for block in buffered_blocks {
                                        if seen.insert(block.hash()) {
                                            let _ = miner.apply_block(block);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(snap_height, error = %e, "state snapshot rejected — staying on current chain");
                            }
                        }
                    } else {
                        tracing::warn!("state snapshot fetch failed");
                    }

                    service_state = GossipServiceState::Working;
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
                            let _ = miner.apply_block(block);
                        }
                    }

                    let buffered_blocks_count = if let GossipServiceState::Syncing { mut buffered_blocks, .. } = service_state {
                        let buffered_blocks_count = buffered_blocks.len();

                        buffered_blocks.sort_unstable_by_key(|b| b.header.height);
                        buffered_blocks.into_iter().for_each(|block| {
                            if seen.insert(block.hash()) {
                                let _ = miner.apply_block(block);
                            }
                        });

                        buffered_blocks_count
                    } else {
                        tracing::error!("sync_fut completed but state is not Syncing — this is a bug");
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
                event = gossip.next_event() => {
                    match event {
                        Some(message) => {
                            match message {
                                NetworkEvent::Message { topic, data, .. } if topic == TOPIC_TRANSACTIONS => {
                                    match bcs::from_bytes::<SignedTransaction>(&data) {
                                        Ok(signed_transaction) => {
                                            let digest = signed_transaction.transaction().digest();
                                            tracing::debug!(%digest, "received transaction via gossip");

                                            if let Err(e) = self.miner.lock().await.submit_transaction(signed_transaction) {
                                                tracing::debug!(%digest, error = %e, "gossip transaction rejected");
                                            } else {
                                                tracing::debug!(%digest, "accepted gossip transaction into mempool");
                                            }
                                        }
                                        Err(e) => tracing::debug!(error = %e, "failed to decode transaction"),
                                    }
                                }
                                NetworkEvent::Message { topic, data, from, .. } if topic == TOPIC_BLOCKS => {
                                    match bcs::from_bytes::<Block>(&data) {
                                        Ok(block) => {
                                            let block_hash = block.hash();
                                            let height = block.header.height;
                                            tracing::debug!(height, %block_hash, "received block via gossip");

                                            if let GossipServiceState::Syncing { buffered_blocks, buffered_hashes }
                                                | GossipServiceState::StateSyncing { buffered_blocks, buffered_hashes } = &mut service_state
                                            {
                                                if buffered_blocks.len() >= MAX_BUFFERED_SYNC_BLOCKS {
                                                    tracing::warn!(height, %block_hash, cap = MAX_BUFFERED_SYNC_BLOCKS, "sync block buffer full — dropping block (will be re-fetched after sync)");
                                                } else if buffered_hashes.insert(block_hash) {
                                                    tracing::debug!(height, %block_hash, "buffering block during sync");
                                                    buffered_blocks.push(block);
                                                }
                                            } else {
                                                let (local_height, sync_start, snapshot_depth) = {
                                                    let miner = self.miner.lock().await;
                                                    (miner.head_height(), miner.sync_from_height(), miner.snapshot_depth())
                                                };

                                                let peer_url = from
                                                    .as_ref()
                                                    .and_then(|id| self.known_peer_urls.get(id))
                                                    .or_else(|| self.known_peer_urls.values().next())
                                                    .cloned();

                                                if height > local_height + snapshot_depth {
                                                    // Gap too large for block sync — fetch a full state snapshot.
                                                    tracing::info!(height, %block_hash, local_height, "large block gap detected, fetching state snapshot from peer");

                                                    service_state = GossipServiceState::StateSyncing {
                                                        buffered_blocks: vec![block],
                                                        buffered_hashes: [block_hash].into_iter().collect(),
                                                    };

                                                    if let Some(peer_url) = peer_url {
                                                        state_sync_fut = Box::pin(pull_snapshot_from_peer(peer_url));
                                                    } else {
                                                        tracing::warn!(height, %block_hash, local_height, "state sync needed but no known peer URL yet");
                                                    }
                                                } else {
                                                    // Decide whether to apply directly or trigger a block sync.
                                                    //
                                                    // A block sync is needed in two cases:
                                                    //   1. Height gap > 1: we are definitely missing intermediate blocks.
                                                    //   2. Height == local_height + 1 but apply_block fails: the fork
                                                    //      diverged before our head so the parent is unknown to us.
                                                    //
                                                    // In both cases we pull from sync_from_height() (head − snapshot_depth)
                                                    // so the pulled range always contains the common ancestor of any
                                                    // fork we can resolve, even when it started before our current head.
                                                    let need_sync = if height > local_height + 1 {
                                                        tracing::info!(height, %block_hash, local_height, sync_start, "block gap detected, syncing from peer");
                                                        true
                                                    } else if height > local_height {
                                                        // Next block — try the fast path first.
                                                        match self.miner.lock().await.apply_block(block.clone()) {
                                                            Ok(true) => {
                                                                tracing::info!(height, %block_hash, "chain head advanced via gossip");
                                                                false
                                                            }
                                                            Ok(false) => {
                                                                tracing::debug!(height, %block_hash, "gossip block stored on fork branch, head unchanged");
                                                                false
                                                            }
                                                            Err(e) => {
                                                                // Block rejected — peer may be on a fork that
                                                                // diverged before our head; pull back far enough
                                                                // to find the common ancestor.
                                                                tracing::info!(height, %block_hash, local_height, error = %e, "block rejected, peer may be on a diverging fork — syncing");
                                                                true
                                                            }
                                                        }
                                                    } else {
                                                        // Block at or below our height — apply opportunistically
                                                        // (builds fork branches that taller blocks can extend).
                                                        match self.miner.lock().await.apply_block(block.clone()) {
                                                            Ok(true) => tracing::debug!(height, %block_hash, local_height, "past block applied, head updated"),
                                                            Ok(false) => tracing::debug!(height, %block_hash, local_height, "past block stored on fork branch"),
                                                            Err(e) => tracing::debug!(height, %block_hash, local_height, error = %e, "past block rejected"),
                                                        }
                                                        false
                                                    };

                                                    if need_sync {
                                                        service_state = GossipServiceState::Syncing {
                                                            buffered_blocks: vec![block],
                                                            buffered_hashes: [block_hash].into_iter().collect(),
                                                        };
                                                        if let Some(peer_url) = peer_url {
                                                            sync_fut = Box::pin(async move {
                                                                pull_blocks_from_peer(peer_url, sync_start).await
                                                            });
                                                        } else {
                                                            tracing::warn!(height, %block_hash, local_height, "sync needed but no known peer URL yet");
                                                        }
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
                                                self.known_peer_urls.entry(peer_id.clone()).or_insert_with(|| {
                                                    tracing::info!(%peer_id, rpc_url = %peer_rpc_url, "new peer discovered");
                                                    peer_rpc_url
                                                });
                                            }
                                        }
                                        Err(e) => tracing::warn!(error = %e, "received invalid peer info"),
                                    }
                                }
                                NetworkEvent::Message { topic, .. } => {
                                    tracing::debug!(topic = %topic, "message on unknown topic — ignoring");
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
                                    tracing::debug!(peer_id = %peer, "broadcasting RPC URL to new peer subscriber");
                                    // The peer is now ready to receive on this topic — safe to send.
                                    let data = self.node_rpc_url.as_str().as_bytes().to_vec();
                                    if let Err(e) = gossip.publish(TOPIC_PEER_INFO, data) {
                                        tracing::warn!(peer_id = %peer, rpc_url = %self.node_rpc_url, error = %e, "failed to publish peer info");
                                    }
                                }
                                NetworkEvent::PeerSubscribedToTopic { topic, .. } => {
                                    tracing::debug!(topic = %topic, "unknown subscribed topic — ignoring");
                                }
                            }
                        }
                        None => {
                            tracing::warn!("event stream ended");
                            break;
                        }
                    }
                }
                signed_transaction_bytes = self.transactions_rx.recv() => {
                    match signed_transaction_bytes {
                        Some(signed_transaction_bytes) => {
                            if let Err(e) = gossip.publish(TOPIC_TRANSACTIONS, signed_transaction_bytes) {
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
            }
        }

        tracing::info!("gossip service stopped");
        Ok(())
    }
}

/// Fetches a full state snapshot from `peer_rpc_url`.
/// Validation (PoW + state_root) is performed by the caller via [`Miner::replace_from_snapshot`].
async fn pull_snapshot_from_peer(peer_rpc_url: Url) -> Option<StateSnapshot> {
    let peer_client = NodeClient::with_url(peer_rpc_url.clone());

    match peer_client.get_state_snapshot().await {
        Ok(snapshot) => {
            tracing::debug!(%peer_rpc_url, height = snapshot.head.header.height, "received state snapshot from peer");
            Some(snapshot)
        }
        Err(e) => {
            tracing::warn!(%peer_rpc_url, error = %e, "state snapshot request failed");
            None
        }
    }
}

/// Pulls all blocks from `peer_rpc_url` starting at `from_height` in a single request.
/// Blocks that arrive during the sync are buffered by the caller and applied afterwards.
async fn pull_blocks_from_peer(peer_rpc_url: Url, from_height: u64) -> Vec<Block> {
    let peer_client = NodeClient::with_url(peer_rpc_url.clone());

    match peer_client.get_blocks_since(from_height).await {
        Ok(blocks) => {
            if !blocks.is_empty() {
                tracing::debug!(from_height, count = blocks.len(), %peer_rpc_url, "pulled blocks from peer");
            }
            blocks
        }
        Err(e) => {
            tracing::warn!(from_height, %peer_rpc_url, error = %e, "block sync request failed");
            Vec::new()
        }
    }
}
