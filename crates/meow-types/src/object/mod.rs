use serde::{Deserialize, Serialize};

use crate::{
    address::Address,
    digest::Digest,
    object::{object_type::ObjectType, object_version::ObjectVersion},
};

pub mod error;
pub mod identifier;
pub mod object_decl_ref;
pub mod object_type;
pub mod object_version;

/// The meow object type.
/// Acts as UTXO in the meow world, and is the only way to store data on-chain.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// The object address.
    /// The address is unique across all objects and remains the same throughout the object's lifetime.
    address: Address,
    /// The object owner.
    /// The owner is the address that has control over the object.
    owner: Address,
    /// The digest of the transaction that created or last mutated the object.
    transaction: Digest,
    /// The object version.
    version: ObjectVersion,
    /// The object type.
    type_: ObjectType,
    /// The object content.
    content: Vec<u8>,
}

impl Object {
    /// Creates a new object.
    pub fn new(
        address: Address,
        owner: Address,
        transaction: Digest,
        version: ObjectVersion,
        type_: ObjectType,
        content: Vec<u8>,
    ) -> Self {
        Self {
            address,
            owner,
            transaction,
            version,
            type_,
            content,
        }
    }

    /// Returns the object address.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Returns the object owner.
    pub fn owner(&self) -> &Address {
        &self.owner
    }

    /// Returns the object version.
    pub fn version(&self) -> &ObjectVersion {
        &self.version
    }

    /// Returns the object type.
    pub fn type_(&self) -> &ObjectType {
        &self.type_
    }

    /// Returns the object content.
    pub fn content(&self) -> &[u8] {
        &self.content
    }

    /// Returns the object digest.
    pub fn digest(&self) -> Digest {
        Digest::compute(self).expect("Failed to compute a transaction digest")
    }
}
