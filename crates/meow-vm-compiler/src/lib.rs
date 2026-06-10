//! Compiler for the Meow language: transforms source text into a bytecode module ready for on-chain publishing.

mod ast;
mod codegen;
mod parser;
mod type_checker;
mod validator;

pub mod error;

use std::collections::{HashMap, HashSet, VecDeque};

use chumsky::Parser;
use error::CompilerError;
use meow_vm_types::{
    address::Address,
    config::CompilerConfig,
    module::Module,
    natives::NativeSig,
    types::{StructDef, Type},
};

use crate::{
    ast::{AstFunction, AstItem},
    codegen::Codegen,
    parser::{parser, strip_line_comments},
};

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
/// mod my_module;
///
/// use dep_a@0xD1;
/// use math@0xD2 as m;
///
/// pub struct Token { id: dep_a::Id, balance: u64 }
/// struct Point { x: u64, y: u64 }
///
/// pub fn add(a: u64, b: u64) -> u64 {
///     return a + b;
/// }
/// ```
///
/// ## Supported features
/// - `mod NAME;` declaration — must be the first item in source
/// - Primitive types: `bool`, `u64`, `address`, `string`
/// - Address literals: `@0x02` (left-padded to 32 bytes)
/// - User-defined structs (`struct`, `pub struct`) with move semantics
/// - Functions (`fn`, `pub fn`) with parameters, optional return type, and tuple return types
/// - `let` bindings, reassignment (`x = expr;`), `return` statements
/// - `if` / `else` statements
/// - Struct destructuring: `let Name { field, .. } = value;`
/// - Tuple destructuring: `let (a, b) = expr;`
/// - Field access and assignment: `obj.field`, `obj.field = expr;`
/// - Binary operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`
/// - Unary operator: `!` (boolean not)
/// - Struct literals: `Foo { field: expr, … }`
/// - Function calls: `foo(arg, …)` (local or native)
/// - String literals: `"..."`
/// - Cross-module imports: `use module_name@0x...;` or `use module_name@0x... as alias;`
/// - Cross-module calls and types: `module_name::fn()`, `alias::TypeName`
pub struct Compiler;

impl Compiler {
    /// Parse `source` and return the declared dependencies.
    ///
    /// Each entry is `(name, alias, address)` where `name` is the actual module name
    /// (as declared in its `mod NAME;`) and `alias` is the local name used to reference
    /// it in source (`use name@address as alias;`, or same as `name` when no alias is given).
    /// The function validates:
    /// - source starts with exactly one `mod NAME;` declaration
    /// - no two `use` declarations share the same alias
    ///
    /// No dep modules need to be provided — this is intended for callers that need
    /// to know *which* modules to fetch before calling [`Compiler::compile`].
    ///
    /// Line comments (`// ...`) are stripped before parsing.
    pub fn extract_deps(source: &str) -> Result<Vec<(String, Option<String>, Address)>> {
        let (_, _, use_decls) = Self::parse_and_extract(source)?;
        Ok(use_decls)
    }

    /// Compile `source` with access to the given dependency modules.
    ///
    /// The source must start with a `mod NAME;` declaration. The name is
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
        native_sigs: &[NativeSig],
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
                "too many struct definitions: {} > limit of {}",
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

