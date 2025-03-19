use std::fmt;

use bip32::{ChildNumber, DerivationPath};
use zeroize::ZeroizeOnDrop;

use super::{
    derivation::DERIVATION_PATH_COIN_TYPE, error::KeyPairError, mnemonic::MnemonicType, Result,
};

/// The Ed25519 derivation path purpose.
const DERIVATION_PATH_PURPOSE_ED25519: u32 = 44;

/// A valid Ed25519 verification key.
///
/// This is also called a public key by other implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed25519PublicKey(pub ed25519_consensus::VerificationKey);

/// An Ed25519 signing key.
///
/// This is also called a secret key by other implementations.
#[derive(ZeroizeOnDrop)]
pub struct Ed25519PrivateKey(pub ed25519_consensus::SigningKey);

/// An Ed25519 key par.
#[derive(Debug, PartialEq, Eq)]
pub struct Ed25519KeyPair {
    public: Ed25519PublicKey,
    private: Ed25519PrivateKey,
}

//
// Implementation of [Ed25519KeyPair].
//

impl Ed25519KeyPair {
    /// Derives an Ed25519 key pair.
    ///
    /// Ed25519 follows SLIP-0010 using hardened path: m/44'/9999'/0'/0'/{index}'.
    ///
    /// # Errors
    /// - [KeyPairError::Bip32Error] if the bip32 error occurs.
    /// - [KeyPairError::InvalidDerivationPath] if the derivation path is invalid.
    /// - [KeyPairError::Ed25519ConsensusError] if the ed25519 consensus error occurs.
    pub fn derive(seed: &[u8], path: Option<DerivationPath>) -> Result<Self> {
        let path = validate_path(path)?;
        let indexes = path.into_iter().map(|i| i.into()).collect::<Vec<_>>();
        let derived = slip10_ed25519::derive_ed25519_private_key(seed, &indexes);
        let private_key = Ed25519PrivateKey(ed25519_consensus::SigningKey::from(derived));

        Ok(private_key.into())
    }

    /// Generates an Ed25519 key pair from a mnemonic phrase.
    ///
    /// # Errors
    /// - [KeyPairError::Bip32Error] if the bip32 error occurs.
    /// - [KeyPairError::InvalidDerivationPath] if the derivation path is invalid.
    /// - [KeyPairError::Ed25519ConsensusError] if the ed25519 consensus error occurs.
    pub fn generate(
        path: Option<DerivationPath>,
        mnemonic_type: Option<MnemonicType>,
    ) -> Result<Self> {
        let mnemonic_type = mnemonic_type.unwrap_or(MnemonicType::Words24);

        let mnemonic = bip39::Mnemonic::new(mnemonic_type.into(), bip39::Language::English);
        let seed = bip39::Seed::new(&mnemonic, "");

        Self::derive(seed.as_bytes(), path)
    }

    /// Returns the public key of the key pair.
    pub fn public(&self) -> &Ed25519PublicKey {
        &self.public
    }

    /// Returns the bytes representation of the key pair.
    pub fn as_bytes(&self) -> &[u8] {
        self.private.0.as_bytes()
    }
}

impl From<Ed25519PrivateKey> for Ed25519KeyPair {
    fn from(private: Ed25519PrivateKey) -> Self {
        let public = Ed25519PublicKey::from(&private);
        Ed25519KeyPair { public, private }
    }
}

impl TryFrom<&[u8]> for Ed25519KeyPair {
    type Error = KeyPairError;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        let private_key = Ed25519PrivateKey(ed25519_consensus::SigningKey::try_from(bytes)?);
        Ok(private_key.into())
    }
}

//
// Implementation of [Ed25519PublicKey].
//

impl From<&Ed25519PrivateKey> for Ed25519PublicKey {
    fn from(private: &Ed25519PrivateKey) -> Self {
        Ed25519PublicKey(private.0.verification_key())
    }
}

//
// Implementation of [Ed25519PrivateKey].
//

impl fmt::Debug for Ed25519PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<elided secret for Ed25519PrivateKey>")
    }
}

impl PartialEq for Ed25519PrivateKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ref() == other.0.as_ref()
    }
}

impl Eq for Ed25519PrivateKey {}

//
// Utility functions.
//

fn validate_path(path: Option<DerivationPath>) -> Result<DerivationPath> {
    match path {
        Some(path) => {
            if let &[purpose, coin_type, account, change, address] = path.as_ref() {
                if Some(purpose) == ChildNumber::new(DERIVATION_PATH_PURPOSE_ED25519, true).ok()
                    && (Some(coin_type) == ChildNumber::new(DERIVATION_PATH_COIN_TYPE, true).ok())
                    && account.is_hardened()
                    && change.is_hardened()
                    && address.is_hardened()
                {
                    Ok(path)
                } else {
                    Err(KeyPairError::InvalidDerivationPath { path })
                }
            } else {
                Err(KeyPairError::InvalidDerivationPath { path })
            }
        }
        None => Ok(format!(
            "m/{DERIVATION_PATH_PURPOSE_ED25519}'/{DERIVATION_PATH_COIN_TYPE}'/0'/0'/0'"
        )
        .parse()?),
    }
}
