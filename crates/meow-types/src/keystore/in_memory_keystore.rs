use std::collections::BTreeMap;

use serde::{ser::SerializeSeq, Serialize, Serializer};

use super::{error::KeystoreError, Result};
use crate::{address::Address, keypair::KeyPair};

/// The in-memory keystore type.
///
/// Stores keys in memory.
#[derive(Debug)]
pub struct InMemoryKeystore {
    keys: BTreeMap<Address, KeyPair>,
}

impl InMemoryKeystore {
    /// Adds a key to the keystore.
    ///
    /// # Errors
    /// - [KeystoreError::KeyPairAlreadyExists] if the key already exists.
    pub fn add_key(&mut self, keypair: KeyPair) -> Result<()> {
        let address = keypair.public().into();

        if self.keys.contains_key(&address) {
            return Err(KeystoreError::KeyPairAlreadyExists(keypair));
        }

        self.keys.insert(address, keypair);
        Ok(())
    }
}

impl Default for InMemoryKeystore {
    fn default() -> Self {
        Self {
            keys: BTreeMap::new(),
        }
    }
}

impl Serialize for InMemoryKeystore {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let keys = self.keys.values().collect::<Vec<_>>();

        let mut seq = serializer.serialize_seq(Some(keys.len()))?;
        for key in keys {
            seq.serialize_element(key)?;
        }
        seq.end()
    }
}
