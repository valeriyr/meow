//! Compiler for the Meow language: source text → [`Module`].
//!
//! The single entry point is [`Compiler::compile`], which parses source text
//! and produces a [`meow_vm_types::module::Module`] ready for execution by the VM.

mod ast;
mod codegen;
mod parser;
mod validator;

pub mod error;

use std::collections::{HashMap, HashSet, VecDeque};

use chumsky::Parser;
use error::CompilerError;
use meow_vm_types::{address::Address, config::CompilerConfig, module::Module, types::StructDef};

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
/// module my_module;
///
/// object Coin { id: address, value: u64 }
/// struct Point { x: u64, y: u64 }
///
/// fn add(a: u64, b: u64): u64 {
///     return a + b;
/// }
/// ```
///
/// ## Supported features
/// - `module NAME;` declaration — must be the first item in source
/// - Primitive types: `bool`, `u64`, `address`
/// - Address literals: `@0x02` (left-padded to 32 bytes)
/// - User-defined structs with primitive-typed fields (and nested struct fields)
/// - User-defined objects with `id: address` as first field
/// - Functions with parameters and an optional return type (not Object)
/// - `let` bindings, `return` statements
/// - `if` / `else` statements
/// - Field assignment: `obj.field = expr;`
/// - Binary operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`
/// - Field access: `expr.field`
/// - Struct/object literals: `Foo { field: expr, … }`
/// - Function calls: `foo(arg, …)` (module or native)
/// - String literals: `"..."`
/// - Cross-module dependencies via `use module_name@0x...;` and qualified names `module_name::TypeOrFn`
pub struct Compiler;

impl Compiler {
    /// Parse `source` and return the list of declared dependencies.
    ///
    /// Each entry is `(module_name, address)` as written in a `use module_name@address;`
    /// declaration. The function validates:
    /// - source starts with exactly one `module NAME;` declaration
    /// - no two `use` declarations share the same module name
    ///
    /// No dep modules need to be provided — this is intended for callers that need
    /// to know *which* modules to fetch before calling [`Compiler::compile`].
    pub fn extract_deps(source: &str) -> Result<Vec<(String, Address)>> {
        let (_, _, use_decls) = Self::parse_and_extract(source)?;
        Ok(use_decls)
    }

