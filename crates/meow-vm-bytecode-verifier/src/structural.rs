//! Phase 1 of bytecode verification: purely syntactic checks that need no stack state.
//!
//! Runs before abstract interpretation because it is cheaper and its errors are independent
//! of type information. If Phase 1 finds errors, Phase 2 is skipped entirely.

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
        }
    }

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
                Instruction::LoadField(slot, _) | Instruction::StoreField(slot, _) => {
                    if *slot >= f.local_count {
                        errors.push(VerificationError::SlotOutOfRange {
                            function: f.name.clone(),
                            pc,
                            slot: *slot,
                            local_count: f.local_count,
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
                        errors.push(VerificationError::CrossModuleStructConstruction {
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
                                    errors.push(VerificationError::NewStructFieldMismatch {
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
                        errors.push(VerificationError::CrossModuleStructConstruction {
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
                                    errors.push(VerificationError::NewStructFieldMismatch {
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
