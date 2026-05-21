//! Shared test helpers for bytecode verifier tests.

#![allow(dead_code)]

use std::collections::HashMap;

use meow_vm_bytecode_verifier::{NativeSig, VerificationError};
use meow_vm_compiler::Compiler;
use meow_vm_types::{
    address::Address, bytecode::Instruction, config::CompilerConfig, module::Module,
    natives::NativeParam, types::Type,
};

pub fn compile(src: &str) -> Module {
    Compiler::compile(src, &[], &[], CompilerConfig::default())
        .unwrap_or_else(|e| panic!("compile failed: {e}"))
}

pub fn compile_with_deps(src: &str, deps: &[(Address, &Module)]) -> Module {
    Compiler::compile(src, deps, &[], CompilerConfig::default())
        .unwrap_or_else(|e| panic!("compile_with_deps failed: {e}"))
}

pub fn verify_ok(module: &Module) {
    meow_vm_bytecode_verifier::verify(
        module,
        &HashMap::new(),
        &test_natives(),
        &CompilerConfig::default(),
    )
    .unwrap_or_else(|errs| panic!("expected verification OK, got errors:\n{errs:#?}"));
}

pub fn verify_ok_with_deps(module: &Module, deps: &HashMap<Address, &Module>) {
    meow_vm_bytecode_verifier::verify(module, deps, &test_natives(), &CompilerConfig::default())
        .unwrap_or_else(|errs| panic!("expected verification OK, got errors:\n{errs:#?}"));
}

pub fn verify_errors(module: &Module) -> Vec<VerificationError> {
    meow_vm_bytecode_verifier::verify(
        module,
        &HashMap::new(),
        &test_natives(),
        &CompilerConfig::default(),
    )
    .expect_err("expected verification errors but verification passed")
}

pub fn verify_errors_with_deps(
    module: &Module,
    deps: &HashMap<Address, &Module>,
) -> Vec<VerificationError> {
    meow_vm_bytecode_verifier::verify(module, deps, &test_natives(), &CompilerConfig::default())
        .expect_err("expected verification errors but verification passed")
}

pub fn tamper(module: &mut Module, fn_name: &str, f: impl FnOnce(&mut Vec<Instruction>)) {
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == fn_name)
        .unwrap_or_else(|| panic!("function '{fn_name}' not found"));
    f(&mut func.code);
}

/// Generic native signatures used across verifier tests.
///
/// These are not tied to any adapter — they exist solely to give the abstract
/// interpreter enough information to type-check the few tests that emit Call
/// instructions to non-local functions.
fn test_natives() -> Vec<NativeSig> {
    vec![
        NativeSig {
            name: "addr_native".to_string(),
            params: vec![],
            return_type: Some(Type::Address),
        },
        NativeSig {
            name: "u64_native".to_string(),
            params: vec![],
            return_type: Some(Type::U64),
        },
        NativeSig {
            name: "consume_native".to_string(),
            params: vec![NativeParam::AnyStruct],
            return_type: None,
        },
    ]
}
