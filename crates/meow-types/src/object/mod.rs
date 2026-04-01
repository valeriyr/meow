pub mod object_conversion;
pub mod object_decl_ref;
pub mod object_owner;
pub mod object_ref;
pub mod object_type;
pub mod object_version;

use serde::{Deserialize, Serialize};

use crate::{
    address::Address,
    digest::Digest,
    object::{
        object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_ref::ObjectRef,
        object_type::ObjectType, object_version::ObjectVersion,
    },
};

/// The meow object type.
/// Acts as UTXO in the meow world, and is the only way to store data on-chain.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// The object address.
    /// The address is unique across all objects and remains the same throughout the object's lifetime.
    address: Address,
    /// The object owner.
    /// The owner is the address that has control over the object.
    owner: ObjectOwner,
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
        owner: ObjectOwner,
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

    /// Creates a fresh object.
    pub fn fresh_object(
        address: Address,
        owner: Address,
        transaction: Digest,
        object_decl_ref: ObjectDeclRef,
        content: Vec<u8>,
    ) -> Self {
        Self {
            address,
            owner: ObjectOwner::Address(owner),
            transaction,
            version: ObjectVersion::ONE,
            type_: ObjectType::Object(object_decl_ref),
            content,
        }
    }

    /// Creates a fresh module object.
    pub fn fresh_module(address: Address, transaction: Digest, content: Vec<u8>) -> Self {
        Self {
            address,
            owner: ObjectOwner::Immutable,
            transaction,
            version: ObjectVersion::ONE,
            type_: ObjectType::Module,
            content,
        }
    }

    /// Returns the object address.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Returns the object owner.
    pub fn owner(&self) -> &ObjectOwner {
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
        Digest::compute(self).expect("Failed to compute an object digest")
    }

    /// Returns the object reference.
    pub fn object_ref(&self) -> ObjectRef {
        ObjectRef::new(self.address, self.version.clone(), self.digest())
    }
}
