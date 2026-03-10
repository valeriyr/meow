use std::fmt;

use serde::{Deserialize, Serialize};

/// The version of an object.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct ObjectVersion(u64);

impl ObjectVersion {
    /// The zero object version.
    pub const ZERO: Self = Self(0);
    /// The maximum object version.
    pub const MAX: Self = Self(u64::MAX);

    /// Creates a new object version which is the next version of the current one.
    ///
    /// Returns `None` if the current version is the maximum version.
    pub fn next(&self) -> Option<ObjectVersion> {
        if self == &Self::MAX {
            None
        } else {
            Some(Self(self.0 + 1))
        }
    }
}

impl fmt::Display for ObjectVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
