use strum_macros::EnumString;

use super::error::KeyPairError;

/// The signature scheme type.
///
/// Currently only EdDSA is supported.
#[derive(Clone, Copy, Debug, EnumString, strum_macros::Display, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum SignatureScheme {
    Ed25519,
}

impl SignatureScheme {
    /// Returns the flag byte of the signature scheme.
    pub fn flag(&self) -> u8 {
        match self {
            SignatureScheme::Ed25519 => 0x00,
        }
    }
}

impl From<SignatureScheme> for u8 {
    fn from(scheme: SignatureScheme) -> Self {
        scheme.flag()
    }
}

impl TryFrom<u8> for SignatureScheme {
    type Error = KeyPairError;

    fn try_from(flag: u8) -> Result<Self, Self::Error> {
        match flag {
            0x00 => Ok(SignatureScheme::Ed25519),
            _ => Err(KeyPairError::InvalidSignatureSchemeFlag { flag }),
        }
    }
}
