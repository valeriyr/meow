//
// ─── Compiler configuration ───
//

/// The compiler configuration.
#[derive(Debug, Clone)]
pub struct CompilerConfig {
    /// Maximum character length of any identifier (module, function, struct, field, or variable name).
    max_identifier_len: usize,
    /// Maximum number of struct/object definitions in a module.
    max_structs: usize,
    /// Maximum number of functions in a module.
    max_functions: usize,
    /// Maximum number of fields in a struct or object definition.
    max_fields: usize,
    /// Maximum number of parameters in a function.
    max_params: usize,
    /// Maximum number of local variable slots in a function.
    max_locals: usize,
    /// Maximum number of bytecode instructions in a single function.
    max_fun_code_size: usize,
    /// Additional function names reserved by the caller (e.g. native functions
    /// registered by the adapter). The compiler rejects any user-defined function
    /// whose name appears in this list.
    reserved_function_names: Vec<String>,
}

impl CompilerConfig {
    /// Returns the maximum character length of any identifier (module, function, struct, field, or variable name).
    pub fn max_identifier_len(&self) -> usize {
        self.max_identifier_len
    }

    /// Returns the maximum number of struct/object definitions allowed in a module.
    pub fn max_structs(&self) -> usize {
        self.max_structs
    }

    /// Returns the maximum number of functions allowed in a module.
    pub fn max_functions(&self) -> usize {
        self.max_functions
    }

    /// Returns the maximum number of fields allowed in a struct or object definition.
    pub fn max_fields(&self) -> usize {
        self.max_fields
    }

    /// Returns the maximum number of parameters allowed in a function.
    pub fn max_params(&self) -> usize {
        self.max_params
    }

    /// Returns the maximum number of local variable slots allowed in a function.
    pub fn max_locals(&self) -> usize {
        self.max_locals
    }

    /// Returns the maximum number of bytecode instructions allowed in a single function.
    pub fn max_fun_code_size(&self) -> usize {
        self.max_fun_code_size
    }

    /// Returns the caller-supplied reserved function names.
    pub fn reserved_function_names(&self) -> &[String] {
        &self.reserved_function_names
    }

    /// Returns a new config with additional reserved function names appended.
    pub fn with_reserved_function_names<T: AsRef<str>>(mut self, names: &[T]) -> Self {
        self.reserved_function_names = names.iter().map(|name| name.as_ref().to_string()).collect();
        self
    }
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            max_identifier_len: 128,
            max_structs: 128,
            max_functions: 256,
            max_fields: 32,
            max_params: 16,
            max_locals: 255,
            max_fun_code_size: 65_536,
            reserved_function_names: Vec::new(),
        }
    }
}

//
// ─── VM configuration ───
//

/// The VM configuration.
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Maximum VM call stack depth.
    max_call_depth: usize,
}

impl VmConfig {
    /// Returns the maximum VM call stack depth.
    pub fn max_call_depth(&self) -> usize {
        self.max_call_depth
    }
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_call_depth: 256,
        }
    }
}
