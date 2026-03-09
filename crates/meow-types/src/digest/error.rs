/// An error related to digests.
#[derive(Debug, thiserror::Error)]
pub enum DigestError {
    #[error("base58 error: {0}")]
    Base58Error(#[from] bs58::decode::Error),
    #[error("bcs error: {0}")]
    BcsError(#[from] bcs::Error),
    #[error("invalid digest bytes length: actual {actual}, expected {expected}")]
    InvalidDigestBytesLength { actual: usize, expected: usize },
}
