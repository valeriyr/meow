use std::fmt;

use serde::{Deserialize, Serialize};

use super::Result;
use crate::keypair::ed25519::Ed25519Signature;

/// The signature of a message.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Signature {
    Ed25519(Ed25519Signature),
}

impl Signature {
    pub fn verify<T: AsRef<[u8]>>(&self, msg: T) -> Result<()> {
        match self {
            Signature::Ed25519(sig) => sig.verify(msg),
        }
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            Signature::Ed25519(sig) => sig.to_string(),
        };
        f.write_str(&string)
    }
}
