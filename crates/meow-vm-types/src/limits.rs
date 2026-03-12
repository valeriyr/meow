/// Maximum character length of any identifier (module, function, struct, field, or variable name).
pub const MAX_IDENTIFIER_LEN: usize = 128;

/// Maximum number of struct/object definitions in a module.
pub const MAX_STRUCTS: usize = 128;

/// Maximum number of functions in a module.
pub const MAX_FUNCTIONS: usize = 256;

/// Maximum number of fields in a struct or object definition.
pub const MAX_FIELDS: usize = 32;

/// Maximum number of parameters in a function.
pub const MAX_PARAMS: usize = 16;

/// Maximum number of local variable slots in a function (parameters + `let` bindings).
///
/// Also bounded by the `u8` slot index used in bytecode.
pub const MAX_LOCALS: usize = 255;

/// Maximum number of bytecode instructions in a single function.
pub const MAX_CODE_SIZE: usize = 65_536;

/// Maximum VM call stack depth.
pub const MAX_CALL_DEPTH: usize = 256;
