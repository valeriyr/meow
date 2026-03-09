use crate::object::{object_id::ObjectId, object_version::ObjectVersion};

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum ObjectType {
    StructDeclaration,
    StructInstance,
}

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Object {
    /// The object ID.
    id: ObjectId,
    /// The object version.
    version: ObjectVersion,
    /// The object type.
    type_: ObjectType,
    /// The object content.
    content: Vec<u8>,
}

impl Object {
    /// Creates a new object.
    pub fn new(id: ObjectId, version: ObjectVersion, type_: ObjectType, content: Vec<u8>) -> Self {
        Self {
            id,
            version,
            type_,
            content,
        }
    }

    /// Returns the object ID.
    pub fn id(&self) -> &ObjectId {
        &self.id
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
