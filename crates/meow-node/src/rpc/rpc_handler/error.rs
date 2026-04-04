/// Errors related to the RPC handler.
#[derive(Debug, thiserror::Error)]
pub enum RpcHandlerError {
    #[error("miner error: {0}")]
    MinerError(#[from] meow_nakamoto::miner::error::MinerError),
}
