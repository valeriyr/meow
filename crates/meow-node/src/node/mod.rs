use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use meow_gossip_network::{config::NetworkConfig, event::NetworkEvent};
use meow_nakamoto::{block::Block, mempool::Mempool, miner::Miner, store::Store};
use meow_types::transaction::SignedTransaction;

use crate::{
    gossip::{self, TOPIC_BLOCKS, TOPIC_TXS},
    rpc::{self, AppState},
};

pub mod error;
use error::NodeError;

/// The result type related to the node.
pub type Result<T> = std::result::Result<T, NodeError>;

/// The main node struct, containing configuration and shared state.
pub struct Node {
    rpc_addr: SocketAddr,
    gossip_config: NetworkConfig,
    miner: Arc<Mutex<Miner>>,
}

impl Node {
    /// Creates a new node with the given RPC address, gossip network config, and PoW difficulty.
    pub fn new(rpc_addr: SocketAddr, gossip_config: NetworkConfig, difficulty: u32) -> Self {
        let miner = Arc::new(Mutex::new(Miner::new(
            Store::default(),
            Mempool::empty(),
            difficulty,
        )));
        Self {
            rpc_addr,
            gossip_config,
            miner,
        }
    }

    /// Starts the node: runs the RPC server, gossip event loop, and mining loop concurrently.
    pub async fn run(self) -> Result<()> {
        let (gossip_handle, mut events_rx) = gossip::start(self.gossip_config).await?;

        let state = AppState {
            miner: self.miner.clone(),
            gossip: gossip_handle.clone(),
        };

        let router = rpc::router(state);
        let listener = tokio::net::TcpListener::bind(self.rpc_addr)
            .await
            .map_err(|e| NodeError::BindFailed(self.rpc_addr, e))?;

        tracing::info!(addr = %self.rpc_addr, "RPC listening");

        // Mining loop — prepare and commit hold the lock only briefly;
        // PoW grinding runs without the lock so RPC handlers are not blocked.
        let miner = self.miner.clone();
        let gossip_for_mining = gossip_handle.clone();
        tokio::spawn(async move {
            loop {
                let work = miner.lock().unwrap().prepare_round();

                match work {
                    None => {
                        // Mempool empty — wait before polling again.
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    Some(work) => {
                        // Grind nonce without holding the lock.
                        let (block, new_store) = work.grind();

                        let committed =
                            miner.lock().unwrap().commit_mined(block.clone(), new_store);
                        if committed {
                            tracing::info!(
                                height = block.header.height,
                                txs = block.results.len(),
                                nonce = block.header.nonce,
                                "block mined"
                            );
                            if let Ok(data) = bcs::to_bytes(&block) {
                                gossip_for_mining.publish(TOPIC_BLOCKS, data);
                            }
                        }
                        // Yield between rounds so other tasks can run.
                        tokio::task::yield_now().await;
                    }
                }
            }
        });

        // Gossip event loop — feeds incoming txs into the mempool and
        // applies incoming blocks to the chain.
        let miner = self.miner.clone();
        tokio::spawn(async move {
            while let Some(event) = events_rx.recv().await {
                match event {
                    NetworkEvent::Message { topic, data, .. } if topic == TOPIC_TXS => {
                        match bcs::from_bytes::<SignedTransaction>(&data) {
                            Ok(tx) => {
                                if let Err(e) = miner.lock().unwrap().submit_tx(tx) {
                                    tracing::debug!("incoming tx rejected: {e}");
                                }
                            }
                            Err(e) => tracing::debug!("gossip: failed to decode tx: {e}"),
                        }
                    }
                    NetworkEvent::Message { topic, data, .. } if topic == TOPIC_BLOCKS => {
                        match bcs::from_bytes::<Block>(&data) {
                            Ok(block) => {
                                let switched = miner.lock().unwrap().on_block_received(block);
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
        });

        axum::serve(listener, router)
            .await
            .map_err(NodeError::RpcServeError)?;

        Ok(())
    }
}
