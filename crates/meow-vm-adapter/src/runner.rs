use std::{cell::RefCell, rc::Rc};

use meow_types::{address::Address, config, identifier::Identifier};
use meow_vm::{Vm, gas_meter::GasMeter, gas_schedule::GasSchedule};

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
    pub transfers: Vec<(Value, Address)>,
    /// Objects destroyed during the call.
    pub destroyed: Vec<Value>,
    /// Gas spent during the call.
    pub gas_spent: u64,
}

/// Run a compiled module function with a fixed context, real natives, and unlimited gas.
pub fn run(module: Module, fn_name: &Identifier, args: Vec<Value>) -> Result<RunResult> {
    let mut gas = GasMeter::unlimited();
    run_with_gas_meter(module, fn_name, args, &mut gas)
}

/// Run a compiled module function with a fixed context, real natives, and the given gas meter.
pub fn run_with_gas_meter(
    module: Module,
    fn_name: &Identifier,
    args: Vec<Value>,
    gas: &mut GasMeter,
) -> Result<RunResult> {
    let ctx = Rc::new(RefCell::new(Context::default()));
    let natives = natives::build_natives(ctx.clone());
    let vm = Vm::new(module, natives, GasSchedule::default(), config::vm_config());

    let call_result = vm.call(fn_name.as_ref(), args, gas)?;

    let ctx = ctx.borrow();
    Ok(RunResult {
        return_value: call_result.return_value,
        final_args: call_result.final_args,
        transfers: ctx.transfers().to_vec(),
        destroyed: ctx.destroyed().to_vec(),
        gas_spent: gas.spent(),
    })
}
