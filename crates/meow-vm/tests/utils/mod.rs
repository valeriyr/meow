#![allow(dead_code)]

use std::collections::HashMap;

use meow_vm::{Vm, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_compiler::Compiler;
use meow_vm_types::{
    address::Address,
    config::{CompilerConfig, VmConfig},
    module::Module,
    natives::{NativeFnEntry, NativeResult, NativeSig},
    types::Value,
};

/// Compile a source snippet. Panics if compilation fails.
pub fn compile(source: &str) -> Module {
    Compiler::compile(source, &[], &[], CompilerConfig::default()).expect("compilation failed")
}

/// Compile a module that depends on `deps`. Panics if compilation fails.
pub fn compile_with_deps(source: &str, deps: &[(Address, &Module)]) -> Module {
    Compiler::compile(source, deps, &[], CompilerConfig::default()).expect("compilation failed")
}

/// Builds a `Vm` with a module and no deps.
pub fn vm(module: Module) -> Vm {
    Vm::new(
        module,
        vec![],
        GasSchedule::default(),
        HashMap::new(),
        VmConfig::default(),
    )
}

/// Builds a `Vm` with a module and dep modules.
pub fn vm_with_deps(module: Module, deps: HashMap<Address, Module>) -> Vm {
    Vm::new(
        module,
        vec![],
        GasSchedule::default(),
        deps,
        VmConfig::default(),
    )
}

/// Build a `Vm` from source.
pub fn vm_with_source(source: &str) -> Vm {
    vm_with_natives(source, vec![])
}

/// Build a `Vm` from source with the given native functions.
pub fn vm_with_natives(source: &str, natives: Vec<NativeFnEntry>) -> Vm {
    let native_sigs: Vec<NativeSig> = natives
        .iter()
        .map(|n| NativeSig {
            name: n.name.clone(),
            params: n.params.clone(),
            return_type: n.return_type.clone(),
        })
        .collect();
    let module = Compiler::compile(source, &[], &native_sigs, CompilerConfig::default())
        .expect("compilation failed");
    Vm::new(
        module,
        natives,
        GasSchedule::default(),
        HashMap::new(),
        VmConfig::default(),
    )
}

/// Build a `Vm` with dep modules and native functions.
pub fn vm_with_deps_and_natives(
    module: Module,
    deps: HashMap<Address, Module>,
    natives: Vec<NativeFnEntry>,
) -> Vm {
    Vm::new(
        module,
        natives,
        GasSchedule::default(),
        deps,
        VmConfig::default(),
    )
}

/// Compile a source snippet, run `fn_name` with `args`, and return the return value.
/// Panics if compilation or execution fails.
pub fn run(source: &str, fn_name: &str, args: Vec<Value>) -> Option<Value> {
    let vm = vm(compile(source));
    let mut gas = GasMeter::unlimited();
    vm.call(fn_name, args, &mut gas)
        .expect("execution failed")
        .return_value
}

//
// ─── Common native functions ───
//

pub fn consume_native(name: &str) -> NativeFnEntry {
    NativeFnEntry {
        name: name.to_string(),
        params: vec![None], // any struct
        return_type: None,
        gas_cost: 1,
        func: Box::new(|_| NativeResult::Return(None)),
    }
}
