/// An error related to the miner.
#[derive(Debug, thiserror::Error)]
pub enum MinerError {
    #[error("simulation error: {0}")]
    SimulationError(#[from] meow_vm_adapter::executor::error::ExecutorError),
    #[error("mempool error: {0}")]
    MempoolError(#[from] crate::mempool::error::MempoolError),
}
