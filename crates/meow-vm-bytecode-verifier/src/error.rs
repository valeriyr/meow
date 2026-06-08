//! Bytecode verifier error type, covering structural and type-safety violations.

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VerificationError {
    //
    // ─── Identifiers ───
    //
    #[error("invalid identifier '{name}' in {context}")]
    InvalidIdentifier { name: String, context: String },

    #[error("duplicate function name '{name}'")]
    DuplicateFunctionName { name: String },

    #[error("duplicate struct name '{name}'")]
    DuplicateStructName { name: String },

    //
    // ─── Module limits ───
    //
    #[error("module has {count} struct definitions, exceeding the limit of {limit}")]
    TooManyStructs { count: usize, limit: usize },

    #[error("module has {count} function definitions, exceeding the limit of {limit}")]
    TooManyFunctions { count: usize, limit: usize },

    #[error("module has {count} imports, exceeding the limit of {limit}")]
    TooManyImports { count: usize, limit: usize },

    //
    // ─── Struct shape ───
    //
    #[error("struct '{struct_name}' has no fields — structs must have at least one field")]
    EmptyStruct { struct_name: String },

    #[error("struct '{struct_name}' has {count} fields, exceeding the limit of {limit}")]
    TooManyFields {
        struct_name: String,
        count: usize,
        limit: usize,
    },

    #[error("struct '{struct_name}' is part of a cyclic field type definition")]
    CyclicStructDefinition { struct_name: String },

    #[error(
        "field '{field_name}' in struct '{struct_name}' has a tuple type — only primitives and struct types are allowed as field types"
    )]
    TupleFieldType {
        struct_name: String,
        field_name: String,
    },

    //
    // ─── Function limits ───
    //
    #[error("function '{function}': tuple has {size} elements, exceeding the limit of {limit}")]
    TupleTooLarge {
        function: String,
        size: usize,
        limit: usize,
    },

    #[error("function '{function}': local_count {local_count} < param count {param_count}")]
    LocalCountTooSmall {
        function: String,
        local_count: u8,
        param_count: usize,
    },

    #[error("function '{function}' has {count} parameters, exceeding the limit of {limit}")]
    TooManyParams {
        function: String,
        count: usize,
        limit: usize,
    },

    #[error(
        "function '{function}' has {count} bytecode instructions, exceeding the limit of {limit}"
    )]
    FunctionTooLarge {
        function: String,
        count: usize,
        limit: usize,
    },

    #[error("function '{function}' has local_count {count} exceeding the limit of {limit}")]
    TooManyLocals {
        function: String,
        count: u8,
        limit: u8,
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

    #[error("function '{function}' at pc {pc}: field path must not be empty")]
    EmptyFieldPath { function: String, pc: usize },

    //
    // ─── Type declarations ───
    //
    #[error("unresolved type '{type_name}' in {context}")]
    UnresolvedTypeReference { context: String, type_name: String },

    //
    // ─── NewStruct / UnpackStruct instructions ───
    //
    #[error("function '{function}' at pc {pc}: NewStruct references unknown type '{type_name}'")]
    UndefinedStructType {
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
        "function '{function}': struct liveness at join point pc {join_pc} differs between branch paths (slot {slot})"
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
    // ─── Struct linearity ───
    //
    #[error(
        "function '{function}' at pc {pc}: field '{field}' has struct type — LoadField on a struct-typed field is forbidden; use UnpackStruct to extract struct fields"
    )]
    StructTypedFieldLoaded {
        function: String,
        pc: usize,
        field: String,
    },

    #[error(
        "function '{function}' at pc {pc}: field '{field}' has struct type — StoreField into a struct-typed field is forbidden; the old value would be implicitly dropped"
    )]
    StructTypedFieldWritten {
        function: String,
        pc: usize,
        field: String,
    },

    #[error(
        "function '{function}' at pc {pc}: GetField on struct '{type_name}' would silently drop linear field '{linear_field}' — use UnpackStruct to consume all fields explicitly"
    )]
    GetFieldDropsLinearField {
        function: String,
        pc: usize,
        type_name: String,
        linear_field: String,
    },

    #[error("function '{function}' at pc {pc}: use-after-move of struct in slot {slot}")]
    UseAfterMove {
        function: String,
        pc: usize,
        slot: u8,
    },

    #[error(
        "function '{function}' at pc {pc}: Pop on struct value — structs have move semantics and must be explicitly consumed"
    )]
    PopOnStruct { function: String, pc: usize },

    #[error("function '{function}' at pc {pc}: Dup on struct value — structs have move semantics")]
    DupOnStruct { function: String, pc: usize },

    #[error(
        "function '{function}' at pc {pc}: Store overwrites live struct in slot {slot} — consume the existing value first"
    )]
    SlotOverwrite {
        function: String,
        pc: usize,
        slot: u8,
    },

    #[error("function '{function}': struct in slot {slot} was not consumed before Return")]
    UnconsumedStruct { function: String, slot: u8 },

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
