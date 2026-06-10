//! Phase 1 of bytecode verification: purely syntactic checks that need no stack state.
//!
//! Runs before abstract interpretation because it is cheaper and its errors are independent
//! of type information. Both phases always run and their errors are accumulated together;
//! the abstract-interpretation phase is written to tolerate structurally-invalid input
//! (it skips individual instructions the structural phase has already flagged).

use std::collections::{HashMap, HashSet};

use meow_vm_types::module_ref;
use meow_vm_types::{
    address::Address,
    bytecode::Instruction,
    config::CompilerConfig,
    identifier::{RESERVED_FUNCTION_NAMES, is_valid_identifier},
    module::Module,
    types::Type,
};

use crate::error::VerificationError;

/// Phase 1 — purely structural checks that require no stack simulation.
pub(crate) fn check_module(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    cfg: &CompilerConfig,
) -> Vec<VerificationError> {
    let mut errors = Vec::new();

    // Module name
    if !is_valid_identifier(&module.name, cfg) {
        errors.push(VerificationError::InvalidIdentifier {
            name: module.name.clone(),
            context: "module declaration".to_string(),
        });
    }

    // Module-level count limits
    let struct_count = module.structs.len();
    let struct_limit = cfg.max_structs();
    if struct_count > struct_limit {
        errors.push(VerificationError::TooManyStructs {
            count: struct_count,
            limit: struct_limit,
        });
    }

    let fn_count = module.functions.len();
    let fn_limit = cfg.max_functions();
    if fn_count > fn_limit {
        errors.push(VerificationError::TooManyFunctions {
            count: fn_count,
            limit: fn_limit,
        });
    }

    let import_count = module.imports.len();
    let import_limit = cfg.max_imports();
    if import_count > import_limit {
        errors.push(VerificationError::TooManyImports {
            count: import_count,
            limit: import_limit,
        });
    }

    // Duplicate struct names
    let mut seen_structs = HashSet::new();
    for s in &module.structs {
        if !is_valid_identifier(&s.name, cfg) {
            errors.push(VerificationError::InvalidIdentifier {
                name: s.name.clone(),
                context: "struct definition".to_string(),
            });
        }
        if !seen_structs.insert(&s.name) {
            errors.push(VerificationError::DuplicateStructName {
                name: s.name.clone(),
            });
        }
        // Field count limits
        let field_count = s.fields.len();
        if field_count == 0 {
            errors.push(VerificationError::EmptyStruct {
                struct_name: s.name.clone(),
            });
        }
        let field_limit = cfg.max_fields();
        if field_count > field_limit {
            errors.push(VerificationError::TooManyFields {
                struct_name: s.name.clone(),
                count: field_count,
                limit: field_limit,
            });
        }

        // Field shape rules
        for field in &s.fields {
            if !is_valid_identifier(&field.name, cfg) {
                errors.push(VerificationError::InvalidIdentifier {
                    name: field.name.clone(),
                    context: format!("field in struct '{}'", s.name),
                });
            }
            if !field.ty.is_valid_field_type() {
                errors.push(VerificationError::TupleFieldType {
                    struct_name: s.name.clone(),
                    field_name: field.name.clone(),
                });
            }
            validate_type_ref(
                &field.ty,
                module,
                deps,
                &format!("field '{}' in struct '{}'", field.name, s.name),
                &mut errors,
            );
        }
    }

    // Cyclic struct field type definitions
    check_struct_cycles(module, &mut errors);

    // Duplicate function names
    let mut seen_fns = HashSet::new();
    for f in &module.functions {
        if !is_valid_identifier(&f.name, cfg)
            || RESERVED_FUNCTION_NAMES.contains(&f.name.as_str())
            || cfg.reserved_function_names().iter().any(|n| n == &f.name)
        {
            errors.push(VerificationError::InvalidIdentifier {
                name: f.name.clone(),
                context: "function definition".to_string(),
            });
        }
        if !seen_fns.insert(&f.name) {
            errors.push(VerificationError::DuplicateFunctionName {
                name: f.name.clone(),
            });
        }

        // Param and return type reference validity
        for (param_name, param_ty) in &f.params {
            validate_type_ref(
                param_ty,
                module,
                deps,
                &format!("param '{}' of function '{}'", param_name, f.name),
                &mut errors,
            );
        }
        if let Some(ret_ty) = &f.return_type {
            validate_type_ref(
                ret_ty,
                module,
                deps,
                &format!("return type of function '{}'", f.name),
                &mut errors,
            );
        }

        // Tuple return type arity
        if let Some(Type::Tuple(types)) = &f.return_type {
            let size = types.len();
            let limit = cfg.max_tuple_elements() as usize;
            if size > limit {
                errors.push(VerificationError::TupleTooLarge {
                    function: f.name.clone(),
                    size,
                    limit,
                });
            }
        }

        // local_count must cover params
        if (f.local_count as usize) < f.params.len() {
            errors.push(VerificationError::LocalCountTooSmall {
                function: f.name.clone(),
                local_count: f.local_count,
                param_count: f.params.len(),
            });
        }

        // Parameter count limit
        let param_count = f.params.len();
        let param_limit = cfg.max_params() as usize;
        if param_count > param_limit {
            errors.push(VerificationError::TooManyParams {
                function: f.name.clone(),
                count: param_count,
                limit: param_limit,
            });
        }

        // Function code size limit
        let code_size = f.code.len();
        let code_limit = cfg.max_fun_code_size();
        if code_size > code_limit {
            errors.push(VerificationError::FunctionTooLarge {
                function: f.name.clone(),
                count: code_size,
                limit: code_limit,
            });
        }

        // Local variable count limit
        let local_limit = cfg.max_locals();
        if f.local_count > local_limit {
            errors.push(VerificationError::TooManyLocals {
                function: f.name.clone(),
                count: f.local_count,
                limit: local_limit,
            });
        }

        // Per-instruction structural checks
        let code_len = f.code.len();
        for (pc, instr) in f.code.iter().enumerate() {
            match instr {
                Instruction::Load(slot) | Instruction::Store(slot) => {
                    if *slot >= f.local_count {
                        errors.push(VerificationError::SlotOutOfRange {
                            function: f.name.clone(),
                            pc,
                            slot: *slot,
                            local_count: f.local_count,
                        });
                    }
                }
                Instruction::LoadField(slot, path) | Instruction::StoreField(slot, path) => {
                    if *slot >= f.local_count {
                        errors.push(VerificationError::SlotOutOfRange {
                            function: f.name.clone(),
                            pc,
                            slot: *slot,
                            local_count: f.local_count,
                        });
                    }
                    if path.is_empty() {
                        errors.push(VerificationError::EmptyFieldPath {
                            function: f.name.clone(),
                            pc,
                        });
                    }
                }
                Instruction::Jump(offset)
                | Instruction::JumpIf(offset)
                | Instruction::JumpIfNot(offset) => {
                    if *offset <= 0 {
                        errors.push(VerificationError::BackwardJump {
                            function: f.name.clone(),
                            pc,
                            offset: *offset,
                        });
                    } else {
                        let target = pc as i64 + *offset as i64;
                        if target > code_len as i64 {
                            errors.push(VerificationError::JumpOutOfBounds {
                                function: f.name.clone(),
                                pc,
                                target: target as usize,
                                code_len,
                            });
                        }
                    }
                }
                Instruction::NewStruct {
                    type_name,
                    field_names,
                } => {
                    // Cross-module construction is forbidden
                    if let Some((_, _)) = module_ref::parse_module_ref(type_name) {
                        errors.push(VerificationError::CrossModuleStructAccess {
                            function: f.name.clone(),
                            pc,
                            type_name: type_name.clone(),
                        });
                    } else {
                        // Local struct: must exist and field list must match
                        match module.get_struct(type_name) {
                            None => {
                                errors.push(VerificationError::UndefinedStructType {
                                    function: f.name.clone(),
                                    pc,
                                    type_name: type_name.clone(),
                                });
                            }
                            Some(def) => {
                                let expected: Vec<&str> =
                                    def.fields.iter().map(|fd| fd.name.as_str()).collect();
                                let given: Vec<&str> =
                                    field_names.iter().map(|n| n.as_str()).collect();
                                if expected != given {
                                    errors.push(VerificationError::StructFieldMismatch {
                                        function: f.name.clone(),
                                        pc,
                                        type_name: type_name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                Instruction::Call(name) => {
                    if let Some((dep_addr, fn_name)) = module_ref::parse_module_ref(name) {
                        match deps.get(&dep_addr) {
                            None => {
                                // Missing dep — UndefinedFunction reported in abstract interp
                            }
                            Some(dep_mod) => match dep_mod.get_function(fn_name) {
                                None => {
                                    // UndefinedFunction reported in abstract interp
                                }
                                Some(callee) => {
                                    if !callee.is_public {
                                        errors.push(
                                            VerificationError::CrossModuleCallToPrivateFunction {
                                                function: f.name.clone(),
                                                pc,
                                                callee: name.clone(),
                                            },
                                        );
                                    }
                                }
                            },
                        }
                    }
                }
                Instruction::UnpackStruct {
                    type_name,
                    field_names,
                } => {
                    if module_ref::parse_module_ref(type_name).is_some() {
                        errors.push(VerificationError::CrossModuleStructAccess {
                            function: f.name.clone(),
                            pc,
                            type_name: type_name.clone(),
                        });
                    } else {
                        match module.get_struct(type_name) {
                            None => {
                                errors.push(VerificationError::UndefinedStructType {
                                    function: f.name.clone(),
                                    pc,
                                    type_name: type_name.clone(),
                                });
                            }
                            Some(def) => {
                                let expected: Vec<&str> =
                                    def.fields.iter().map(|fd| fd.name.as_str()).collect();
                                let given: Vec<&str> =
                                    field_names.iter().map(|n| n.as_str()).collect();
                                if expected != given {
                                    errors.push(VerificationError::StructFieldMismatch {
                                        function: f.name.clone(),
                                        pc,
                                        type_name: type_name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
                Instruction::MakeTuple(n) | Instruction::UnpackTuple(n) => {
                    let size = *n as usize;
                    let limit = cfg.max_tuple_elements() as usize;
                    if size > limit {
                        errors.push(VerificationError::TupleTooLarge {
                            function: f.name.clone(),
                            size,
                            limit,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    errors
}

/// Validate that every `Type::Struct` name in `ty` resolves to either a local
/// struct defined in `module` or a fully-qualified `@0xHEX::Name` whose address
/// is a registered import (and, when the dep module is available, whose struct
/// name exists there).
fn validate_type_ref(
    ty: &Type,
    module: &Module,
    deps: &HashMap<Address, &Module>,
    context: &str,
    errors: &mut Vec<VerificationError>,
) {
    match ty {
        Type::Bool | Type::U64 | Type::Address | Type::Str => {}
        Type::Struct(name) => {
            if let Some((dep_addr, struct_name)) = module_ref::parse_module_ref(name) {
                if !module.imports.contains(&dep_addr) {
                    errors.push(VerificationError::UnresolvedTypeReference {
                        context: context.to_string(),
                        type_name: name.clone(),
                    });
                } else if let Some(dep_mod) = deps.get(&dep_addr) {
                    match dep_mod.get_struct(struct_name) {
                        None => errors.push(VerificationError::UnresolvedTypeReference {
                            context: context.to_string(),
                            type_name: name.clone(),
                        }),
                        // Only `pub` structs are visible across modules — referencing a
                        // private dep struct as a type is rejected (mirrors the compiler).
                        Some(def) if !def.is_public => {
                            errors.push(VerificationError::CrossModulePrivateStructReference {
                                context: context.to_string(),
                                type_name: name.clone(),
                            })
                        }
                        Some(_) => {}
                    }
                }
                // dep not in the provided deps map → can't verify further, assume valid
            } else if module.get_struct(name).is_none() {
                errors.push(VerificationError::UnresolvedTypeReference {
                    context: context.to_string(),
                    type_name: name.clone(),
                });
            }
        }
        Type::Tuple(types) => {
            for t in types {
                validate_type_ref(t, module, deps, context, errors);
            }
        }
    }
}

/// Detect cycles in struct field type references using DFS.
///
/// A cycle like `struct A { x: B }` + `struct B { y: A }` makes it impossible
/// to construct either struct, but a hand-crafted module could express it. We
/// reject it here so the abstract interpreter never has to reason about it.
fn check_struct_cycles(module: &Module, errors: &mut Vec<VerificationError>) {
    let local_structs: HashSet<String> = module.structs.iter().map(|s| s.name.clone()).collect();
    let mut visiting = HashSet::<String>::new();
    let mut visited = HashSet::<String>::new();

    for s in &module.structs {
        if !visited.contains(&s.name) {
            struct_dfs(
                &s.name,
                module,
                &local_structs,
                &mut visiting,
                &mut visited,
                errors,
            );
        }
    }
}

fn struct_dfs(
    name: &str,
    module: &Module,
    local_structs: &HashSet<String>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    errors: &mut Vec<VerificationError>,
) {
    visiting.insert(name.to_string());
    if let Some(def) = module.get_struct(name) {
        for field in &def.fields {
            if let Type::Struct(ty_name) = &field.ty
                && module_ref::parse_module_ref(ty_name).is_none()
                && local_structs.contains(ty_name)
            {
                if visiting.contains(ty_name) {
                    // ty_name is the ancestor in the current path — report it as
                    // the struct whose definition is cyclic.
                    errors.push(VerificationError::CyclicStructDefinition {
                        struct_name: ty_name.clone(),
                    });
                } else if !visited.contains(ty_name) {
                    struct_dfs(ty_name, module, local_structs, visiting, visited, errors);
                }
            }
        }
    }
    visiting.remove(name);
    visited.insert(name.to_string());
}
