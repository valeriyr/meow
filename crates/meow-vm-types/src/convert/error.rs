//! Error type for BCS-based VM value conversion.

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("bcs error: {0}")]
    BcsError(#[from] bcs::Error),
    #[error("unsupported type: {0}")]
    UnsupportedType(String),
}

impl serde::ser::Error for ConversionError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        ConversionError::UnsupportedType(msg.to_string())
    }
}
