//! Error type for transaction execution.

use meow_types::{address::Address, digest::Digest, object::object_version::ObjectVersion};

/// An error related to the executor.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("gas coin not found")]
    GasCoinNotFound,
    #[error("gas coin object is not a valid gas coin")]
    InvalidGasCoin,
    #[error("gas coin object is not owned by the transaction sender")]
    InvalidGasCoinOwner,
    #[error("object {address} has an invalid digest: expected {expected}, found {found}")]
    InvalidObjectDigest {
        address: Address,
        expected: Digest,
        found: Digest,
    },
    #[error("object {address} has an invalid version: expected {expected}, found {found}")]
    InvalidObjectVersion {
        address: Address,
        expected: ObjectVersion,
        found: ObjectVersion,
    },
    #[error("object {0} is at the maximum version")]
    ObjectAtMaxVersion(Address),
    #[error("module publishing is not allowed")]
    ModulePublishNotAllowed,
}
