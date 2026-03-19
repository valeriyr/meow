//! Compiler for the Meow language: source text → [`Module`].
//!
//! The single entry point is [`Compiler::compile`], which parses source text
//! and produces a [`meow_vm_types::module::Module`] ready for execution by the VM.

mod ast;
mod codegen;
mod parser;
mod validator;

pub mod error;

use chumsky::Parser;
use meow_vm_types::{config::CompilerConfig, module::Module, types::StructDef};

use error::CompilerError;

use crate::{ast::AstItem, codegen::Codegen, parser::parser};

/// An error that can occur during compilation.
pub type Result<T> = std::result::Result<T, CompilerError>;

//
// ─── Public API ───
//

/// Compiler: source text → [`Module`].
///
/// # Language overview
///
/// ```text
/// object Coin { id: address, value: u64 }
/// struct Point { x: u64, y: u64 }
///
/// fn add(a: u64, b: u64): u64 {
///     return a + b;
/// }
///
/// fn make_point(x: u64, y: u64): Point {
///     return Point { x: x, y: y };
/// }
/// ```
///
/// ## Supported features
/// - Primitive types: `bool`, `u64`, `address`
/// - User-defined structs with primitive-typed fields
/// - User-defined objects with `id: address` as first field
/// - Functions with parameters and an optional return type (not Object)
/// - `let` bindings, `return` statements
/// - `if` statements
/// - Field assignment: `obj.field = expr;`
/// - Binary operators: `+`, `-`, `*`, `/`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`
/// - Field access: `expr.field` (LoadField for locals, GetField for stack values)
/// - Struct/object literals: `Foo { field: expr, … }`
/// - Function calls: `foo(arg, …)` (module or native)
/// - String literals: `"..."` (for native function arguments)
pub struct Compiler;

impl Compiler {
    /// Compile `source` into a module named `module_name` using the given `config`.
    pub fn compile(module_name: &str, source: &str, config: CompilerConfig) -> Result<Module> {
        validator::validate_identifier(module_name, "module name")?;

        let items = parser().parse(source).into_result().map_err(|errs| {
            let msg = errs
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            CompilerError::Message(msg)
        })?;

        let struct_count = items
            .iter()
            .filter(|i| matches!(i, AstItem::Struct(_)))
            .count();
        let max_structs = config.max_structs();
        if struct_count > max_structs {
            return Err(CompilerError::Message(format!(
                "too many struct/object definitions: {} > limit of {}",
                struct_count, max_structs,
            )));
        }

        let fn_count = items.iter().filter(|i| matches!(i, AstItem::Fn(_))).count();
        let max_functions = config.max_functions();
        if fn_count > max_functions {
            return Err(CompilerError::Message(format!(
                "too many functions: {} > limit of {}",
                fn_count, max_functions,
            )));
        }

        let mut module = Module::new(module_name);

        // First pass: collect struct/object definitions.
        for item in &items {
            if let AstItem::Struct(ast_struct) = item {
                validator::validate_struct_def(ast_struct, &config)?;
                module.structs.push(StructDef {
                    name: ast_struct.name.clone(),
                    fields: ast_struct.fields.clone(),
                    is_object: ast_struct.is_object,
                });
            }
        }

        // Second pass: compile functions.
        let structs_snapshot = module.structs.clone();
        for item in items {
            if let AstItem::Fn(ast_fn) = item {
                let func = Codegen::compile_function(&structs_snapshot, ast_fn, &config)?;
                module.functions.push(func);
            }
        }

        Ok(module)
    }
}
