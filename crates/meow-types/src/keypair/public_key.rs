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
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        match self {
            PublicKey::Ed25519(public_key) => public_key.0.as_bytes(),
        }
    }
}
