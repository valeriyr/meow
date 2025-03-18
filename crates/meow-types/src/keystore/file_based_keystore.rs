use std::{fs, path::PathBuf};

use crate::keypair::KeyPair;

use super::{in_memory_keystore::InMemoryKeystore, Result};

/// The file-based keystore type.
///
/// Stores keys on the filesystem.
/// Uses an in-memory a a base implementation.
pub struct FileBasedKeystore {
    in_memory_store: InMemoryKeystore,
    path: PathBuf,
}

impl FileBasedKeystore {
    /// Adds a key to the keystore.
    ///
    /// # Errors
    /// - [KeystoreError::IoError] if the file cannot be written.
    /// - [KeystoreError::KeyPairAlreadyExists] if the key already exists.
    /// - [KeystoreError::SerdeJsonError] if the key cannot be serialized.
    pub fn add_key(&mut self, keypair: KeyPair) -> Result<()> {
        self.in_memory_store.add_key(keypair)?;
        self.save()?;
        Ok(())
    }

    /// Saves the keystore to the filesystem.
    ///
    /// # Errors
    /// - [KeystoreError::IoError] if the file cannot be written.
    /// - [KeystoreError::SerdeJsonError] if the key cannot be serialized.
    fn save(&self) -> Result<()> {
        let contents = serde_json::to_string(&self.in_memory_store)?;

        fs::write(&self.path, contents)?;
        Ok(())
    }
}

impl Default for FileBasedKeystore {
    fn default() -> Self {
        Self {
            in_memory_store: Default::default(),
            path: Default::default(),
        }
    }
}
