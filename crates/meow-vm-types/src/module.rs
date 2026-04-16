use serde::{Deserialize, Serialize};

use crate::{
    address::Address,
    bytecode::Instruction,
    types::{StructDef, Type},
};

/// A compiled function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// The function name.
    pub name: String,
    /// Parameters in call order (name, type).
    pub params: Vec<(String, Type)>,
    /// Return type, or `None` for void functions.
    pub return_type: Option<Type>,
    /// Total number of local variable slots (parameters + `let` bindings).
    pub local_count: u8,
    /// Compiled bytecode.
    pub code: Vec<Instruction>,
}

/// A compiled module — the unit of compilation and execution.
///
/// A module contains struct/object definitions (schemas) and compiled functions.
/// It is produced by the compiler and consumed by the VM.
///
/// The module itself does not carry an on-chain address — the address is always
/// supplied externally (e.g. as the address of the object that stores the module
/// on chain, or as an explicit `(Address, Module)` pair). This keeps the module
/// content pure and reproducible regardless of where it is deployed.
///
/// Cross-module bytecode references embed the dep module's address (not its name)
/// so that resolution is unambiguous even when names collide.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Human-readable module name declared by the `module NAME;` statement.
    pub name: String,
    /// Addresses of dependency modules declared via `use module_name@address;`.
    pub imports: Vec<Address>,
    /// Struct and object definitions.
    pub structs: Vec<StructDef>,
    /// Compiled functions.
    pub functions: Vec<Function>,
}

impl Module {
    /// Creates a new empty module with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            imports: Vec::new(),
            structs: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Find a struct/object definition by name.
    pub fn get_struct(&self, name: &str) -> Option<&StructDef> {
        self.structs.iter().find(|s| s.name == name)
    }

    /// Find a compiled function by name.
    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }
}
