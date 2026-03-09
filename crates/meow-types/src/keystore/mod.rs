pub mod error;

mod file_based_keystore;
mod in_memory_keystore;

use std::path::Path;

use error::KeystoreError;
use file_based_keystore::FileBasedKeystore;
use in_memory_keystore::InMemoryKeystore;

use crate::{address::Address, keypair::KeyPair};

/// The result type related to keystores.
pub type Result<T> = std::result::Result<T, KeystoreError>;

/// The keystore type.
pub enum Keystore {
    FileBased(FileBasedKeystore),
    InMemory(InMemoryKeystore),
}

impl Keystore {
    /// Loads a keystore from the file.
    pub fn file_based(path: &impl AsRef<Path>) -> Result<Self> {
        Ok(Keystore::FileBased(FileBasedKeystore::load(path)?))
    }

    /// Creates an empty in-memory keystore.
    pub fn in_memory() -> Self {
        Keystore::InMemory(InMemoryKeystore::default())
    }

    /// Adds a key to the keystore.
    pub fn add_key(&mut self, keypair: KeyPair) -> Result<()> {
        match self {
            Keystore::FileBased(keystore) => keystore.add_key(keypair),
            Keystore::InMemory(keystore) => keystore.add_key(keypair),
        }
    }

    /// Gets a key from the keystore.
    pub fn get_key(&self, address: &Address) -> Option<&KeyPair> {
        match self {
            Keystore::FileBased(keystore) => keystore.get_key(address),
            Keystore::InMemory(keystore) => keystore.get_key(address),
        }
    }

    /// Removes a key from the keystore.
    pub fn remove_key(&mut self, address: &Address) -> Result<Option<KeyPair>> {
        match self {
            Keystore::FileBased(keystore) => keystore.remove_key(address),
            Keystore::InMemory(keystore) => keystore.remove_key(address),
        }
    }

    /// Gets an iterator over the keys, sorted by address.
    pub fn iter(&self) -> impl Iterator<Item = (&Address, &KeyPair)> {
        let elements: Vec<_> = match self {
            Keystore::FileBased(keystore) => keystore.iter().collect(),
            Keystore::InMemory(keystore) => keystore.iter().collect(),
        };

        elements.into_iter()
    }
}
