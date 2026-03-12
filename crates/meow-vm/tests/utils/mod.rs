#![allow(dead_code)]

use meow_vm::{
    compiler::Compiler,
    module::Module,
    types::Value,
    vm::{GasMeter, GasSchedule, NativeFnEntry, NativeResult, Vm},
};

/// Compile a source snippet, run `fn_name` with `args`, and return the return value.
/// Panics if compilation or execution fails.
pub fn run(source: &str, fn_name: &str, args: Vec<Value>) -> Option<Value> {
    let vm = Vm::new(compile(source), vec![], GasSchedule::default());
    let mut gas = GasMeter::unlimited();
    vm.call(fn_name, args, &mut gas)
        .expect("execution failed")
        .return_value
}

/// Compile a source snippet. Panics if compilation fails.
pub fn compile(source: &str) -> Module {
    Compiler::compile("test", source).expect("compilation failed")
}

/// Build a `Vm` from source with the given native functions.
pub fn vm_with_natives(source: &str, natives: Vec<NativeFnEntry>) -> Vm {
    Vm::new(compile(source), natives, GasSchedule::default())
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
