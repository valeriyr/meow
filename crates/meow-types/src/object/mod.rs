use crate::{address::Address, object::object_version::ObjectVersion};

pub mod error;
pub mod object_version;

/// The type of an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectType {
    StructDeclaration,
    StructInstance,
}

/// The meow object type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// The object address.
    address: Address,
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
        version: ObjectVersion,
        type_: ObjectType,
        content: Vec<u8>,
    ) -> Self {
        Self {
            address,
            version,
            type_,
            content,
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

    /// Returns the object type.
    pub fn type_(&self) -> &ObjectType {
        &self.type_
    }

    /// Returns the object content.
    pub fn content(&self) -> &[u8] {
        &self.content
    }
}
