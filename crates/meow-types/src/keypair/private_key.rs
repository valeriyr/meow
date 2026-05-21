//! Private key type abstracting over supported signature schemes.

use super::ed25519::Ed25519PrivateKey;

/// The private key type.
///
/// Currently only EdDSA is supported.
#[derive(Debug, PartialEq, Eq)]
pub enum PrivateKey {
    Ed25519(Ed25519PrivateKey),
}
