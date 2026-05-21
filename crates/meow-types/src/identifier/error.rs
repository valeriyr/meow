//! Error type for identifier validation.

/// An error related to identifiers.
#[derive(Debug, thiserror::Error)]
pub enum IdentifierError {
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
}
