use crate::keypair::KeyPair;

/// An error related to keystores.
#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("key already exists: {keypair:?}")]
    KeyPairAlreadyExists { keypair: KeyPair },
    #[error("serde json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}
