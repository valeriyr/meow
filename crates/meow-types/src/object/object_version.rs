use std::fmt;

/// The version of an object.
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct ObjectVersion(u64);

impl ObjectVersion {
    /// The zero object version.
    pub const ZERO: Self = Self(0);

    /// Creates a new object version which is the next version of the current one.
    pub fn next(&self) -> ObjectVersion {
        ObjectVersion(self.0 + 1)
    }
}

impl fmt::Display for ObjectVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
