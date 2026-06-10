//! Semantic validator: checks well-formedness constraints that require cross-referencing
//! the full module (struct existence, field type validity, identifier rules, count limits).

use std::collections::{HashMap, HashSet};

use meow_vm_types::{
    address::Address,
    config::CompilerConfig,
    identifier::{self, RESERVED_FUNCTION_NAMES},
    module::Module,
    module_ref,
    types::{FieldDef, StructDef, Type},
};

use crate::{
    Result,
    ast::{AstFunction, AstStruct},
    error::CompilerError,
};

pub fn validate_identifier(name: &str, context: &str, config: &CompilerConfig) -> Result<()> {
    if !identifier::is_valid_identifier(name, config) {
        Err(CompilerError::Message(format!(
            "{context}: '{}' is not a valid identifier \
             (must start with a letter or underscore, followed by letters, digits, or underscores; \
             max {} characters)",
            name,
            config.max_identifier_len(),
        )))
    } else {
        Ok(())
    }
}

pub fn validate_function_name(name: &str, config: &CompilerConfig) -> Result<()> {
    validate_identifier(name, "function name", config)?;

    let is_vm_reserved = RESERVED_FUNCTION_NAMES.contains(&name);
    let is_config_reserved = config.reserved_function_names().iter().any(|n| n == name);

    if is_vm_reserved || is_config_reserved {
        return Err(CompilerError::Message(format!(
            "function name '{}' is reserved for a built-in native function",
            name
        )));
    }
    Ok(())
}

/// Basic per-struct validation: identifier names and field count.
/// Field *type* cross-references are checked separately in [`validate_struct_refs`].
pub fn validate_struct_def(def: &AstStruct, config: &CompilerConfig) -> Result<()> {
    validate_identifier(&def.name, "struct name", config)?;

    if def.fields.is_empty() {
        return Err(CompilerError::Message(format!(
            "struct '{}': must have at least one field",
            def.name,
        )));
    }

    let max_fields = config.max_fields();
    if def.fields.len() > max_fields {
        return Err(CompilerError::Message(format!(
            "struct '{}': too many fields ({} > limit of {})",
            def.name,
            def.fields.len(),
            max_fields,
        )));
    }

    for (field_name, ty) in &def.fields {
        validate_identifier(
            field_name,
            &format!("field in struct '{}'", def.name),
            config,
        )?;
        if !ty.is_valid_field_type() {
            return Err(CompilerError::Message(format!(
                "struct '{}': field '{field_name}' has type '{}' which is not allowed as a field type — \
                 only primitives (bool, u64, address, string) and structs are allowed",
                def.name,
                ty.name()
            )));
        }
    }

    Ok(())
}

/// Build a [`FieldDef`] list from an AST struct's field tuples.
pub fn ast_fields_to_field_defs(fields: &[(String, Type)]) -> Vec<FieldDef> {
    fields
        .iter()
        .map(|(name, ty)| FieldDef {
            name: name.clone(),
            ty: ty.clone(),
        })
        .collect()
}

/// Translate cross-module type names in struct field definitions from source-level
/// `dep_name::TypeName` form to address-qualified `@<hex>::TypeName` form.
///
/// This must be called after validation (which uses source-level names for lookups)
/// so that the stored struct definitions match the address-qualified names used in
/// bytecode instructions and native signatures.
pub fn translate_struct_field_types(
    structs: &mut [StructDef],
    dep_addresses: &HashMap<String, Address>,
) {
    for def in structs.iter_mut() {
        for field in def.fields.iter_mut() {
            field.ty = translate_type(&field.ty, dep_addresses);
        }
    }
}

/// Translate a `Struct(name)` type in `dep_name::TypeName` form to
/// `Struct("@<hex_address>::TypeName")`. Other types pass through unchanged.
fn translate_type(ty: &Type, dep_addresses: &HashMap<String, Address>) -> Type {
    match ty {
        Type::Struct(name) => {
            if let Some((mod_name, type_name)) = name.split_once("::")
                && let Some(addr) = dep_addresses.get(mod_name)
            {
                return Type::Struct(module_ref::qualify(addr, type_name));
            }
            ty.clone()
        }
        Type::Tuple(types) => Type::Tuple(
            types
                .iter()
                .map(|t| translate_type(t, dep_addresses))
                .collect(),
        ),
        _ => ty.clone(),
    }
}

/// Detect circular import dependencies among the provided dep modules.
///
/// Checks the import graph formed by the `imports` lists of all provided dep modules.
/// If any cycle is found among those deps (e.g. A imports B and B imports A), compilation
/// is rejected — circular module dependencies are not allowed.
///
/// Note: this check covers cycles within the *provided deps only*. A full cycle check
/// that includes the module being compiled (which has no address yet) must be performed
/// at publish time by the node.
pub fn detect_module_dep_cycles(deps: &[(Address, &Module)]) -> Result<()> {
    // Build a set of known dep addresses for fast lookup.
    let dep_map: HashMap<Address, &Module> = deps.iter().map(|(addr, m)| (*addr, *m)).collect();

    let mut visited: HashSet<Address> = HashSet::new();

    for (start_addr, _) in deps {
        if !visited.contains(start_addr) {
            let mut in_stack: HashSet<Address> = HashSet::new();
            if let Some(cycle_addr) =
                dfs_module_cycle(start_addr, &dep_map, &mut visited, &mut in_stack)
            {
                let cycle_name = dep_map
                    .get(&cycle_addr)
                    .map(|m| m.name.as_str())
                    .unwrap_or("<unknown>");
                return Err(CompilerError::Message(format!(
                    "circular module dependency detected: module '{cycle_name}' at {cycle_addr} \
                     is part of a recursive import chain",
                )));
            }
        }
    }
    Ok(())
}

