/// An error related to keypairs.
#[derive(Debug, thiserror::Error)]
pub enum AddressError {
    #[error("prefix hex error: {0}")]
    PrefixHexError(#[from] prefix_hex::Error),
    #[error("invalid address bytes length: actual {actual}, expected {expected}")]
    InvalidAddressBytesLength { actual: usize, expected: usize },
}
