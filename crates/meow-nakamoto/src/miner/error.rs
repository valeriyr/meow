/// An error related to the miner.
#[derive(Debug, thiserror::Error)]
pub enum MinerError {
    #[error("mempool error: {0}")]
    MempoolError(#[from] crate::mempool::error::MempoolError),
}
