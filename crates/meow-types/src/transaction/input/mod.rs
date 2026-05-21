//! Transaction call argument: an object reference or a raw BCS-encoded primitive value.

pub mod error;

use serde::{Deserialize, Serialize};

use crate::{object::object_ref::ObjectRef, transaction::input::error::InputError};

/// The result type for transaction input.
pub type Result<T> = std::result::Result<T, InputError>;

/// The input of a transaction call.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Wraps an `ObjectRef` referring to an existing on-chain object passed by reference.
    Object(ObjectRef),
    /// Contains BCS-serialized bytes representing a plain value argument (not an object).
    Raw(Vec<u8>),
}

impl Input {
    /// Creates a new `Input` from a plain value by BCS-serializing it.
    pub fn raw<T: Serialize>(value: &T) -> Result<Self> {
        Ok(Self::Raw(bcs::to_bytes(value)?))
    }
}
