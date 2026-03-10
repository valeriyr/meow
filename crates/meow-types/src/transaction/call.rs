use serde::{Deserialize, Serialize};

use crate::{address::Address, object::identifier::Identifier};

/// The input of a transaction call.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Input is an object instance stored on-chain; identified by the address.
    Object(Address),
    /// Input is a raw BCS serialized data.
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
