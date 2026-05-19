//! VM runtime error type.

/// Errors that can occur during VM execution.
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("out of gas: spent {spent}, limit {limit}")]
    OutOfGas { spent: u64, limit: u64 },

    #[error("stack underflow")]
    StackUnderflow,

    #[error("type error: {0}")]
    TypeError(String),

    #[error("undefined variable at slot {0}")]
    UndefinedVariable(u8),

    #[error("undefined function: {0}")]
    UndefinedFunction(String),

    #[error("undefined struct: {0}")]
    UndefinedStruct(String),

    #[error("undefined field '{field}' on struct '{type_name}'")]
    UndefinedField { type_name: String, field: String },

    #[error("division by zero")]
    DivisionByZero,

    #[error("call stack overflow (max depth {0})")]
    CallStackOverflow(usize),

    #[error("too many dependency modules (max {0})")]
    TooManyDepModules(usize),

    #[error("use after move: {0}")]
    UseAfterMove(String),

    #[error("aborted with code {code}: {message}")]
    Aborted { code: u64, message: String },

    #[error("native function error: {0}")]
    NativeError(String),

    #[error("function '{0}' is private — only `pub fn` can be called from outside the module")]
    PrivateFunction(String),

    #[error("'{0}' is a native function and cannot be called directly from outside a contract")]
    NativeFunctionCallDirect(String),
}
