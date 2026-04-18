#![allow(dead_code)]

use std::collections::HashMap;

use meow_vm_bytecode_verifier::{
    BytecodeVerifier, NativeParam, NativeSignature, VerificationError,
};
use meow_vm_compiler::Compiler;
use meow_vm_types::{
    address::Address, bytecode::Instruction, config::CompilerConfig, module::Module, types::Type,
};

pub fn compile(src: &str) -> Module {
    Compiler::compile(src, &[], CompilerConfig::default())
        .unwrap_or_else(|e| panic!("compile failed: {e}"))
}

pub fn compile_with_deps(src: &str, deps: &[(Address, &Module)]) -> Module {
    Compiler::compile(src, deps, CompilerConfig::default())
        .unwrap_or_else(|e| panic!("compile_with_deps failed: {e}"))
}

pub fn verify_ok(module: &Module, deps: &HashMap<Address, &Module>) {
    BytecodeVerifier::new(adapter_natives())
        .verify(module, deps)
        .unwrap_or_else(|errs| panic!("expected verification OK, got errors:\n{errs:#?}"));
}

pub fn verify_errors(module: &Module, deps: &HashMap<Address, &Module>) -> Vec<VerificationError> {
    BytecodeVerifier::new(adapter_natives())
        .verify(module, deps)
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

fn adapter_natives() -> Vec<NativeSignature> {
    vec![
        NativeSignature::new("meow_vm_fresh_id", vec![], Some(Type::Address)),
        NativeSignature::new(
            "meow_vm_transfer",
            vec![NativeParam::AnyObject, NativeParam::Concrete(Type::Address)],
            None,
        ),
        NativeSignature::new("meow_vm_destroy", vec![NativeParam::AnyObject], None),
        NativeSignature::new("meow_vm_sender", vec![], Some(Type::Address)),
        NativeSignature::new("meow_vm_rand", vec![], Some(Type::U64)),
        NativeSignature::new("meow_vm_timestamp", vec![], Some(Type::U64)),
    ]
}
