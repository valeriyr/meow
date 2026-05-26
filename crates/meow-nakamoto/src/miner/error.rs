//! Error type for the proof-of-work miner.

/// An error related to the miner.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum MinerError {
    #[error("chain error: {0}")]
    ChainError(#[from] crate::chain::error::ChainError),
    #[error("mempool error: {0}")]
    MempoolError(#[from] crate::mempool::error::MempoolError),
    #[error("simulation error: {0}")]
    SimulationError(#[from] meow_vm_adapter::executor::error::ExecutorError),
}
