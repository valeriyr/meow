use serde::{Deserialize, Serialize};

use crate::{address::Address, object::identifier::Identifier};

/// The type of an object reference.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ObjectDeclRef {
    /// The address of the module which contains the object definition.
    module: Address,
    /// The object name.
    name: Identifier,
}

impl ObjectDeclRef {
    /// Creates a new object declaration reference.
    pub fn new(module: Address, name: Identifier) -> Self {
        Self { module, name }
    }

    /// Returns the module address.
    pub fn module(&self) -> &Address {
        &self.module
    }

    /// Returns the object name.
    pub fn name(&self) -> &Identifier {
        &self.name
    }
}
