use std::fmt;

use serde::{Deserialize, Serialize};

use crate::object::identifier::error::IdentifierError;

pub mod error;

/// The result type related to transactions.
pub type Result<T> = std::result::Result<T, IdentifierError>;

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Identifier(String);

impl Identifier {
    /// Creates a new identifier.
    pub fn new(identifier: impl Into<String>) -> Result<Self> {
        let identifier = identifier.into();
        if Self::is_valid(&identifier) {
            Ok(Self(identifier))
        } else {
            Err(IdentifierError::InvalidIdentifier(identifier))
        }
    }

    /// Validates the identifier string.
    ///
    /// A valid identifier starts with an ASCII letter or underscore, followed by
    /// zero or more ASCII letters, digits, or underscores.
    fn is_valid(name: &str) -> bool {
        let mut chars = name.chars();
        match chars.next() {
            Some(first) if first.is_ascii_alphabetic() || first == '_' => {
                chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            }
            _ => false,
        }
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
