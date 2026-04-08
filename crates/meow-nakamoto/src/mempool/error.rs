use meow_types::{address::Address, digest::Digest, object::object_version::ObjectVersion};

/// An error related to the mempool.
#[derive(Debug, thiserror::Error)]
pub enum MempoolError {
    #[error("transaction already in mempool: {0}")]
    DuplicateTransaction(Digest),
    #[error("object {0} not found in store")]
    ObjectNotFound(Address),
    #[error("object {address} has invalid version: expected {expected}, found {found}")]
    InvalidObjectVersion {
        address: Address,
        expected: ObjectVersion,
        found: ObjectVersion,
    },
    #[error("object {address} has invalid digest: expected {expected}, found {found}")]
    InvalidObjectDigest {
        address: Address,
        expected: Digest,
        found: Digest,
    },
    #[error("transaction validation error: {0}")]
    TransactionValidationError(#[from] meow_types::transaction::validator::ValidationError),
}
