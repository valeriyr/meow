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

use crate::{error::VerificationError, natives::NativeSignature};

/// Phase 1 — purely structural checks that require no stack simulation.
pub(crate) fn check_module(
    module: &Module,
    deps: &HashMap<Address, &Module>,
    _natives: &[NativeSignature],
) -> Vec<VerificationError> {
    let cfg = CompilerConfig::default();
    let mut errors = Vec::new();

    // Module name
    if !is_valid_identifier(&module.name, &cfg) {
        errors.push(VerificationError::InvalidIdentifier {
            name: module.name.clone(),
            context: "module declaration".to_string(),
        });
    }

    // Duplicate struct names
    let mut seen_structs = HashSet::new();
    for s in &module.structs {
        if !is_valid_identifier(&s.name, &cfg) {
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
        // Field shape rules
        for field in &s.fields {
            if !is_valid_identifier(&field.name, &cfg) {
                errors.push(VerificationError::InvalidIdentifier {
                    name: field.name.clone(),
                    context: format!("field in struct '{}'", s.name),
                });
            }
            if matches!(field.ty, Type::Object(_)) {
                errors.push(VerificationError::ObjectAsFieldType {
                    struct_name: s.name.clone(),
                    field_name: field.name.clone(),
                });
            }
        }
        // Object first-field constraint
        if s.is_object {
            let first = s.fields.first();
            let valid = first
                .map(|f| f.name == "id" && f.ty == Type::Address)
                .unwrap_or(false);
            if !valid {
                errors.push(VerificationError::ObjectMissingIdField {
                    struct_name: s.name.clone(),
                });
            }
        }
    }

    // Duplicate function names
    let mut seen_fns = HashSet::new();
    for f in &module.functions {
        if !is_valid_identifier(&f.name, &cfg) || RESERVED_FUNCTION_NAMES.contains(&f.name.as_str())
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

        // local_count must cover params
        if (f.local_count as usize) < f.params.len() {
            errors.push(VerificationError::LocalCountTooSmall {
                function: f.name.clone(),
                local_count: f.local_count,
                param_count: f.params.len(),
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
                                errors.push(VerificationError::UnknownStructType {
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
                _ => {}
            }
        }
    }

    errors
}
