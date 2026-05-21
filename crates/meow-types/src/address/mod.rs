//! 32-byte identifier used for both user accounts and on-chain objects.
//!
//! The same address type covers both accounts (derived from public keys) and objects
//! (derived from the transaction digest and a counter), so any address can be used
//! as an object reference or an ownership target without conversion.

pub mod error;

use std::str::FromStr;

use blake2::{Blake2b, digest::consts::U32};
use error::AddressError;
use serde::{Deserialize, Serialize};

use crate::{
    digest::Digest,
    keypair::{KeyPair, public_key::PublicKey},
};

/// The result type related to addresses.
pub type Result<T> = std::result::Result<T, AddressError>;

/// The address length.
pub const ADDRESS_LENGTH: usize = 32;

/// The meow account address type.
#[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct Address([u8; ADDRESS_LENGTH]);

impl Address {
    /// The zero address constant.
    pub const ZERO: Self = Self([0; ADDRESS_LENGTH]);

    /// Creates a new address.
    pub const fn new(bytes: [u8; ADDRESS_LENGTH]) -> Self {
        Self(bytes)
    }

    /// Creates an address filled with the given byte.
    pub const fn fill(byte: u8) -> Self {
        Self([byte; ADDRESS_LENGTH])
    }

    /// Creates an address whose last two bytes hold `suffix` (big-endian) and all other bytes are zero.
    pub const fn suffixed(suffix: u16) -> Self {
        let mut addr = [0u8; ADDRESS_LENGTH];
        let [hi, lo] = suffix.to_be_bytes();
        addr[ADDRESS_LENGTH - 2] = hi;
        addr[ADDRESS_LENGTH - 1] = lo;
        Self(addr)
    }

    /// Derives a new address from the given digest, tag, and counter.
    ///
    /// Constructs a 33-byte input buffer as `[tag, digest_bytes...]`, appends the
    /// 8-byte little-endian encoding of `number`, then Blake2b-256 hashes the
    /// result to produce the 32-byte address.
    pub fn derive(digest: Digest, tag: u8, number: u64) -> Self {
        use blake2::Digest;

        let mut hasher = Blake2b::<U32>::default();

        hasher.update([tag]);
        hasher.update(digest);
        hasher.update(number.to_le_bytes());

        Address::new(hasher.finalize().into())
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", prefix_hex::encode(self.0))
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", prefix_hex::encode(self.0))
    }
}

impl FromStr for Address {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Self> {
        let bytes: Vec<u8> = prefix_hex::decode(s)?;

        if bytes.len() < ADDRESS_LENGTH {
            // Accept shortened even-length hex forms like `0x42` by left-padding with zeros.
            let mut padded = vec![0u8; ADDRESS_LENGTH - bytes.len()];
            padded.extend_from_slice(&bytes);
            return Address::try_from(padded.as_slice());
        }

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
        use blake2::Digest;

        let mut hasher = Blake2b::<U32>::default();

        hasher.update([public_key.scheme().flag()]);
        hasher.update(&public_key);

        Address::new(hasher.finalize().into())
    }
}

impl From<meow_vm_types::address::Address> for Address {
    fn from(address: meow_vm_types::address::Address) -> Self {
        let bytes: [u8; ADDRESS_LENGTH] = address.into();
        Address::from(bytes)
    }
}

impl From<Address> for meow_vm_types::address::Address {
    fn from(address: Address) -> Self {
        let bytes: [u8; ADDRESS_LENGTH] = address.into();
        meow_vm_types::address::Address::from(bytes)
    }
}

impl From<Address> for [u8; ADDRESS_LENGTH] {
    fn from(address: Address) -> [u8; ADDRESS_LENGTH] {
        address.0
    }
}

impl From<[u8; ADDRESS_LENGTH]> for Address {
    fn from(bytes: [u8; ADDRESS_LENGTH]) -> Address {
        Address::new(bytes)
    }
}

impl TryFrom<&[u8]> for Address {
    type Error = AddressError;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        <[u8; ADDRESS_LENGTH]>::try_from(bytes)
            .map_err(|_| AddressError::InvalidLength {
                actual: bytes.len(),
                expected: ADDRESS_LENGTH,
            })
            .map(Address)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
