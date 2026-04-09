/// Errors related to the RPC service.
#[derive(Debug, thiserror::Error)]
pub enum RpcServiceError {
    #[error("RPC service I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
