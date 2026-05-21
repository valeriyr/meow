//! In-memory keystore for ephemeral key pair storage.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Result, error::KeystoreError};
use crate::{address::Address, keypair::KeyPair};

/// The in-memory keystore type.
///
/// Stores keys in memory.
#[derive(Default, Debug)]
pub struct InMemoryKeystore {
    keys: BTreeMap<Address, KeyPair>,
}

impl InMemoryKeystore {
    /// Adds a key to the keystore.
    pub fn add_key(&mut self, keypair: KeyPair) -> Result<()> {
        let address = keypair.public().into();

        if self.keys.contains_key(&address) {
            return Err(KeystoreError::KeyPairAlreadyExists(address));
        }

        self.keys.insert(address, keypair);
        Ok(())
    }

    /// Gets a key from the keystore.
    pub fn get_key(&self, address: &Address) -> Option<&KeyPair> {
        self.keys.get(address)
    }

    /// Removes a key from the keystore.
    pub fn remove_key(&mut self, address: &Address) -> Result<Option<KeyPair>> {
        Ok(self.keys.remove(address))
    }

    /// Gets an iterator over the keys, sorted by address.
    pub fn iter(&self) -> impl Iterator<Item = (&Address, &KeyPair)> {
        self.keys.iter()
    }
}

impl Serialize for InMemoryKeystore {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let keys = self.keys.values().collect::<Vec<_>>();
        keys.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for InMemoryKeystore {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let mut keystore = Self::default();

        let keys = Vec::<KeyPair>::deserialize(deserializer)?;
        for key in keys {
            keystore.add_key(key).map_err(D::Error::custom)?;
        }
        Ok(keystore)
    }
}
