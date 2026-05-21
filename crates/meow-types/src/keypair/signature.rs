//! Signature type abstracting over supported signature schemes.

use serde::{Deserialize, Serialize};

use super::Result;
use crate::{
    address::Address,
    keypair::{ed25519::Ed25519Signature, public_key::PublicKey},
};

/// The signature of a message.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Signature {
    Ed25519(Ed25519Signature),
}

impl Signature {
    /// Verifies the signature against the given message.
    pub fn verify<T: AsRef<[u8]>>(&self, msg: T) -> Result<()> {
        match self {
            Signature::Ed25519(sig) => sig.verify(msg),
        }
    }

    /// Returns the public key associated with the signature.
    pub fn public_key(&self) -> PublicKey {
        match self {
            Signature::Ed25519(sig) => PublicKey::Ed25519(sig.public_key()),
        }
    }

    /// Returns the signer of the signature.
    pub fn signer(&self) -> Address {
        self.public_key().into()
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Signature::Ed25519(sig) => sig.to_string(),
        };
        f.write_str(&string)
    }
}
