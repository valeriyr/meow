//! Error type for the MEOW node.

use std::net::SocketAddr;

/// Errors related to the node.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("failed to bind RPC server on {0}: {1}")]
    BindFailed(SocketAddr, std::io::Error),
    #[error("gossip service error: {0}")]
    GossipServiceError(#[from] crate::gossip_service::error::GossipServiceError),
    #[error("miner service error: {0}")]
    MinerServiceError(#[from] crate::miner_service::error::MinerServiceError),
    #[error("RPC service error: {0}")]
    RpcServiceError(#[from] crate::rpc::error::RpcServiceError),
    #[error("signal handler error: {0}")]
    SignalHandlerError(std::io::Error),
    #[error("TCP listener error: {0}")]
    TcpListenerError(std::io::Error),
}
