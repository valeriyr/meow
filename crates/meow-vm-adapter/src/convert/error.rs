/// An error related to identifiers.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("invalid object type")]
    InvalidObjectType,
    #[error("invalid VM value type")]
    InvalidVMValueType,
}
