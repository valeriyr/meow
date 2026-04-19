#![allow(dead_code)]

use std::collections::HashMap;

use meow_vm_bytecode_verifier::{NativeParam, NativeSignature, VerificationError};
use meow_vm_compiler::Compiler;
use meow_vm_types::{
    address::Address, bytecode::Instruction, config::CompilerConfig, module::Module, types::Type,
};

pub fn compile(src: &str) -> Module {
    Compiler::compile(src, &[], &[], CompilerConfig::default())
        .unwrap_or_else(|e| panic!("compile failed: {e}"))
}

pub fn compile_with_deps(src: &str, deps: &[(Address, &Module)]) -> Module {
    Compiler::compile(src, deps, &[], CompilerConfig::default())
        .unwrap_or_else(|e| panic!("compile_with_deps failed: {e}"))
}

pub fn verify_ok(module: &Module, deps: &HashMap<Address, &Module>) {
    meow_vm_bytecode_verifier::verify(module, deps, &test_natives(), &CompilerConfig::default())
        .unwrap_or_else(|errs| panic!("expected verification OK, got errors:\n{errs:#?}"));
}

pub fn verify_errors(module: &Module, deps: &HashMap<Address, &Module>) -> Vec<VerificationError> {
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

pub fn no_deps() -> HashMap<Address, &'static Module> {
    HashMap::new()
}

/// Simplified native signatures for language-level verifier tests.
///
/// These use `AnyStruct` for `meow_vm_transfer` and `meow_vm_destroy` to keep
/// tests independent of the adapter's specific `meow_object::Id` type. The
/// adapter enforces actual type constraints separately via its own verifier.
fn test_natives() -> Vec<NativeSignature> {
    vec![
        NativeSignature::new("meow_vm_fresh_id", vec![], Some(Type::Address)),
        NativeSignature::new(
            "meow_vm_transfer",
            vec![NativeParam::AnyStruct, NativeParam::Concrete(Type::Address)],
            None,
        ),
        NativeSignature::new("meow_vm_destroy", vec![NativeParam::AnyStruct], None),
        NativeSignature::new("meow_vm_sender", vec![], Some(Type::Address)),
        NativeSignature::new("meow_vm_rand", vec![], Some(Type::U64)),
        NativeSignature::new("meow_vm_timestamp", vec![], Some(Type::U64)),
    ]
}
