use std::{fs, path::Path};

use crate::{address::Address, keypair::KeyPair};

use super::{Result, in_memory_keystore::InMemoryKeystore};

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
    pub fn add_key(&mut self, keypair: KeyPair) -> Result<()> {
        let address = keypair.public().into();
        self.inner.add_key(keypair)?;
        if let Err(e) = self.save() {
            // Roll back the in-memory add so memory and disk stay in sync.
            let _ = self.inner.remove_key(&address);
            return Err(e);
        }
        Ok(())
    }

    /// Gets a key from the keystore.
    pub fn get_key(&self, address: &Address) -> Option<&KeyPair> {
        self.inner.get_key(address)
    }

    /// Removes a key from the keystore.
    pub fn remove_key(&mut self, address: &Address) -> Result<Option<KeyPair>> {
        let removed = self.inner.remove_key(address)?;
        match self.save() {
            Ok(()) => Ok(removed),
            Err(e) => {
                // Roll back the in-memory remove so memory and disk stay in sync.
                if let Some(keypair) = removed {
                    let _ = self.inner.add_key(keypair);
                }
                Err(e)
            }
        }
    }

    /// Gets an iterator over the keys, sorted by address.
    pub fn iter(&self) -> impl Iterator<Item = (&Address, &KeyPair)> {
        self.inner.iter()
    }

    /// Saves the keystore to the filesystem.
    fn save(&self) -> Result<()> {
        let contents = serde_json::to_string_pretty(&self.inner)?;

        fs::write(&*self.path, contents)?;
        Ok(())
    }
}
