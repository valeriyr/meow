mod effects;
mod gas;
mod resolvers;
mod versioning;

pub mod error;

use std::{cell::RefCell, rc::Rc};

use meow_types::{
    address::Address,
    config::{self, MAX_BCS_SERIALIZED_MODULE_SIZE},
    digest::Digest,
    object::Object,
    system_framework::meow_coin,
    transaction::{
        Transaction,
        call::Call,
        execution_result::{ExecutionResult, ExecutionStatus},
        transaction_type::TransactionType,
    },
};
use meow_vm::{Vm, error::VmError, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_types::{config::VmConfig, module::Module};

use crate::{
    context::Context, executor::error::ExecutorError, external_context::ExternalContext, natives,
};

/// Gas cost per byte of module bytecode when publishing.
const GAS_PER_MODULE_BYTE: u64 = 10;
/// Base gas cost for executing a transaction (covers overhead of deserialization, etc).
const BASE_TRANSACTION_GAS_COST: u64 = 1000;

/// The result type related to the executor.
pub type Result<T> = std::result::Result<T, ExecutorError>;

/// Execute a transaction against a set of input objects.
///
/// The `inputs` slice must contain the module object (identified by
/// `ObjectType::Module`), the gas coin, and any object arguments the
/// transaction references.
///
/// Gas is always spent and the gas coin is always returned as a changed object,
/// even when execution fails.
pub fn execute(
    transaction: &Transaction,
    inputs: Vec<Object>,
    external_context: &ExternalContext,
) -> Result<ExecutionResult> {
    let sender = transaction.sender();
    let tx_digest = transaction.digest();

    let gas_coin = resolvers::resolve_gas_coin_object(transaction.gas_coin(), sender, &inputs)?;

    // The whole gas coin balance is used as the gas budget for the transaction.
    let gas_budget = meow_coin::gas_meow_coin_balance(gas_coin)
        .expect("it is expected to have a valid balance field in the gas coin object");
    let mut gas = GasMeter::new(gas_budget);

    let result = match gas.charge(BASE_TRANSACTION_GAS_COST) {
        Ok(_) => match transaction.type_() {
            TransactionType::MeowCall(call) => execute_meow_call(
                sender,
                &tx_digest,
                call,
                &inputs,
                &mut gas,
                config::vm_config(),
                external_context,
            ),
            TransactionType::MeowModulePublish(module) => {
                execute_meow_module_publish(&tx_digest, module, &mut gas)
            }
        },
        Err(e) => ExecutionResult::failure(e.to_string(), tx_digest),
    };

    // Gas is spent regardless of whether execution succeeded or failed.
    Ok(gas::apply_gas_spending(
        result,
        gas_coin,
        gas.spent(),
        &tx_digest,
    ))
}

/// Execute a genesis transaction.
///
/// This is a special code path that executes a transaction without charging any gas,
/// and is used only for building the genesis state of the chain.
pub fn execute_genesis_transaction(
    transaction: &Transaction,
    inputs: Vec<Object>,
) -> Result<ExecutionResult> {
    let sender = transaction.sender();
    let tx_digest = transaction.digest();

    let mut gas_meter = GasMeter::unlimited();

    let result = match transaction.type_() {
        TransactionType::MeowCall(call) => execute_meow_call(
            sender,
            &tx_digest,
            call,
            &inputs,
            &mut gas_meter,
            config::vm_config_privileged(),
            &ExternalContext::default(),
        ),
        TransactionType::MeowModulePublish(module) => {
            execute_meow_module_publish(&tx_digest, module, &mut gas_meter)
        }
    };

    Ok(result.with_gas_used(gas_meter.spent()))
}

/// Execute a `meow_call` transaction.
fn execute_meow_call(
    sender: &Address,
    tx_digest: &Digest,
    call: &Call,
    inputs: &[Object],
    gas: &mut GasMeter,
    vm_config: VmConfig,
    external_context: &ExternalContext,
) -> ExecutionResult {
    let module_address = call.module();

    // Resolve the main module (identified by its on-chain address).
    let module = match resolvers::resolve_module(inputs, module_address) {
        Ok(m) => m,
        Err(e) => {
            return ExecutionResult::failure(e, *tx_digest);
        }
    };

    // Resolve dependency modules declared in the main module's imports.
    let deps = match resolvers::resolve_dep_modules(inputs, &module.imports) {
        Ok(d) => d,
        Err(e) => {
            return ExecutionResult::failure(e, *tx_digest);
        }
    };

    // Find the function in the module.
    let fn_name = call.function().as_ref();
    let func = match module.get_function(fn_name) {
        Some(f) => f,
        None => {
            return ExecutionResult::failure(
                format!("function '{fn_name}' not found in module"),
                *tx_digest,
            );
        }
    };

    let (vm_args, object_args) = match resolvers::resolve_args(call, func, inputs) {
        Ok(res) => res,
        Err(e) => {
            return ExecutionResult::failure(e, *tx_digest);
        }
    };

    // Build executor context and native functions.
    let ctx = Rc::new(RefCell::new(Context::new(
        *sender,
        *tx_digest,
        *external_context.rand_seed(),
        external_context.timestamp(),
    )));
    let natives = natives::build_natives(ctx.clone());
    let vm = Vm::new(module, natives, GasSchedule::default(), deps, vm_config);

    // Execute the function.
    let call_result = match vm.call(fn_name, vm_args, gas) {
        Ok(r) => r,
        Err(VmError::Aborted { message, .. }) => {
            return ExecutionResult::failure(message, *tx_digest);
        }
        Err(e) => {
            return ExecutionResult::failure(e.to_string(), *tx_digest);
        }
    };

    // Collect object effects.
    match effects::collect_object_effects(
        &ctx.borrow(),
        &call_result,
        &object_args,
        module_address,
        tx_digest,
    ) {
        Ok((created, changed, destroyed)) => ExecutionResult::new(
            ExecutionStatus::Success,
            *tx_digest,
            created,
            changed,
            destroyed,
        ),
        Err(e) => ExecutionResult::new(
            ExecutionStatus::Failure(e),
            *tx_digest,
            vec![],
            vec![],
            vec![],
        ),
    }
}

/// Execute a `meow_module_publish` transaction.
fn execute_meow_module_publish(
    tx_digest: &Digest,
    module: &[u8],
    gas: &mut GasMeter,
) -> ExecutionResult {
    let module_size = module.len();

    let max_module_size = MAX_BCS_SERIALIZED_MODULE_SIZE;
    if module_size > max_module_size {
        return ExecutionResult::failure(
            format!(
                "module size {} bytes exceeds maximum of {} bytes",
                module_size, max_module_size
            ),
            *tx_digest,
        );
    }

    if let Err(e) = bcs::from_bytes::<Module>(module) {
        return ExecutionResult::failure(format!("failed to deserialize module: {e}"), *tx_digest);
    }

    let cost = module_size as u64 * GAS_PER_MODULE_BYTE;
    if let Err(e) = gas.charge(cost) {
        return ExecutionResult::failure(e.to_string(), *tx_digest);
    }

    let module_address = Address::derive(*tx_digest, 0, 0);

    let created = vec![Object::fresh_module(
        module_address,
        *tx_digest,
        module.to_vec(),
    )];

    ExecutionResult::new(
        ExecutionStatus::Success,
        *tx_digest,
        created,
        vec![],
        vec![],
    )
}
