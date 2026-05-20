//! Lightweight call harness for unit tests.
//!
//! Sets up real native functions and a shared execution context but skips transaction
//! overhead, making it easy to test module logic in isolation.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use meow_types::{address::Address, config, digest::Digest, identifier::Identifier};
use meow_vm::{Vm, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_types::config::VmConfig;

use crate::{Module, Value, context::Context, external_context::ExternalContext, natives};

pub use meow_vm::error::VmError;

/// The result type related to the runner.
pub type Result<T> = std::result::Result<T, VmError>;

/// Result of a test VM run — the call outcome plus all native side-effects.
#[derive(Debug)]
pub struct RunResult {
    /// The return value of the call, if any.
    pub return_value: Option<Value>,
    /// Objects transferred out during the call: `(object, new_owner)`.
    pub transfers: Vec<(Value, Address)>,
    /// IDs of objects destroyed during the call.
    pub destroyed: Vec<Address>,
    /// Gas spent during the call.
    pub gas_spent: u64,
}

/// Run a compiled module function with a fixed context, real natives, and unlimited gas.
///
/// The first element of the `module` tuple is the on-chain address of the module, used to
/// qualify struct type names produced by `NewStruct` instructions (e.g. `@0x20::MeowCoin`).
pub fn run(
    module: (Address, Module),
    fn_name: &Identifier,
    args: Vec<Value>,
    deps: HashMap<Address, Module>,
    external_context: ExternalContext,
) -> Result<RunResult> {
    let mut gas = GasMeter::unlimited();
    let vm_config = config::vm_config();

    run_inner(
        module,
        fn_name,
        args,
        deps,
        &mut gas,
        vm_config,
        external_context,
    )
}

/// Like [`run`] but uses the privileged VM config, which allows calling private functions.
/// Use this to test functions that are intentionally private (e.g. `mint`).
pub fn run_privileged(
    module: (Address, Module),
    fn_name: &Identifier,
    args: Vec<Value>,
    deps: HashMap<Address, Module>,
    external_context: ExternalContext,
) -> Result<RunResult> {
    let mut gas = GasMeter::unlimited();
    let vm_config = config::vm_config_privileged();

    run_inner(
        module,
        fn_name,
        args,
        deps,
        &mut gas,
        vm_config,
        external_context,
    )
}

/// Run a compiled module function with a fixed context, real natives, and the given gas meter.
fn run_inner(
    module: (Address, Module),
    fn_name: &Identifier,
    args: Vec<Value>,
    deps: HashMap<Address, Module>,
    gas: &mut GasMeter,
    vm_config: VmConfig,
    external_context: ExternalContext,
) -> Result<RunResult> {
    let ctx = Rc::new(RefCell::new(Context::new(
        Address::ZERO,
        Digest::ZERO,
        *external_context.rand_seed(),
        external_context.timestamp(),
    )));
    let natives = natives::build_natives(ctx.clone());
    let deps = deps.into_iter().map(|(addr, m)| (addr.into(), m)).collect();

    let vm = Vm::new(
        (module.0.into(), module.1),
        natives,
        GasSchedule::default(),
        deps,
        vm_config,
    );

    let call_result = vm.call(fn_name.as_ref(), args, gas)?;

    let ctx = ctx.borrow();
    Ok(RunResult {
        return_value: call_result.return_value,
        transfers: ctx.transfers().to_vec(),
        destroyed: ctx.destroyed().to_vec(),
        gas_spent: gas.spent(),
    })
}
