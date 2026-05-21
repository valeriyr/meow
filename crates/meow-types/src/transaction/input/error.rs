//! Error type for transaction input encoding.

/// An error produced by transaction input.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("bcs error: {0}")]
    BcsError(#[from] bcs::Error),
}
