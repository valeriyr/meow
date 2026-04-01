use meow_types::{address::Address, digest::Digest};

/// An error related to the mempool.
#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("invalid transaction signature")]
    InvalidSignature,
    #[error("transaction already in mempool: {0}")]
    DuplicateTransaction(Digest),
    #[error("gas coin {0} not found in store")]
    GasCoinNotFound(Address),
}
