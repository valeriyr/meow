use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use meow_types::{
    address::Address,
    digest::Digest,
    object::{
        Object, object_owner::ObjectOwner, object_type::ObjectType, object_version::ObjectVersion,
    },
    transaction::{
        Transaction,
        call::{Call, Input},
        execution_result::{ExecutionResult, ExecutionStatus},
        transaction_type::TransactionType,
    },
};
use meow_vm::{
    module::Module,
    types::{Type, Value},
    vm::{GasMeter, GasSchedule, Vm, VmCallResult, error::VmError},
};

use crate::{
    context::Context,
    convert::{object_to_vm_object_value, vm_object_value_to_object},
    natives,
};

/// Maximum serialized byte size of a compiled [`crate::module::Module`].
pub const MAX_MODULE_SIZE_BYTES: usize = 512 * 1024; // 512 KiB

/// Execute a transaction against a set of input objects.
///
/// The `inputs` slice must contain the module object (identified by
/// `ObjectType::Module`), the gas coin, and any object arguments the
/// transaction references.
///
/// Gas is always spent and the gas coin is always returned as a changed object,
/// even when execution fails.
pub fn execute(transaction: &Transaction, inputs: Vec<Object>) -> ExecutionResult {
    let sender = transaction.sender();
    let tx_digest = &transaction.digest();

    // Locate and validate the gas coin.
    let gas_coin = match inputs
        .iter()
        .find(|o| o.address() == transaction.gas_coin())
    {
        Some(o) => o,
        None => return ExecutionResult::failure("gas coin not found in inputs", *tx_digest),
    };
    if !gas_coin.is_gas_coin() {
        return ExecutionResult::failure("gas coin object is not a valid gas coin", *tx_digest);
    }

    let gas_budget = gas_coin_balance(gas_coin);
    let mut gas = GasMeter::new(gas_budget);

    let result = match transaction.type_() {
        TransactionType::MeowCall(call) => {
            execute_meow_call(sender, tx_digest, call, &inputs, &mut gas)
        }
        TransactionType::MeowModulePublish(module) => {
            execute_meow_module_publish(sender, tx_digest, module, &inputs, &mut gas)
        }
    };

    // Gas is spent regardless of whether execution succeeded or failed.
    apply_gas_spending(result, gas_coin, gas.consumed(), tx_digest)
}

