/// An error related to transactions.
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("key pair error: {0}")]
    KeyPairError(#[from] crate::keypair::error::KeyPairError),
}
