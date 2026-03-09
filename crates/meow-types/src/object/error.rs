/// An error related to object IDs.
#[derive(Debug, thiserror::Error)]
pub enum ObjectIdError {
    #[error("address error: {0}")]
    AddressError(#[from] crate::address::error::AddressError),
}
