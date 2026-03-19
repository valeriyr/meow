use std::{cell::RefCell, rc::Rc};

use meow_types::{address::Address, digest::Digest};
use meow_vm::{Vm, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_types::config::VmConfig;

use crate::{Module, Value, context::Context, natives};

pub use meow_vm::error::VmError;

/// The result type related to the runner.
pub type Result<T> = std::result::Result<T, VmError>;

/// Result of a test VM run — the call outcome plus all native side-effects.
#[derive(Debug)]
pub struct RunResult {
    /// The return value of the call, if any.
    pub return_value: Option<Value>,
    /// Post-call slot state: `None` means the object was consumed (moved out).
    pub final_args: Vec<Option<Value>>,
    /// Objects transferred out during the call: `(object, new_owner)`.
    pub transfers: Vec<(Value, [u8; 32])>,
    /// Objects destroyed during the call.
    pub destroyed: Vec<Value>,
}

/// Run a compiled module function with a fixed context, real natives, and unlimited gas.
pub fn run(module: Module, fn_name: &str, args: Vec<Value>) -> Result<RunResult> {
    let mut gas = GasMeter::unlimited();
    run_with_gas_meter(module, fn_name, args, &mut gas)
}

/// Run a compiled module function with a fixed context, real natives, and the given gas meter.
pub fn run_with_gas_meter(
    module: Module,
    fn_name: &str,
    args: Vec<Value>,
    gas: &mut GasMeter,
) -> Result<RunResult> {
    let ctx = Rc::new(RefCell::new(Context::new(Address::ZERO, Digest::ZERO)));
    let natives = natives::build_natives(ctx.clone());
    let vm = Vm::new(module, natives, GasSchedule::default(), VmConfig::default());

    let call_result = vm.call(fn_name, args, gas)?;

    let ctx = ctx.borrow();
    Ok(RunResult {
        return_value: call_result.return_value,
        final_args: call_result.final_args,
        transfers: ctx
            .transfers()
            .to_owned()
            .into_iter()
            .map(|(v, o)| (v.clone(), o.into()))
            .collect(),
        destroyed: ctx.destroyed().to_vec(),
    })
}
