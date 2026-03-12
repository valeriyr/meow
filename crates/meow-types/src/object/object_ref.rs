use serde::{Deserialize, Serialize};

use crate::{address::Address, digest::Digest, object::object_version::ObjectVersion};

/// The type of an object reference.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ObjectRef {
    /// The object address.
    address: Address,
    /// The object version.
    version: ObjectVersion,
    /// The object digest.
    digest: Digest,
}

impl ObjectRef {
    /// Creates a new object reference.
    pub fn new(address: Address, version: ObjectVersion, digest: Digest) -> Self {
        Self {
            address,
            version,
            digest,
        }
    }

    /// Returns the object address.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Returns the object version.
    pub fn version(&self) -> &ObjectVersion {
        &self.version
    }

    /// Returns the object digest.
    pub fn digest(&self) -> &Digest {
        &self.digest
    }
}
