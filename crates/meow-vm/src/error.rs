//! VM runtime error type.

/// Errors that can occur during VM execution.
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("aborted with code {code}: {message}")]
    Aborted { code: u64, message: String },

    #[error("arithmetic overflow")]
    ArithmeticOverflow,

    #[error(
        "function '{function}' received {got} arguments but declares only {local_count} local slots"
    )]
    ArityMismatch {
        function: String,
        got: usize,
        local_count: usize,
    },

    #[error("call stack overflow (max depth {0})")]
    CallStackOverflow(usize),

    #[error("division by zero")]
    DivisionByZero,

    #[error(
        "'{0}': struct types and tuples containing structs cannot be compared with == or != — destructure and compare fields individually"
    )]
    EqOnLinearType(String),

    #[error("invalid jump target: offset {offset} from pc {pc} is out of range")]
    InvalidJumpTarget { pc: usize, offset: i32 },

    #[error("store-field would overwrite live struct field '{0}'; consume it before storing")]
    LinearFieldOverwrite(String),

    #[error("native function error: {0}")]
    NativeError(String),

    #[error("'{0}' is a native function and cannot be called directly from outside a contract")]
    NativeFunctionCallDirect(String),

    #[error("out of gas: spent {spent}, limit {limit}")]
    OutOfGas { spent: u64, limit: u64 },

    #[error("function '{0}' is private — only `pub fn` can be called from outside the module")]
    PrivateFunction(String),

    #[error("slot overwrite: slot {0} holds a live struct; consume it before storing")]
    SlotOverwrite(u8),

    #[error("stack underflow")]
    StackUnderflow,

    #[error("too many dependency modules (max {0})")]
    TooManyDepModules(usize),

    #[error("type error: {0}")]
    TypeError(String),

    #[error("undefined field '{field}' on struct '{type_name}'")]
    UndefinedField { type_name: String, field: String },

    #[error("undefined function: {0}")]
    UndefinedFunction(String),

    #[error("undefined struct: {0}")]
    UndefinedStruct(String),

    #[error("undefined variable at slot {0}")]
    UndefinedVariable(u8),

    #[error("use after move: {0}")]
    UseAfterMove(String),
}
