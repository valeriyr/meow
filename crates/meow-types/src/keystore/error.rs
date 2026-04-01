use crate::address::Address;

/// An error related to keystores.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("key pair already exists for address: {0}")]
    KeyPairAlreadyExists(Address),
    #[error("serde json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}
