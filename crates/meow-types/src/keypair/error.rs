use bip32::DerivationPath;

/// An error related to key pairs.
#[derive(Debug, thiserror::Error)]
pub enum KeyPairError {
    #[error("bip32 error: {0}")]
    Bip32Error(#[from] bip32::Error),
    #[error("base64 decode error: {0}")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("ed25519_consensus error: {0}")]
    Ed25519ConsensusError(#[from] ed25519_consensus::Error),
    #[error("invalid derivation path: {path}")]
    InvalidDerivationPath { path: DerivationPath },
    #[error("invalid signature scheme flag: {flag}")]
    InvalidSignatureSchemeFlag { flag: u8 },
    #[error("invalid key pair bytes: {bytes:?}")]
    InvalidKeyPairBytes { bytes: Vec<u8> },
}
