use std::net::SocketAddr;

/// Errors related to the node.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("failed to bind RPC server on {0}: {1}")]
    BindFailed(SocketAddr, std::io::Error),
    #[error("RPC service error: {0}")]
    RpcServiceError(std::io::Error),
    #[error("miner service error: {0}")]
    MinerServiceError(#[from] crate::miner_service::error::MinerServiceError),
    #[error("gossip service error: {0}")]
    GossipServiceError(#[from] crate::gossip_service::error::GossipServiceError),
    #[error("signal handler error: {0}")]
    SignalHandlerError(std::io::Error),
}
