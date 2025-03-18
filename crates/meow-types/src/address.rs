use std::fmt;

use blake2::{
    digest::{consts::U32, generic_array::GenericArray},
    Blake2b, Digest,
};

use crate::keypair::{public_key::PublicKey, KeyPair};

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
        write!(f, "0x{}", hex::encode(self.0))
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