    /// Compile `source` with access to the given dependency modules.
    ///
    /// The source must start with a `module NAME;` declaration. The name is
    /// extracted from the source and becomes the module's human-readable identifier.
    ///
    /// `deps` is a list of `(address, module)` pairs for all modules declared via
    /// `use module_name@address;`. Each dep is validated to ensure the declared
    /// name and address match the provided module.
    ///
    /// Cross-module calls and struct constructions are encoded in bytecode using the
    /// dep module's **address** (from the `use` declaration), so two modules with the
    /// same name but different addresses are kept distinct in the bytecode and at runtime.
    pub fn compile(
        source: &str,
        deps: &[(Address, &Module)],
        config: CompilerConfig,
    ) -> Result<Module> {
        let (items, module_name, use_decls) = Self::parse_and_extract(source)?;

        validator::validate_identifier(&module_name, "module name", &config)?;

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

        let import_count = use_decls.len();
        let max_imports = config.max_imports();
        if import_count > max_imports {
            return Err(CompilerError::Message(format!(
                "too many use declarations: {} > limit of {}",
                import_count, max_imports,
            )));
        }

        let dep_count = deps.len();
        let max_dep_modules = config.max_dep_modules();
        if dep_count > max_dep_modules {
            return Err(CompilerError::Message(format!(
                "too many dependency modules: {} > limit of {}",
                dep_count, max_dep_modules,
            )));
        }

        // Validate no duplicate addresses in deps.
        let mut seen_dep_addrs = std::collections::HashSet::new();
        for (addr, _) in deps {
            if !seen_dep_addrs.insert(addr) {
                return Err(CompilerError::Message(format!(
                    "duplicate dep address: {addr}",
                )));
            }
        }

        // Validate each declared `use` against the provided dep modules.
        // Build a name → address map for codegen to translate `name::something`.
        let mut dep_addresses: HashMap<String, Address> = HashMap::new();
        let mut import_addresses: Vec<Address> = Vec::new();

        for (dep_name, dep_addr) in use_decls {
            let found = deps
                .iter()
                .find(|(addr, m)| *addr == dep_addr && m.name == dep_name);
            if found.is_none() {
                return Err(CompilerError::Message(format!(
                    "unknown dependency '{dep_name}@{dep_addr}': \
                     no provided dep module matches that name and address",
                )));
            }
            dep_addresses.insert(dep_name, dep_addr);
            import_addresses.push(dep_addr);
        }

        // Validate the full transitive closure by BFS from declared imports.
        // Only modules reachable from declared `use` addresses are checked —
        // extra deps not in the reachable import tree are intentionally ignored.
        {
            let dep_map: HashMap<Address, &Module> = deps.iter().map(|(a, m)| (*a, *m)).collect();
            let mut visited: HashSet<Address> = HashSet::new();
            let mut queue: VecDeque<Address> = import_addresses.iter().cloned().collect();

            while let Some(addr) = queue.pop_front() {
                if !visited.insert(addr) {
                    continue; // already verified
                }
                let dep_module = dep_map.get(&addr).ok_or_else(|| {
                    CompilerError::Message(format!(
                        "transitive dependency at {addr} is required but was not provided",
                    ))
                })?;
                for import_vm_addr in &dep_module.imports {
                    if !visited.contains(import_vm_addr) {
                        queue.push_back(*import_vm_addr);
                    }
                }
            }
        }

        // Detect cycles among provided dep modules.
        validator::detect_module_dep_cycles(deps)?;

        let mut module = Module::new(&module_name);
        module.imports = import_addresses;

        // First pass: collect struct/object definitions with basic per-struct validation.
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

        // Build dep struct list.
        // Structs are stored under source-level qualified names (`dep_name::StructName`)
        // for compile-time look-up. The codegen later translates these to address-qualified
        // names (`@<hex>::StructName`) in the emitted bytecode.
        let dep_structs: Vec<StructDef> = deps
            .iter()
            .filter_map(|(addr, dep)| {
                // Only include deps that were actually declared via `use`.
                dep_addresses.values().find(|a| *a == addr)?;
                // Find the dep name from dep_addresses (by matching address).
                let dep_name = dep_addresses
                    .iter()
                    .find(|(_, a)| *a == addr)
                    .map(|(n, _)| n.clone())?;
                Some(dep.structs.iter().map(move |s| StructDef {
                    name: format!("{}::{}", dep_name, s.name),
                    fields: s.fields.clone(),
                    is_object: s.is_object,
                }))
            })
            .flatten()
            .collect();

        // Cross-reference + cycle detection (after all local structs are collected).
        validator::validate_struct_refs(&module.structs, &dep_structs, &config)?;

        // Build combined struct list (local + qualified dep structs) for codegen.
        let mut all_structs = module.structs.clone();
        all_structs.extend(dep_structs);

        // Second pass: compile functions.
        for item in items {
            if let AstItem::Fn(ast_fn) = item {
                let func =
                    Codegen::compile_function(&all_structs, &dep_addresses, ast_fn, &config)?;
                module.functions.push(func);
            }
        }

        Ok(module)
    }

    /// Parse `source` and extract the module name and `use` declarations.
    ///
    /// Validates:
    /// - source is parseable
    /// - first item is `module NAME;`
    /// - no duplicate `module NAME;` declarations
    /// - no duplicate `use` names
    ///
    /// Returns `(items, module_name, use_decls)` where `use_decls` is a list of
    /// `(name, address)` pairs in source order.
    #[allow(clippy::type_complexity)]
    fn parse_and_extract(source: &str) -> Result<(Vec<AstItem>, String, Vec<(String, Address)>)> {
        let items = parser().parse(source).into_result().map_err(|errs| {
            let msg = errs
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            CompilerError::Message(msg)
        })?;

        let module_name = match items.first() {
            Some(AstItem::ModuleDecl(name)) => name.clone(),
            _ => {
                return Err(CompilerError::Message(
                    "source must begin with 'module NAME;' declaration".to_string(),
                ));
            }
        };

        if items[1..]
            .iter()
            .any(|i| matches!(i, AstItem::ModuleDecl(_)))
        {
            return Err(CompilerError::Message(
                "duplicate 'module NAME;' declaration: only one is allowed per file".to_string(),
            ));
        }

        let mut use_decls: Vec<(String, Address)> = Vec::new();
        for item in &items {
            if let AstItem::Use {
                name: dep_name,
                address: dep_addr,
            } = item
            {
                if use_decls.iter().any(|(n, _)| n == dep_name) {
                    return Err(CompilerError::Message(format!(
                        "duplicate use declaration: '{dep_name}' is already imported",
                    )));
                }
                use_decls.push((dep_name.clone(), *dep_addr));
            }
        }

        Ok((items, module_name, use_decls))
    }
}