/// DFS helper: returns the address of a module involved in a cycle, or `None`.
fn dfs_module_cycle(
    addr: &Address,
    dep_map: &HashMap<Address, &Module>,
    visited: &mut HashSet<Address>,
    in_stack: &mut HashSet<Address>,
) -> Option<Address> {
    visited.insert(*addr);
    in_stack.insert(*addr);

    if let Some(module) = dep_map.get(addr) {
        for import_addr in &module.imports {
            if !dep_map.contains_key(import_addr) {
                // Import references a module not in the provided set — skip.
                continue;
            }
            if !visited.contains(import_addr) {
                if let Some(c) = dfs_module_cycle(import_addr, dep_map, visited, in_stack) {
                    return Some(c);
                }
            } else if in_stack.contains(import_addr) {
                return Some(*import_addr);
            }
        }
    }

    in_stack.remove(addr);
    None
}

/// Cross-reference and cycle validation for all struct definitions in a module.
///
/// `local_structs` — structs defined in the module being compiled.
/// `dep_structs` — structs from dependency modules (already qualified with `module::` prefix).
///
/// This must be called after all struct definitions have been collected so that
/// forward references (struct A has a field of type B, defined later) work correctly.
pub fn validate_struct_refs(
    local_structs: &[StructDef],
    dep_structs: &[StructDef],
    config: &CompilerConfig,
) -> Result<()> {
    // Build the full set of known struct names (local + dep).
    let all_structs: Vec<&StructDef> = local_structs.iter().chain(dep_structs.iter()).collect();
    let known_struct_names: HashSet<&str> = all_structs.iter().map(|s| s.name.as_str()).collect();

    for def in local_structs {
        for field in &def.fields {
            if let Type::Struct(sname) = &field.ty
                && !known_struct_names.contains(sname.as_str())
            {
                return Err(CompilerError::Message(format!(
                    "struct '{}': field '{}' references unknown struct '{sname}'",
                    def.name, field.name,
                )));
            }
        }
    }

    // Cycle detection via DFS (only among local structs; dep structs are pre-validated).
    detect_struct_cycles(local_structs, dep_structs, config)?;

    Ok(())
}

/// Reject function parameter and return types that reference an unknown struct.
///
/// `dep_structs` contains only `pub` structs from dependencies, so a private dep
/// struct (or a nonexistent type) used in a signature resolves to nothing and is
/// rejected. This mirrors the field-type check in [`validate_struct_refs`], keeping
/// the struct-visibility rule consistent across fields, parameters, and return types.
/// Type names are the source-level form (e.g. `lib::Hidden`) at this stage.
pub fn validate_function_type_refs(
    functions: &[&AstFunction],
    local_structs: &[StructDef],
    dep_structs: &[StructDef],
) -> Result<()> {
    let known: HashSet<&str> = local_structs
        .iter()
        .chain(dep_structs.iter())
        .map(|s| s.name.as_str())
        .collect();

    for f in functions {
        for (param_name, ty) in &f.params {
            check_type_ref_known(
                ty,
                &known,
                &format!("function '{}' parameter '{param_name}'", f.name),
            )?;
        }
        if let Some(ty) = &f.return_type {
            check_type_ref_known(ty, &known, &format!("function '{}' return type", f.name))?;
        }
    }
    Ok(())
}

/// Recursively verify that every struct type named in `ty` is a known struct.
fn check_type_ref_known(ty: &Type, known: &HashSet<&str>, context: &str) -> Result<()> {
    match ty {
        Type::Struct(name) if !known.contains(name.as_str()) => Err(CompilerError::Message(
            format!("{context} references unknown struct '{name}'"),
        )),
        Type::Tuple(types) => {
            for t in types {
                check_type_ref_known(t, known, context)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Detects reference cycles among struct definitions using DFS.
fn detect_struct_cycles(
    local_structs: &[StructDef],
    dep_structs: &[StructDef],
    _config: &CompilerConfig,
) -> Result<()> {
    let all: Vec<&StructDef> = local_structs.iter().chain(dep_structs.iter()).collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut in_stack: HashSet<String> = HashSet::new();

    for s in local_structs {
        if !visited.contains(&s.name)
            && let Some(cycle_name) = dfs_cycle(&s.name, &all, &mut visited, &mut in_stack)
        {
            return Err(CompilerError::Message(format!(
                "struct cycle detected: '{}' is part of a recursive struct definition",
                cycle_name,
            )));
        }
    }
    Ok(())
}

/// Returns the name of a struct involved in a cycle, or `None` if no cycle.
fn dfs_cycle<'a>(
    name: &'a str,
    all: &[&'a StructDef],
    visited: &mut HashSet<String>,
    in_stack: &mut HashSet<String>,
) -> Option<String> {
    visited.insert(name.to_string());
    in_stack.insert(name.to_string());

    if let Some(def) = all.iter().find(|s| s.name == name) {
        for field in &def.fields {
            if let Type::Struct(dep_name) = &field.ty {
                if !visited.contains(dep_name.as_str()) {
                    if let Some(c) = dfs_cycle(dep_name, all, visited, in_stack) {
                        return Some(c);
                    }
                } else if in_stack.contains(dep_name.as_str()) {
                    return Some(dep_name.clone());
                }
            }
        }
    }

    in_stack.remove(name);
    None
}
