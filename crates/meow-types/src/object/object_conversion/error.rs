/// An error related to identifiers.
#[derive(Debug, thiserror::Error)]
pub enum ObjectConversionError {
    #[error("invalid object type")]
    InvalidObjectType,
    #[error("invalid VM value type")]
    InvalidVMValueType,
}
