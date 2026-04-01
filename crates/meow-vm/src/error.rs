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

    /// Attempted to use a variable whose Object value was already moved out.
    #[error("use after move: {0}")]
    UseAfterMove(String),

    /// Execution was aborted by meow_vm_abort.
    #[error("aborted with code {code}: {message}")]
    Aborted { code: u64, message: String },

    /// A native function returned an error (e.g. wrong argument type).
    #[error("native function error: {0}")]
    NativeError(String),

    /// A native function expected an Object argument but received something else.
    #[error("expected object argument: {0}")]
    ObjectRequired(String),

    /// An object is missing the required `id: address` first field.
    #[error("invalid object definition '{0}': first field must be 'id: address'")]
    InvalidObject(String),
}
