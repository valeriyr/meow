#![allow(dead_code)]

use meow_vm::{NativeFnEntry, NativeResult, Vm, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_compiler::Compiler;
use meow_vm_types::{config::Config, module::Module, types::Value};

/// Compile a source snippet, run `fn_name` with `args`, and return the return value.
/// Panics if compilation or execution fails.
pub fn run(source: &str, fn_name: &str, args: Vec<Value>) -> Option<Value> {
    let vm = vm(compile(source));
    let mut gas = GasMeter::unlimited();
    vm.call(fn_name, args, &mut gas)
        .expect("execution failed")
        .return_value
}

/// Compile a source snippet. Panics if compilation fails.
pub fn compile(source: &str) -> Module {
    Compiler::compile("test", source, Config::default()).expect("compilation failed")
}

/// Builds a `Vm` with a module.
pub fn vm(module: Module) -> Vm {
    Vm::new(module, vec![], GasSchedule::default(), Config::default())
}

/// Build a `Vm` from source with the given native functions.
pub fn vm_with_natives(source: &str, natives: Vec<NativeFnEntry>) -> Vm {
    Vm::new(
        compile(source),
        natives,
        GasSchedule::default(),
        Config::default(),
    )
}

//
// ─── Common native functions ───
//

pub fn fresh_id_native() -> NativeFnEntry {
    NativeFnEntry {
        name: "meow_vm_fresh_id".to_string(),
        param_count: 0,
        gas_cost: 1,
        func: Box::new(|_| NativeResult::Return(Some(Value::Address([0u8; 32])))),
    }
}

pub fn consume_native(name: &str) -> NativeFnEntry {
    NativeFnEntry {
        name: name.to_string(),
        param_count: 1,
        gas_cost: 1,
        func: Box::new(|_| NativeResult::Return(None)),
    }
}
