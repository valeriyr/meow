/// An error that can occur when converting between chain objects and VM values.
#[derive(Debug, thiserror::Error)]
pub enum ObjectConversionError {
    #[error("invalid object type")]
    InvalidObjectType,
    #[error("invalid VM value type")]
    InvalidVMValueType,
    #[error("type name is not a qualified address::name reference")]
    InvalidTypeName,
    #[error("object struct is missing the required id field")]
    MissingIdField,
}
