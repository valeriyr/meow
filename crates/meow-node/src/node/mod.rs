pub mod config;
pub mod error;

use std::{net::SocketAddr, sync::Arc};

use meow_genesis::Genesis;
use meow_nakamoto::miner::Miner;
use meow_nakamoto_types::miner_config::MinerConfig;
use tokio::pin;
use tokio::sync::{Mutex, mpsc, oneshot, watch};

use error::NodeError;

use crate::{
    gossip_service::GossipService,
    miner_service::MinerService,
    node::config::NodeConfig,
    rpc::{
        rpc_handler::RpcHandler,
        rpc_service::{self, RpcState},
    },
};

/// The result type related to the node.
pub type Result<T> = std::result::Result<T, NodeError>;

/// A macro to await a service shutdown and log any errors that occur during shutdown.
macro_rules! await_service_shutdown {
    ($service_name:literal, $future:ident, $err_variant:path) => {
        match (&mut $future).await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!(
                    service = $service_name,
                    error = %e,
                    "service terminated with error during shutdown"
                );
                return Err($err_variant(e));
            }
        }
    };
}

/// The main node struct, containing configuration and shared state.
pub struct Node {
    config: NodeConfig,
    miner: Arc<Mutex<Miner>>,
}

impl Node {
    /// Creates a new node with the given configuration.
    pub fn empty(mode_config: NodeConfig, miner_config: MinerConfig) -> Self {
        Self {
            config: mode_config,
            miner: Arc::new(Mutex::new(Miner::empty(miner_config))),
        }
    }

    /// Creates a new node pre-seeded with the given genesis state.
    pub fn with_genesis(
        mode_config: NodeConfig,
        miner_config: MinerConfig,
        genesis: &Genesis,
    ) -> Self {
        Self {
            config: mode_config,
            miner: Arc::new(Mutex::new(Miner::with_genesis(genesis, miner_config))),
        }
    }

    /// Returns a reference to the node's configuration.
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }

    /// Starts the node: runs the RPC server, gossip event loop, and mining loop concurrently.
    pub async fn run(self) -> Result<()> {
        self.run_internal(None).await
    }

    /// Starts the node, sending the bound `SocketAddr` through `tcp_listener_ready_tx` once
    /// the TCP listener is ready — useful for tests that use port 0.
    pub async fn run_notifying(
        self,
        tcp_listener_ready_tx: oneshot::Sender<SocketAddr>,
    ) -> Result<()> {
        self.run_internal(Some(tcp_listener_ready_tx)).await
    }

    /// Internal run method that optionally sends the bound address through a channel.
    async fn run_internal(
        self,
        tcp_listener_ready_tx: Option<oneshot::Sender<SocketAddr>>,
    ) -> Result<()> {
        let NodeConfig {
            rpc_listen,
            gossip_network_config,
        } = self.config;

        tracing::info!(
            rpc_listen = %rpc_listen,
            gossip_listen_addr = %gossip_network_config.listen_address,
            bootstrap_peers = gossip_network_config.bootstrap_peers.len(),
            "starting node services"
        );

        let (transactions_tx, transactions_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (blocks_tx, blocks_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        let rpc_handler = RpcHandler::new(self.miner.clone(), transactions_tx);
        let rpc_state = RpcState::new(rpc_handler);
        let rpc_router = rpc_service::router(rpc_state);
        let tcp_listener = tokio::net::TcpListener::bind(rpc_listen)
            .await
            .map_err(|e| NodeError::BindFailed(rpc_listen, e))?;

        let bound_addr = tcp_listener
            .local_addr()
            .map_err(NodeError::TcpListenerError)?;
        tracing::info!(addr = %bound_addr, "RPC listening");

        if let Some(tcp_listener_ready_tx) = tcp_listener_ready_tx {
            let _ = tcp_listener_ready_tx.send(bound_addr);
        }

        let miner_service = MinerService::new(self.miner.clone(), blocks_tx);
        let gossip_service =
            GossipService::new(self.miner.clone(), transactions_rx, blocks_rx, bound_addr);

        let rpc_future = rpc_service::run(rpc_router, tcp_listener, shutdown_rx.clone());
        let miner_future = miner_service.run(shutdown_rx.clone());
        let gossip_future = gossip_service.run(gossip_network_config, shutdown_rx.clone());
        let ctrl_c_future = tokio::signal::ctrl_c();

        pin!(rpc_future);
        pin!(miner_future);
        pin!(gossip_future);
        pin!(ctrl_c_future);

        tokio::select! {
            result = &mut ctrl_c_future => {
                match result {
                    Ok(()) => {
                        tracing::info!("Ctrl+C received, shutting down node gracefully");
                        let _ = shutdown_tx.send(());

                        await_service_shutdown!("rpc", rpc_future, NodeError::RpcServiceError);
                        await_service_shutdown!("miner", miner_future, NodeError::MinerServiceError);
                        await_service_shutdown!("gossip", gossip_future, NodeError::GossipServiceError);

                        tracing::info!("node shutdown complete");
                    }
                    Err(e) => return Err(NodeError::SignalHandlerError(e)),
                }
            }
            rpc_result = &mut rpc_future => {
                match rpc_result {
                    Ok(()) => {
                        tracing::warn!(service = "rpc", "service terminated gracefully");
                    }
                    Err(e) => {
                        tracing::error!(service = "rpc", error = %e, "service terminated unexpectedly");
                        return Err(NodeError::RpcServiceError(e));
                    }
                }
            }
            miner_result = &mut miner_future => {
                match miner_result {
                    Ok(()) => {
                        tracing::warn!(service = "miner", "service terminated gracefully");
                    }
                    Err(e) => {
                        tracing::error!(service = "miner", error = %e, "service terminated unexpectedly");
                        return Err(NodeError::MinerServiceError(e));
                    }
                }
            }
            gossip_result = &mut gossip_future => {
                match gossip_result {
                    Ok(()) => {
                        tracing::warn!(service = "gossip", "service terminated gracefully");
                    }
                    Err(e) => {
                        tracing::error!(service = "gossip", error = %e, "service terminated unexpectedly");
                        return Err(NodeError::GossipServiceError(e));
                    }
                }
            }
        }

        Ok(())
    }
}
