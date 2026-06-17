//! Identifier type that enforces naming rules at construction time.
//!
//! Validating at construction time means invalid names are rejected early and cleanly,
//! rather than causing confusing failures deep in the compiler or VM.

use serde::{Deserialize, Serialize};

use crate::{config, identifier::error::IdentifierError};

pub mod error;

/// The result type related to transactions.
pub type Result<T> = std::result::Result<T, IdentifierError>;

#[derive(Serialize, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
        let config = config::compiler_config();
        meow_vm_types::identifier::is_valid_identifier(name, &config)
    }
}

impl<'de> Deserialize<'de> for Identifier {
    /// Deserialization routes through [`Identifier::new`] so the validity invariant
    /// (held at construction) is preserved for values arriving over the wire. Without
    /// this, a deserialized transaction could carry an empty, oversized, or otherwise
    /// malformed identifier that bypassed all the construction-time checks.
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Identifier::new(raw).map_err(serde::de::Error::custom)
    }
}

impl TryFrom<&str> for Identifier {
    type Error = IdentifierError;

    fn try_from(value: &str) -> Result<Self> {
        Identifier::new(value)
    }
}

impl std::str::FromStr for Identifier {
    type Err = IdentifierError;

    fn from_str(value: &str) -> Result<Self> {
        Identifier::new(value)
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
