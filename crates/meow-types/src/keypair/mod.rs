mod derivation;
mod ed25519;

pub mod error;
pub mod mnemonic;

pub mod private_key;
pub mod public_key;
pub mod signature_scheme;

use base64::{Engine, engine::general_purpose};
use bip32::DerivationPath;
use ed25519::Ed25519KeyPair;
use error::KeyPairError;
use mnemonic::MnemonicType;
use public_key::PublicKey;
use rand::{CryptoRng, RngCore};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use signature_scheme::SignatureScheme;

/// The result type related to keypairs.
pub type Result<T> = std::result::Result<T, KeyPairError>;

/// The keypair type.
///
/// Currently only EdDSA is supported.
#[derive(Debug, PartialEq, Eq)]
pub enum KeyPair {
    Ed25519(Ed25519KeyPair),
}

impl KeyPair {
    /// Derives a keypair.
    pub fn derive(
        seed: &[u8],
        scheme: SignatureScheme,
        path: Option<DerivationPath>,
    ) -> Result<Self> {
        match scheme {
            SignatureScheme::Ed25519 => Ok(KeyPair::Ed25519(Ed25519KeyPair::derive(seed, path)?)),
        }
    }

    /// Generates a keypair.
    pub fn generate(
        scheme: SignatureScheme,
        path: Option<DerivationPath>,
        mnemonic_type: Option<MnemonicType>,
    ) -> Result<Self> {
        match scheme {
            SignatureScheme::Ed25519 => Ok(KeyPair::Ed25519(Ed25519KeyPair::generate(
                path,
                mnemonic_type,
            )?)),
        }
    }

    /// Generates a random keypair.
    pub fn random<R: CryptoRng + RngCore>(scheme: SignatureScheme, rnd: R) -> Self {
        match scheme {
            SignatureScheme::Ed25519 => KeyPair::Ed25519(Ed25519KeyPair::random(rnd)),
        }
    }

    /// Returns the public key of the keypair.
    pub fn public(&self) -> PublicKey {
        match self {
            KeyPair::Ed25519(keypair) => PublicKey::Ed25519(keypair.public().to_owned()),
        }
    }

    /// Returns a bytes representation of the keypair.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.push(self.public().scheme().flag());

        match self {
            KeyPair::Ed25519(keypair) => {
                bytes.extend_from_slice(keypair.as_bytes());
            }
        }
        bytes
    }

    /// Encodes the keypair to a base64 string.
    pub fn encode_base64(&self) -> String {
        general_purpose::STANDARD.encode(self.to_bytes())
    }

    /// Decodes a keypair from the base64 string.
    pub fn decode_base64(base64: &str) -> Result<Self> {
        let bytes = general_purpose::STANDARD.decode(base64)?;
        Self::from_bytes(&bytes)
    }

    /// Decodes a keypair from the bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let scheme_byte = bytes
            .first()
            .ok_or_else(|| KeyPairError::InvalidKeyPairBytes {
                bytes: bytes.to_owned(),
            })?;

        let scheme = SignatureScheme::try_from(*scheme_byte)?;

        let keypair = match scheme {
            SignatureScheme::Ed25519 => {
                KeyPair::Ed25519(Ed25519KeyPair::try_from(bytes.get(1..).ok_or_else(
                    || KeyPairError::InvalidKeyPairBytes {
                        bytes: bytes.to_owned(),
                    },
                )?)?)
            }
        };

        Ok(keypair)
    }
}

impl Serialize for KeyPair {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let base64 = self.encode_base64();
        serializer.serialize_str(&base64)
    }
}

impl<'de> Deserialize<'de> for KeyPair {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let base64 = String::deserialize(deserializer)?;
        KeyPair::decode_base64(&base64).map_err(|e| Error::custom(e.to_string()))
    }
}
