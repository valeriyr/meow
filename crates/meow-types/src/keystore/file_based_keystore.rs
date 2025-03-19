use std::{fs, path::Path};

use crate::keypair::KeyPair;

use super::{in_memory_keystore::InMemoryKeystore, Result};

/// The file-based keystore type.
///
/// Stores keys on the filesystem.
/// Uses an in-memory a a base implementation.
pub struct FileBasedKeystore {
    in_memory_store: InMemoryKeystore,
    path: Box<dyn AsRef<Path>>,
}

impl FileBasedKeystore {
    /// Loads a keystore from the file.
    ///
    /// # Errors
    /// - [KeystoreError::IoError] if the file cannot be written.
    /// - [KeystoreError::SerdeJsonError] if the key cannot be serialized.
    pub fn load(path: &impl AsRef<Path>) -> Result<Self> {
        let in_memory_store = if path.as_ref().exists() {
            let content = fs::read(path)?;
            serde_json::from_slice::<InMemoryKeystore>(&content)?
        } else {
            InMemoryKeystore::default()
        };

        Ok(Self {
            in_memory_store,
            path: Box::new(path.as_ref().to_owned()),
        })
    }

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

        fs::write(&*self.path, contents)?;
        Ok(())
    }
}
