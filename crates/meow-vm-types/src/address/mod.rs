//! 32-byte VM address type.

pub mod error;

use std::str::FromStr;

use error::AddressError;
use serde::{Deserialize, Serialize};

/// The result type related to addresses.
pub type Result<T> = std::result::Result<T, AddressError>;

/// The address length in bytes.
pub const ADDRESS_LENGTH: usize = 32;

/// A 32-byte VM address — the unique identifier for a module or object.
///
/// Freely copyable and comparable. Serializes as a raw `[u8; 32]` (no discriminant).
#[derive(Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq, Ord, PartialOrd)]
pub struct Address([u8; ADDRESS_LENGTH]);

impl Address {
    /// The zero address constant.
    pub const ZERO: Self = Self([0; ADDRESS_LENGTH]);

    /// Creates a new address from raw bytes.
    pub const fn new(bytes: [u8; ADDRESS_LENGTH]) -> Self {
        Self(bytes)
    }

    /// Creates an address with every byte set to `byte`.
    pub const fn fill(byte: u8) -> Self {
        Self([byte; ADDRESS_LENGTH])
    }

    /// Creates an address whose last two bytes hold `suffix` (big-endian) and all other bytes are zero.
    ///
    /// Used for well-known built-in module addresses (e.g. `Address::suffixed(0x0010)` for `meow_object`).
    pub const fn suffixed(suffix: u16) -> Self {
        let mut addr = [0u8; ADDRESS_LENGTH];
        let [hi, lo] = suffix.to_be_bytes();
        addr[ADDRESS_LENGTH - 2] = hi;
        addr[ADDRESS_LENGTH - 1] = lo;
        Self(addr)
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

    /// Parse a hex address with or without a `0x` prefix.
    ///
    /// Short forms like `0x42` are accepted and left-padded with zeros.
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
