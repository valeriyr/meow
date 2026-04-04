/// Errors related to the miner service.
#[derive(Debug, thiserror::Error)]
pub enum MinerServiceError {
    #[error("miner error: {0}")]
    MinerError(#[from] meow_nakamoto::miner::error::MinerError),
}
