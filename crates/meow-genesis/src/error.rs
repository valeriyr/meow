//! Error type for genesis construction.

/// An error that occurred while building the genesis state.
#[derive(Debug, thiserror::Error)]
pub enum GenesisError {
    #[error("executor error: {0}")]
    ExecutorError(#[from] meow_vm_adapter::executor::error::ExecutorError),
    #[error("meow coin mint failed: {0}")]
    MeowCoinMintFailed(String),
}
