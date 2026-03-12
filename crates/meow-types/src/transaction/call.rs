use serde::{Deserialize, Serialize};

use crate::{address::Address, identifier::Identifier, object::object_ref::ObjectRef};

/// The input of a transaction call.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Wraps an `ObjectRef` referring to an existing on-chain object passed by reference.
    Object(ObjectRef),
    /// Contains BCS-serialized bytes representing a plain value argument (not an object).
    Raw(Vec<u8>),
}

/// The Meow VM call which can be done within a transaction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The module address.
    module: Address,
    /// The function name.
    function: Identifier,
    /// The call arguments.
    arguments: Vec<Input>,
}

impl Call {
    /// Creates a new call.
    pub fn new(module: Address, function: Identifier, arguments: Vec<Input>) -> Self {
        Self {
            module,
            function,
            arguments,
        }
    }

    /// Returns the module address.
    pub fn module(&self) -> &Address {
        &self.module
    }

    /// Returns the function name.
    pub fn function(&self) -> &Identifier {
        &self.function
    }

    /// Returns the call arguments.
    pub fn arguments(&self) -> &[Input] {
        &self.arguments
    }
}
