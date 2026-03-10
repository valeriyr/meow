use serde::{Deserialize, Serialize};

use crate::object::object_decl_ref::ObjectDeclRef;

/// The type of an object.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ObjectType {
    /// Object is a module stored on-chain.
    Module,
    /// Object is an instance stored on-chain.
    Object(ObjectDeclRef),
}
