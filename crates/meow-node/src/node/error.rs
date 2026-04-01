use std::net::SocketAddr;

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("failed to bind RPC server on {0}: {1}")]
    BindError(SocketAddr, std::io::Error),
    #[error("RPC server error: {0}")]
    RpcServeError(std::io::Error),
    #[error("gossip network error: {0}")]
    GossipError(#[from] meow_gossip_network::error::NetworkError),
}
