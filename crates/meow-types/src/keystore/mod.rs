pub mod error;

mod file_based_keystore;
mod in_memory_keystore;

use error::KeystoreError;
use file_based_keystore::FileBasedKeystore;
use in_memory_keystore::InMemoryKeystore;

use crate::keypair::KeyPair;

/// The result type related to keystores.
pub type Result<T> = std::result::Result<T, KeystoreError>;

/// The keystore type.
pub enum Keystore {
    FileBased(FileBasedKeystore),
    InMemory(InMemoryKeystore),
}

impl Keystore {
    /// Adds a key to the keystore.
    ///
    /// # Errors
    /// - [KeystoreError::KeyPairAlreadyExists] if the key already exists.
    pub fn add_key(&mut self, keypair: KeyPair) -> Result<()> {
        match self {
            Keystore::FileBased(keystore) => keystore.add_key(keypair),
            Keystore::InMemory(keystore) => keystore.add_key(keypair),
        }
    }
}
