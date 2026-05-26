//! Error type for the transaction mempool.

use meow_types::{address::Address, digest::Digest, object::object_version::ObjectVersion};

/// An error related to the mempool.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum MempoolError {
    #[error("transaction already in mempool: {digest}")]
    DuplicateTransaction { digest: Digest },
    #[error("object {address} has invalid digest: expected {expected}, found {found}")]
    InvalidObjectDigest {
        address: Address,
        expected: Digest,
        found: Digest,
    },
    #[error("object {address} has invalid version: expected {expected}, found {found}")]
    InvalidObjectVersion {
        address: Address,
        expected: ObjectVersion,
        found: ObjectVersion,
    },
    #[error("object {address} not found in store")]
    ObjectNotFound { address: Address },
    #[error("transaction validation error: {0}")]
    TransactionValidationError(#[from] meow_types::transaction::validator::ValidationError),
}
