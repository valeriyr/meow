use std::{fs, path::Path};

use crate::{address::Address, keypair::KeyPair};

use super::{in_memory_keystore::InMemoryKeystore, Result};

/// The file-based keystore type.
///
/// Stores keys on the filesystem.
/// Uses the in-memory keystore internally to store the keys.
pub struct FileBasedKeystore {
    inner: InMemoryKeystore,
    path: Box<dyn AsRef<Path>>,
}

impl FileBasedKeystore {
    /// Loads a keystore from the file.
    ///
    /// # Errors
    /// - [KeystoreError::IoError] if the file cannot be written.
    /// - [KeystoreError::SerdeJsonError] if the key cannot be serialized.
    pub fn load(path: &impl AsRef<Path>) -> Result<Self> {
        let inner = if path.as_ref().exists() {
            let content = fs::read(path)?;
            serde_json::from_slice::<InMemoryKeystore>(&content)?
        } else {
            InMemoryKeystore::default()
        };

        Ok(Self {
            inner,
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
        self.inner.add_key(keypair)?;
        self.save()?;
        Ok(())
    }

    /// Gets a key from the keystore.
    pub fn get_key(&self, address: &Address) -> Option<&KeyPair> {
        self.inner.get_key(address)
    }

    /// Gets an iterator over the keys, sorted by address.
    pub fn iter(&self) -> impl Iterator<Item = (&Address, &KeyPair)> {
        self.inner.iter()
    }

    /// Saves the keystore to the filesystem.
    ///
    /// # Errors
    /// - [KeystoreError::IoError] if the file cannot be written.
    /// - [KeystoreError::SerdeJsonError] if the key cannot be serialized.
    fn save(&self) -> Result<()> {
        let contents = serde_json::to_string_pretty(&self.inner)?;

        fs::write(&*self.path, contents)?;
        Ok(())
    }
}
