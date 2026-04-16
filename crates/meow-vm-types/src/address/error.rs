/// An error related to addresses.
#[derive(Debug, thiserror::Error)]
pub enum AddressError {
    #[error("prefix hex error: {0}")]
    PrefixHexError(#[from] prefix_hex::Error),
    #[error("invalid address bytes length: actual {actual}, expected {expected}")]
    InvalidLength { actual: usize, expected: usize },
}
