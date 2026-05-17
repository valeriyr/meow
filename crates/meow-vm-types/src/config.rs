//
// ─── Compiler configuration ───
//

/// The compiler configuration.
#[derive(Debug, Clone)]
pub struct CompilerConfig {
    /// Maximum character length of any identifier (module, function, struct, field, or variable name).
    max_identifier_len: usize,
    /// Maximum number of struct definitions in a module.
    max_structs: usize,
    /// Maximum number of functions in a module.
    max_functions: usize,
    /// Maximum number of fields in a struct definition.
    max_fields: usize,
    /// Maximum number of parameters in a function.
    /// Bytecode-constrained: parameters occupy the first N slots, and slot indices are `u8`.
    max_params: u8,
    /// Maximum number of elements in a tuple literal or tuple return type.
    /// Bytecode-constrained: `MakeTuple` and `UnpackTuple` encode the count as `u8`.
    max_tuple_elements: u8,
    /// Maximum number of local variable slots in a function.
    /// Bytecode-constrained: slot indices are encoded as `u8` in `Load` / `Store`.
    max_locals: u8,
    /// Maximum number of bytecode instructions in a single function.
    max_fun_code_size: usize,
    /// Maximum number of `use` (import) declarations in a module.
    max_imports: usize,
    /// Maximum total number of dependency modules (direct + transitive) that
    /// may be provided when compiling a module. Prevents modules from being
    /// published with a dependency graph that would exceed the runtime limit.
    max_dep_modules: usize,
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

    /// Returns the maximum number of struct definitions allowed in a module.
    pub fn max_structs(&self) -> usize {
        self.max_structs
    }

    /// Returns the maximum number of functions allowed in a module.
    pub fn max_functions(&self) -> usize {
        self.max_functions
    }

    /// Returns the maximum number of fields allowed in a struct definition.
    pub fn max_fields(&self) -> usize {
        self.max_fields
    }

    /// Returns the maximum number of parameters allowed in a function.
    /// Returns `u8` because slot indices are `u8` in the bytecode.
    pub fn max_params(&self) -> u8 {
        self.max_params
    }

    /// Returns the maximum number of elements allowed in a tuple literal or tuple return type.
    /// Returns `u8` because `MakeTuple` / `UnpackTuple` encode the count as `u8` in the bytecode.
    pub fn max_tuple_elements(&self) -> u8 {
        self.max_tuple_elements
    }

    /// Returns the maximum number of local variable slots allowed in a function.
    /// Returns `u8` because `Load` / `Store` encode slot indices as `u8` in the bytecode.
    pub fn max_locals(&self) -> u8 {
        self.max_locals
    }

    /// Returns the maximum number of bytecode instructions allowed in a single function.
    pub fn max_fun_code_size(&self) -> usize {
        self.max_fun_code_size
    }

    /// Returns the maximum number of `use` (import) declarations allowed in a module.
    pub fn max_imports(&self) -> usize {
        self.max_imports
    }

    /// Returns the maximum number of dependency modules (direct + transitive)
    /// allowed when compiling a module.
    pub fn max_dep_modules(&self) -> usize {
        self.max_dep_modules
    }

    /// Returns the caller-supplied reserved function names.
    pub fn reserved_function_names(&self) -> &[String] {
        &self.reserved_function_names
    }

    /// Returns a new config with the maximum number of dependency modules set.
    pub fn with_max_dep_modules(mut self, max: usize) -> Self {
        self.max_dep_modules = max;
        self
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
            max_tuple_elements: 16,
            max_locals: 255,
            max_fun_code_size: 65_536,
            max_imports: 64,
            max_dep_modules: 64,
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
    /// Maximum total number of dependency modules (direct + transitive) that
    /// may be loaded for a single call. Prevents unbounded memory use from
    /// deeply nested or wide dependency graphs.
    max_dep_modules: usize,
    /// Whether to allow calls to private functions.
    /// This is used to enforce that only `pub fn` can be called from outside a module
    /// (e.g. directly from a transaction).
    enable_call_private_functions: bool,
}

impl VmConfig {
    /// Returns the maximum VM call stack depth.
    pub fn max_call_depth(&self) -> usize {
        self.max_call_depth
    }

    /// Returns the maximum number of dependency modules (direct + transitive)
    /// allowed during a single call.
    pub fn max_dep_modules(&self) -> usize {
        self.max_dep_modules
    }

    /// Returns whether calls to private functions are allowed.
    pub fn enable_call_private_functions(&self) -> bool {
        self.enable_call_private_functions
    }

    /// Returns a new config with the maximum call stack depth set.
    pub fn with_max_call_depth(mut self, max: usize) -> Self {
        self.max_call_depth = max;
        self
    }

    /// Returns a new config with the maximum number of dependency modules set.
    pub fn with_max_dep_modules(mut self, max: usize) -> Self {
        self.max_dep_modules = max;
        self
    }

    /// Returns a new config with the `enable_call_private_functions` flag set.
    pub fn with_enable_call_private_functions(mut self, enable: bool) -> Self {
        self.enable_call_private_functions = enable;
        self
    }
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            max_call_depth: 256,
            max_dep_modules: 64,
            enable_call_private_functions: false,
        }
    }
}
