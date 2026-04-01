/// An error related to genesis.
#[derive(Debug, thiserror::Error)]
pub enum GenesisError {
    #[error("bcs error: {0}")]
    BcsError(#[from] bcs::Error),
    #[error("builder error: {0}")]
    BuilderError(#[from] meow_vm_adapter::builder::error::BuilderError),
    #[error("executor error: {0}")]
    ExecutorError(#[from] meow_vm_adapter::executor::error::ExecutorError),
    #[error("identifier error: {0}")]
    IdentifierError(#[from] meow_types::identifier::error::IdentifierError),
    #[error("meow coin mint failed: {0}")]
    MeowCoinMintFailed(String),
}
