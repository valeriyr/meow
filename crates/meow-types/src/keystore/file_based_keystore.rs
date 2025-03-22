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
        // TODO: Should we remove the keypair from the store if the save fails?
        self.save()
    }

    /// Gets a key from the keystore.
    pub fn get_key(&self, address: &Address) -> Option<&KeyPair> {
        self.inner.get_key(address)
    }

    /// Removes a key from the keystore.
    pub fn remove_key(&mut self, address: &Address) -> Result<Option<KeyPair>> {
        let result = self.inner.remove_key(address)?;
        // TODO: Should we add the keypair back if the save fails?
        self.save()?;
        Ok(result)
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
