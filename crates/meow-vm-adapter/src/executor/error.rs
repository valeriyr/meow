/// An error related to the executor.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("gas coin not found")]
    GasCoinNotFound,
    #[error("gas coin object is not a valid gas coin")]
    InvalidGasCoin,
    #[error("gas coin object is not owned by the transaction sender")]
    InvalidGasCoinOwner,
    #[error("object {0} is at the maximum version")]
    ObjectAtMaxVersion(meow_types::address::Address),
}
