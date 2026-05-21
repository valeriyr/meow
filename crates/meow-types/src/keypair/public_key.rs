//! Public key type abstracting over supported signature schemes.

use super::{ed25519::Ed25519PublicKey, signature_scheme::SignatureScheme};

/// The public key type.
///
/// Currently only EdDSA is supported.
#[derive(Debug, PartialEq, Eq)]
pub enum PublicKey {
    Ed25519(Ed25519PublicKey),
}

impl PublicKey {
    /// Returns the scheme.
    pub fn scheme(&self) -> SignatureScheme {
        match self {
            PublicKey::Ed25519(_) => SignatureScheme::Ed25519,
        }
    }

    /// Encodes the public key to a base64 string.
    pub fn encode_base64(&self) -> String {
        match self {
            PublicKey::Ed25519(public_key) => public_key.encode_base64(),
        }
    }

    /// Encodes the public key to a hex string.
    pub fn encode_hex(&self) -> String {
        match self {
            PublicKey::Ed25519(public_key) => public_key.encode_hex(),
        }
    }
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        match self {
            PublicKey::Ed25519(public_key) => public_key.as_bytes(),
        }
    }
}
