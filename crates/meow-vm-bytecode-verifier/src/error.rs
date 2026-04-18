/// All errors the bytecode verifier can report.
///
/// Every variant carries the `function` name and, where applicable, the
/// 0-based instruction index (`pc`) for precise location reporting.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VerificationError {
    //
    // ─── Naming / structure ───
    //
    #[error("invalid identifier '{name}' in {context}")]
    InvalidIdentifier { name: String, context: String },

    #[error("duplicate function name '{name}'")]
    DuplicateFunctionName { name: String },

    #[error("duplicate struct name '{name}'")]
    DuplicateStructName { name: String },

    #[error("function '{function}': local_count {local_count} < param count {param_count}")]
    LocalCountTooSmall {
        function: String,
        local_count: u8,
        param_count: usize,
    },

    //
    // ─── Jump / slot bounds ───
    //
    #[error(
        "function '{function}' at pc {pc}: backward or zero jump offset {offset} — loops are not allowed"
    )]
    BackwardJump {
        function: String,
        pc: usize,
        offset: i32,
    },

    #[error(
        "function '{function}' at pc {pc}: jump target {target} is out of bounds (code length {code_len})"
    )]
    JumpOutOfBounds {
        function: String,
        pc: usize,
        target: usize,
        code_len: usize,
    },

    #[error(
        "function '{function}' at pc {pc}: local slot {slot} is out of range (local_count is {local_count})"
    )]
    SlotOutOfRange {
        function: String,
        pc: usize,
        slot: u8,
        local_count: u8,
    },

    //
    // ─── Struct / object shape ───
    //
    #[error("function '{function}' at pc {pc}: NewStruct references unknown type '{type_name}'")]
    UnknownStructType {
        function: String,
        pc: usize,
        type_name: String,
    },

    #[error(
        "function '{function}' at pc {pc}: NewStruct field list for '{type_name}' does not match definition"
    )]
    NewStructFieldMismatch {
        function: String,
        pc: usize,
        type_name: String,
    },

    #[error(
        "struct '{struct_name}': field '{field_name}' has Object type which is not a valid field type"
    )]
    ObjectAsFieldType {
        struct_name: String,
        field_name: String,
    },

    #[error("object '{struct_name}': first field must be 'id: address'")]
    ObjectMissingIdField { struct_name: String },

    //
    // ─── Visibility ───
    //
    #[error("function '{function}' at pc {pc}: call to '{callee}' which is private")]
    CrossModuleCallToPrivateFunction {
        function: String,
        pc: usize,
        callee: String,
    },

    #[error(
        "function '{function}' at pc {pc}: cross-module construction of type '{type_name}' is forbidden"
    )]
    CrossModuleStructConstruction {
        function: String,
        pc: usize,
        type_name: String,
    },

    #[error(
        "function '{function}' at pc {pc}: field '{field}' on cross-module type '{type_name}' is private"
    )]
    CrossModulePrivateFieldRead {
        function: String,
        pc: usize,
        type_name: String,
        field: String,
    },

    #[error(
        "function '{function}' at pc {pc}: cross-module field write to '{field}' on '{type_name}' is forbidden"
    )]
    CrossModuleFieldWrite {
        function: String,
        pc: usize,
        type_name: String,
        field: String,
    },

    //
    // ─── Type safety (abstract interpretation) ───
    //
    #[error("function '{function}' at pc {pc}: stack underflow — expected {expected}")]
    StackUnderflow {
        function: String,
        pc: usize,
        expected: String,
    },

    #[error("function '{function}' at pc {pc}: type mismatch — expected {expected}, found {found}")]
    TypeMismatch {
        function: String,
        pc: usize,
        expected: String,
        found: String,
    },

    #[error(
        "function '{function}': stack types at join point pc {join_pc} differ between branch paths"
    )]
    StackMergeConflict { function: String, join_pc: usize },

    #[error(
        "function '{function}': object liveness at join point pc {join_pc} differs between branch paths (slot {slot})"
    )]
    LivenessMergeConflict {
        function: String,
        join_pc: usize,
        slot: u8,
    },

    #[error("function '{function}': return type mismatch — declared {declared}, found {found}")]
    ReturnTypeMismatch {
        function: String,
        declared: String,
        found: String,
    },

    #[error("function '{function}': missing Return at end of function")]
    MissingReturn { function: String },

    //
    // ─── Object linearity ───
    //
    #[error("function '{function}' at pc {pc}: use-after-move of Object in slot {slot}")]
    UseAfterMove {
        function: String,
        pc: usize,
        slot: u8,
    },

    #[error(
        "function '{function}' at pc {pc}: Pop on Object value — objects must be transferred or destroyed"
    )]
    PopOnObject { function: String, pc: usize },

    #[error("function '{function}' at pc {pc}: Dup on Object value — objects have move semantics")]
    DupOnObject { function: String, pc: usize },

    #[error(
        "function '{function}' at pc {pc}: Store overwrites live Object in slot {slot} — consume the existing object first"
    )]
    ObjectSlotOverwrite {
        function: String,
        pc: usize,
        slot: u8,
    },

    #[error("function '{function}': Object in slot {slot} was not consumed before Return")]
    UnconsumedObject { function: String, slot: u8 },

    //
    // ─── Call resolution ───
    //
    #[error("function '{function}' at pc {pc}: call to undefined function '{callee}'")]
    UndefinedFunction {
        function: String,
        pc: usize,
        callee: String,
    },

    #[error(
        "function '{function}' at pc {pc}: native '{callee}' expects {expected} args but {found} were provided"
    )]
    NativeArgCountMismatch {
        function: String,
        pc: usize,
        callee: String,
        expected: usize,
        found: usize,
    },

    #[error(
        "function '{function}' at pc {pc}: native '{callee}' arg {arg_index} expects {expected}, found {found}"
    )]
    NativeArgTypeMismatch {
        function: String,
        pc: usize,
        callee: String,
        arg_index: usize,
        expected: String,
        found: String,
    },
}
