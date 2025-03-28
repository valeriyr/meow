pub mod error;

use std::{fmt, str::FromStr};

use blake2::{
    Blake2b, Digest,
    digest::{consts::U32, generic_array::GenericArray},
};
use error::AddressError;

use crate::keypair::{KeyPair, public_key::PublicKey};

/// The result type related to keystores.
pub type Result<T> = std::result::Result<T, AddressError>;

/// The address length.
pub const ADDRESS_LENGTH: usize = 32;

/// The meow account address type.
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Address([u8; ADDRESS_LENGTH]);

impl Address {
    /// The zero address constant.
    pub const ZERO: Self = Self([0; ADDRESS_LENGTH]);

    /// Creates a new address.
    pub fn new(address: [u8; ADDRESS_LENGTH]) -> Self {
        Self(address)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", prefix_hex::encode(self.0))
    }
}

impl FromStr for Address {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Self> {
        let bytes: Vec<u8> = prefix_hex::decode(s)?;

        Address::try_from(bytes.as_slice())
    }
}

impl From<&KeyPair> for Address {
    fn from(keypair: &KeyPair) -> Self {
        Address::from(keypair.public())
    }
}

impl From<PublicKey> for Address {
    fn from(public_key: PublicKey) -> Self {
        let mut hasher = Blake2b::<U32>::default();

        hasher.update([public_key.scheme().flag()]);
        hasher.update(&public_key);

        let mut bytes = [0; ADDRESS_LENGTH];
        hasher.finalize_into(GenericArray::from_mut_slice(&mut bytes));

        Address::new(bytes)
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = AddressError;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        <[u8; ADDRESS_LENGTH]>::try_from(bytes)
            .map_err(|_| AddressError::InvalidAddressBytesLength {
                actual: bytes.len(),
                expected: ADDRESS_LENGTH,
            })
            .map(Address)
    }
}