        for (dep_name, alias, dep_addr) in use_decls {
            let found = deps
                .iter()
                .find(|(addr, m)| *addr == dep_addr && m.name == dep_name);
            if found.is_none() {
                return Err(CompilerError::Message(format!(
                    "unknown dependency '{dep_name}@{dep_addr}': \
                     no provided dep module matches that name and address",
                )));
            }
            let local_name = alias.unwrap_or_else(|| dep_name.clone());
            validator::validate_identifier(&local_name, "use alias", &config)?;
            dep_addresses.insert(local_name, dep_addr);
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

        // First pass: collect struct definitions with basic per-struct validation.
        for item in &items {
            if let AstItem::Struct(ast_struct) = item {
                validator::validate_struct_def(ast_struct, &config)?;
                module.structs.push(StructDef {
                    name: ast_struct.name.clone(),
                    fields: validator::ast_fields_to_field_defs(&ast_struct.fields),
                    is_public: ast_struct.is_public,
                });
            }
        }

        // Build dep struct list.
        // Only `pub` structs from dep modules are included — private types are not visible
        // to other modules. Structs are stored under source-level qualified names
        // (`dep_name::StructName`) for compile-time look-up. The codegen later translates
        // these to address-qualified names (`@<hex>::StructName`) in the emitted bytecode.
        // Fields are retained for type resolution (struct literal construction, etc.),
        // but field access from other modules is always rejected (all fields are private).
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
                Some(
                    dep.structs
                        .iter()
                        .filter(|s| s.is_public) // only pub types cross-module visible
                        .map(move |s| StructDef {
                            name: format!("{}::{}", dep_name, s.name),
                            fields: s.fields.clone(),
                            is_public: true,
                        }),
                )
            })
            .flatten()
            .collect();

        // Cross-reference + cycle detection (after all local structs are collected).
        // Validation uses source-level names (e.g. "meow_object::Id") that match dep_structs.
        validator::validate_struct_refs(&module.structs, &dep_structs, &config)?;

        // Function parameter/return type references must resolve to a known struct too
        // (a private dep struct is excluded from `dep_structs`, so it is rejected here).
        let ast_fns: Vec<&AstFunction> = items
            .iter()
            .filter_map(|i| match i {
                AstItem::Fn(f) => Some(f),
                _ => None,
            })
            .collect();
        validator::validate_function_type_refs(&ast_fns, &module.structs, &dep_structs)?;

        // Translate cross-module field types to address-qualified form (e.g. "@0x1::Id") so
        // that the stored struct definitions match bytecode instructions and native signatures.
        validator::translate_struct_field_types(&mut module.structs, &dep_addresses);

        // Build combined struct list (local + qualified dep structs) for codegen.
        let mut all_structs = module.structs.clone();
        all_structs.extend(dep_structs);

        // Build dep module map (address → Module) for function visibility lookup in codegen.
        let dep_modules: HashMap<Address, &Module> = deps.iter().map(|(a, m)| (*a, *m)).collect();

        // Pre-collect local function return types for intra-module type inference.
        let local_fn_return_types: HashMap<String, Option<Type>> = items
            .iter()
            .filter_map(|item| match item {
                AstItem::Fn(f) => Some((f.name.clone(), f.return_type.clone())),
                _ => None,
            })
            .collect();

        let local_fn_param_types: HashMap<String, Vec<Type>> = items
            .iter()
            .filter_map(|item| match item {
                AstItem::Fn(f) => Some((
                    f.name.clone(),
                    f.params.iter().map(|(_, ty)| ty.clone()).collect(),
                )),
                _ => None,
            })
            .collect();

        // Type-check all functions before codegen.
        {
            let ast_structs: Vec<&crate::ast::AstStruct> = items
                .iter()
                .filter_map(|i| {
                    if let AstItem::Struct(s) = i {
                        Some(s)
                    } else {
                        None
                    }
                })
                .collect();
            let ast_fns: Vec<&crate::ast::AstFunction> = items
                .iter()
                .filter_map(|i| {
                    if let AstItem::Fn(f) = i {
                        Some(f)
                    } else {
                        None
                    }
                })
                .collect();
            // Build the full set of native signatures visible to the type checker.
            // meow_vm_abort is always injected by the VM (in Vm::new), so the compiler
            // always knows its signature. Caller-provided sigs follow and may override it.
            let mut all_native_sigs: Vec<NativeSig> =
                vec![meow_vm_types::natives::meow_vm_abort_sig()];
            all_native_sigs.extend_from_slice(native_sigs);

            type_checker::check(
                &ast_structs,
                &dep_addresses,
                &dep_modules,
                &all_native_sigs,
                &local_fn_param_types,
                &local_fn_return_types,
                &ast_fns,
            )?;
        }

        // Second pass: compile functions.
        for item in items {
            if let AstItem::Fn(ast_fn) = item {
                let func = Codegen::compile_function(
                    &all_structs,
                    &dep_addresses,
                    dep_modules.clone(),
                    &local_fn_return_types,
                    ast_fn,
                    &config,
                )?;
                module.functions.push(func);
            }
        }

        Ok(module)
    }

    /// Parse `source` and extract the module name and `use` declarations.
    ///
    /// Validates:
    /// - source is parseable
    /// - first item is `mod NAME;`
    /// - no duplicate `mod NAME;` declarations
    /// - no duplicate `use` names
    ///
    /// Returns `(items, module_name, use_decls)` where `use_decls` is a list of
    /// `(dep_name, alias, address)` triples in source order.
    #[allow(clippy::type_complexity)]
    fn parse_and_extract(
        source: &str,
    ) -> Result<(Vec<AstItem>, String, Vec<(String, Option<String>, Address)>)> {
        let source_no_comments = strip_line_comments(source);
        let items = parser()
            .parse(source_no_comments.as_str())
            .into_result()
            .map_err(|errs| {
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
                    "source must begin with 'mod NAME;' declaration".to_string(),
                ));
            }
        };

        if items[1..]
            .iter()
            .any(|i| matches!(i, AstItem::ModuleDecl(_)))
        {
            return Err(CompilerError::Message(
                "duplicate 'mod NAME;' declaration: only one is allowed per file".to_string(),
            ));
        }

        let mut use_decls: Vec<(String, Option<String>, Address)> = Vec::new();
        for item in &items {
            if let AstItem::Use {
                name: dep_name,
                address: dep_addr,
                alias,
            } = item
            {
                let local_name = alias.as_ref().unwrap_or(dep_name);
                if use_decls
                    .iter()
                    .any(|(name, a, _)| a.as_deref().unwrap_or(name.as_str()) == local_name)
                {
                    return Err(CompilerError::Message(format!(
                        "duplicate use declaration: '{local_name}' is already used as an alias",
                    )));
                }
                use_decls.push((dep_name.clone(), alias.clone(), *dep_addr));
            }
        }

        Ok((items, module_name, use_decls))
    }
}
