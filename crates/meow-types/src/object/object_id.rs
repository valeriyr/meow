use std::{fmt, str::FromStr};

use crate::{address::Address, object::error::ObjectIdError};

/// The result type related to object IDs.
pub type Result<T> = std::result::Result<T, ObjectIdError>;

/// The meow object ID type.
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct ObjectId(Address);

impl ObjectId {
    /// The zero object ID constant.
    pub const ZERO: Self = Self(Address::ZERO);

    /// Creates a new object ID.
    pub fn new(id: Address) -> Self {
        Self(id)
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ObjectId {
    type Err = ObjectIdError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(ObjectId(Address::from_str(s)?))
    }
}

impl TryFrom<&[u8]> for ObjectId {
    type Error = ObjectIdError;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        Ok(ObjectId(Address::try_from(bytes)?))
    }
}
