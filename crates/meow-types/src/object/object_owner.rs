use serde::{Deserialize, Serialize};

use crate::address::Address;

/// The owner of an object.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ObjectOwner {
    /// Object is owned by a specific address.
    Address(Address),
    /// Object is immutable and cannot be changed.
    Immutable,
}

impl ObjectOwner {
    /// Returns the address of the owner if it is an address, or `None` if it is immutable.
    pub fn address(&self) -> Option<&Address> {
        match self {
            ObjectOwner::Address(addr) => Some(addr),
            ObjectOwner::Immutable => None,
        }
    }

    /// Returns `true` if the object is address owned, or `false` if it is immutable.
    pub fn is_address_owned(&self) -> bool {
        matches!(self, ObjectOwner::Address(_))
    }

    /// Returns `true` if the object is immutable, or `false` if it is owned by an address.
    pub fn is_immutable(&self) -> bool {
        matches!(self, ObjectOwner::Immutable)
    }
}
