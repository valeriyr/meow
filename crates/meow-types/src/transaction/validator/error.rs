use crate::address::Address;

/// An error produced by transaction validation.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("object {0} appears more than once in call arguments")]
    AliasedCallArgument(Address),
    #[error("gas coin {0} cannot be used as a call argument")]
    GasCoinUsedAsCallArgument(Address),
    #[error("module is too large: {size} bytes (limit: {limit})")]
    ModuleTooLarge { size: usize, limit: usize },
    #[error("too many call arguments: {amount} (limit: {limit})")]
    TooManyCallArguments { amount: usize, limit: usize },
    #[error("transaction is too large: {size} bytes (limit: {limit})")]
    TransactionTooLarge { size: usize, limit: usize },
}