fn execute_meow_call(
    sender: &Address,
    tx_digest: &Digest,
    call: &Call,
    inputs: &[Object],
    gas: &mut GasMeter,
) -> ExecutionResult {
    let module_address = call.module();

    // Find the module object in inputs.
    let module_obj = match inputs
        .iter()
        .find(|o| matches!(o.type_(), ObjectType::Module))
    {
        Some(o) => o,
        None => {
            return ExecutionResult::failure("module object not found in inputs", *tx_digest);
        }
    };

    // Deserialize the module.
    let module: Module = match bcs::from_bytes(module_obj.content()) {
        Ok(m) => m,
        Err(e) => {
            return ExecutionResult::failure(
                format!("failed to deserialize module: {e}"),
                *tx_digest,
            );
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

    // Resolve call arguments to VM values.
    let call_args_inputs = call.arguments();
    if call_args_inputs.len() != func.params.len() {
        return ExecutionResult::failure(
            format!(
                "argument count mismatch: expected {}, got {}",
                func.params.len(),
                call_args_inputs.len()
            ),
            *tx_digest,
        );
    }

    let mut vm_args: Vec<Value> = Vec::with_capacity(call_args_inputs.len());
    // Track which arg indices are object inputs (for final_args analysis).
    let mut input_object_args: Vec<(usize, &Object)> = Vec::new();

    for (i, (input, (_param_name, param_type))) in
        call_args_inputs.iter().zip(func.params.iter()).enumerate()
    {
        match resolve_arg(input, param_type, inputs) {
            Ok(v) => {
                if let Input::Object(object_ref) = input {
                    if let Some(obj) = inputs.iter().find(|o| o.address() == object_ref.address()) {
                        input_object_args.push((i, obj));
                    }
                }
                vm_args.push(v);
            }
            Err(e) => {
                return ExecutionResult::failure(format!("argument {i}: {e}"), *tx_digest);
            }
        }
    }

    // Build executor context and native functions.
    let ctx = Rc::new(RefCell::new(Context::new(*sender, *tx_digest)));
    let natives = natives::build_natives(ctx.clone());
    let vm = Vm::new(module, natives, GasSchedule::default());

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
    match collect_object_effects(
        &ctx.borrow(),
        &call_result,
        &input_object_args,
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

/// Gas cost per byte of module bytecode when publishing.
const GAS_PER_MODULE_BYTE: u64 = 10;

fn execute_meow_module_publish(
    _sender: &Address,
    tx_digest: &Digest,
    module: &Vec<u8>,
    _inputs: &[Object],
    gas: &mut GasMeter,
) -> ExecutionResult {
    let module_size = module.len();

    if module_size > MAX_MODULE_SIZE_BYTES {
        return ExecutionResult::failure(
            format!(
                "module size {} bytes exceeds maximum of {} bytes",
                module_size, MAX_MODULE_SIZE_BYTES
            ),
            *tx_digest,
        );
    }

    let cost = module_size as u64 * GAS_PER_MODULE_BYTE;
    if let Err(e) = gas.charge(cost) {
        return ExecutionResult::failure(e.to_string(), *tx_digest);
    }

    let module_id = Address::derive(*tx_digest, 0, 0);

    // Modules are always immutable. New objects start at version 1.
    let created = vec![Object::new(
        module_id,
        ObjectOwner::Immutable,
        *tx_digest,
        ObjectVersion::ZERO
            .next()
            .expect("ZERO.next() is always Some"),
        ObjectType::Module,
        module.clone(),
    )];
    ExecutionResult::new(
        ExecutionStatus::Success,
        *tx_digest,
        created,
        vec![],
        vec![],
    )
}

// ─── Version helpers ──────────────────────────────────────────────────────────

/// Bump an object's version by 1, capping at MAX.
fn bump_version(obj: &Object) -> ObjectVersion {
    obj.version().next().unwrap_or(ObjectVersion::MAX)
}

/// Version for a newly created object (no prior history): starts at 1.
fn new_object_version() -> ObjectVersion {
    ObjectVersion::ZERO
        .next()
        .expect("ZERO.next() is always Some")
}

/// Determine the owner for an output object.
///
/// If the new version is MAX, the object becomes immutable to signal that it
/// can no longer be mutated.
fn owner_for_version(owner: Address, new_version: &ObjectVersion) -> ObjectOwner {
    if new_version == &ObjectVersion::MAX {
        ObjectOwner::Immutable
    } else {
        ObjectOwner::Address(owner)
    }
}

// ─── Gas spending ─────────────────────────────────────────────────────────────

/// Read the balance field of a gas coin object.
fn gas_coin_balance(gas_coin: &Object) -> u64 {
    let fields: Vec<(String, Value)> =
        bcs::from_bytes(gas_coin.content()).expect("gas coin content must be valid BCS");
    fields
        .iter()
        .find(|(name, _)| name == "balance")
        .and_then(|(_, val)| val.as_u64())
        .unwrap_or(0)
}

/// Deduct gas from the gas coin balance and append it to the changed objects list.
///
/// Called after every execution path — success or failure — so the gas coin is
/// always returned as a changed object.
fn apply_gas_spending(
    result: ExecutionResult,
    gas_coin: &Object,
    gas_spent: u64,
    tx_digest: &Digest,
) -> ExecutionResult {
    let fields: Vec<(String, Value)> =
        bcs::from_bytes(gas_coin.content()).expect("gas coin content must be valid BCS");

    let updated_fields: Vec<(String, Value)> = fields
        .into_iter()
        .map(|(name, val)| {
            if name == "balance" {
                let balance = val.as_u64().unwrap_or(0);
                (name, Value::U64(balance.saturating_sub(gas_spent)))
            } else {
                (name, val)
            }
        })
        .collect();

    let updated_content =
        bcs::to_bytes(&updated_fields).expect("updated gas coin fields must serialize");

    let new_version = bump_version(gas_coin);
    let gas_owner = gas_coin
        .owner()
        .address()
        .copied()
        .map(|a| owner_for_version(a, &new_version))
        .unwrap_or(ObjectOwner::Immutable);

    let updated_gas_coin = Object::new(
        *gas_coin.address(),
        gas_owner,
        *tx_digest,
        new_version,
        gas_coin.type_().clone(),
        updated_content,
    );

    let mut changed = result.changed_objects().to_vec();
    changed.push(updated_gas_coin);

    ExecutionResult::new(
        result.status().clone(),
        *result.transaction_digest(),
        result.created_objects().to_vec(),
        changed,
        result.destroyed_objects().to_vec(),
    )
}

// ─── Argument resolution ──────────────────────────────────────────────────────

/// Resolve a single call argument to a VM value.
fn resolve_arg(input: &Input, expected_type: &Type, inputs: &[Object]) -> Result<Value, String> {
    match input {
        Input::Object(object_ref) => {
            let obj = inputs
                .iter()
                .find(|o| o.address() == object_ref.address())
                .ok_or_else(|| format!("input object {} not found", object_ref.address()))?;
            Ok(object_to_vm_object_value(obj).map_err(|e| e.to_string())?)
        }
        Input::Raw(bytes) => match expected_type {
            Type::U64 => {
                let v: u64 =
                    bcs::from_bytes(bytes).map_err(|e| format!("u64 deserialization: {e}"))?;
                Ok(Value::U64(v))
            }
            Type::Bool => {
                let v: bool =
                    bcs::from_bytes(bytes).map_err(|e| format!("bool deserialization: {e}"))?;
                Ok(Value::Bool(v))
            }
            Type::Address => {
                let v: [u8; 32] =
                    bcs::from_bytes(bytes).map_err(|e| format!("address deserialization: {e}"))?;
                Ok(Value::Address(v))
            }
            Type::Str => {
                let v: String =
                    bcs::from_bytes(bytes).map_err(|e| format!("string deserialization: {e}"))?;
                Ok(Value::Str(v))
            }
            other => Err(format!(
                "Raw input cannot be resolved to type '{}'",
                other.name()
            )),
        },
    }
}

// ─── Object effects ───────────────────────────────────────────────────────────

/// Build the created/changed/destroyed object lists from execution results.
///
/// Each output object's version is bumped independently from its own input
/// version. Newly created objects (from `meow_vm_fresh_id`) start at version 1.
fn collect_object_effects(
    ctx: &Context,
    call_result: &VmCallResult,
    input_objects: &[(usize, &Object)], // (arg_index, input_object)
    module_address: &Address,
    tx_digest: &Digest,
) -> Result<(Vec<Object>, Vec<Object>, Vec<Object>), String> {
    let mut created_objects: Vec<Object> = Vec::new();
    let mut changed_objects: Vec<Object> = Vec::new();
    let mut destroyed_objects: Vec<Object> = Vec::new();

    // Build a lookup from address → input object for version resolution.
    let input_by_addr: BTreeMap<Address, &Object> = input_objects
        .iter()
        .map(|(_, obj)| (*obj.address(), *obj))
        .collect();

    // Validate that all fresh IDs were accounted for (transferred or destroyed).
    let transferred_ids = ctx
        .transfers()
        .iter()
        .filter_map(|(v, _)| v.object_id().map(Into::into))
        .collect::<Vec<Address>>();
    let destroyed_ids = ctx
        .destroyed()
        .iter()
        .filter_map(|v| v.object_id().map(Into::into))
        .collect::<Vec<Address>>();

    for fresh_id in ctx.fresh_ids() {
        let consumed = transferred_ids.contains(fresh_id) || destroyed_ids.contains(fresh_id);
        if !consumed {
            return Err(format!("created object not consumed: {fresh_id}"));
        }
    }

    // Collect transferred objects.
    for (obj_val, owner) in ctx.transfers() {
        let id: Address = obj_val
            .object_id()
            .ok_or("transferred object has no id")?
            .into();

        let (version, is_fresh) = if ctx.fresh_ids().contains(&id) {
            (new_object_version(), true)
        } else {
            let original = input_by_addr
                .get(&id)
                .ok_or_else(|| format!("transferred object {id} not found in inputs"))?;
            (bump_version(original), false)
        };

        let obj = vm_object_value_to_object(
            obj_val,
            owner_for_version(*owner, &version),
            *tx_digest,
            version,
            module_address,
        )
        .map_err(|e| e.to_string())?;

        if is_fresh {
            created_objects.push(obj);
        } else {
            changed_objects.push(obj);
        }
    }

    // Collect destroyed objects.
    for obj_val in ctx.destroyed() {
        let id: Address = obj_val
            .object_id()
            .ok_or("destroyed object has no id")?
            .into();

        let version = if ctx.fresh_ids().contains(&id) {
            new_object_version()
        } else {
            let original = input_by_addr
                .get(&id)
                .ok_or_else(|| format!("destroyed object {id} not found in inputs"))?;
            bump_version(original)
        };

        destroyed_objects.push(
            vm_object_value_to_object(
                obj_val,
                ObjectOwner::Immutable,
                *tx_digest,
                version,
                module_address,
            )
            .map_err(|e| e.to_string())?,
        );
    }

    // Collect surviving input objects (those not consumed by transfer/destroy).
    for (arg_idx, input_obj) in input_objects {
        let final_val = match call_result.final_args.get(*arg_idx) {
            Some(Some(v)) => v,
            _ => continue, // consumed (moved to transfer/destroy)
        };

        let id: Address = final_val
            .object_id()
            .ok_or("surviving object has no id")?
            .into();

        // Skip if already handled as a transferred input.
        if transferred_ids.contains(&id) || destroyed_ids.contains(&id) {
            continue;
        }

        let version = bump_version(input_obj);
        let original_owner = *input_obj.owner().address().unwrap();
        changed_objects.push(
            vm_object_value_to_object(
                final_val,
                owner_for_version(original_owner, &version),
                *tx_digest,
                version,
                module_address,
            )
            .map_err(|e| e.to_string())?,
        );
    }

    Ok((created_objects, changed_objects, destroyed_objects))
}
