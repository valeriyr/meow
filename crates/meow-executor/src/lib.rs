mod context;
mod natives;

use std::{cell::RefCell, rc::Rc};

use meow_types::{
    address::Address,
    digest::Digest,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    transaction::{
        Transaction,
        call::Input,
        execution_result::{ExecutionResult, ExecutionStatus},
    },
};
use meow_vm::{
    module::Module,
    types::{Type, Value},
    vm::{GasMeter, Vm, VmCallResult},
};

use crate::context::Context;

/// Execute a transaction against a set of input objects.
///
/// The `inputs` slice must contain the module object (identified by
/// `ObjectType::Module`) and any object arguments the transaction references.
///
/// Returns an [`ExecutionResult`] with the execution status and object effects.
pub fn execute(transaction: &Transaction, inputs: Vec<Object>) -> ExecutionResult {
    let tx_digest = transaction.digest();
    let sender = transaction.sender();
    let module_address = transaction.call().module();

    // Find the module object in inputs.
    let module_obj = match inputs
        .iter()
        .find(|o| matches!(o.type_(), ObjectType::Module))
    {
        Some(o) => o,
        None => {
            return ExecutionResult::failure("module object not found in inputs", tx_digest);
        }
    };

    // Deserialize the module.
    let module: Module = match bcs::from_bytes(module_obj.content()) {
        Ok(m) => m,
        Err(e) => {
            return ExecutionResult::failure(
                format!("failed to deserialize module: {e}"),
                tx_digest,
            );
        }
    };

    // Find the function in the module.
    let fn_name = transaction.call().function().as_ref();
    let func = match module.get_function(fn_name) {
        Some(f) => f,
        None => {
            return ExecutionResult::failure(
                format!("function '{fn_name}' not found in module"),
                tx_digest,
            );
        }
    };

    // Resolve call arguments to VM values.
    let call_args_inputs = transaction.call().arguments();
    if call_args_inputs.len() != func.params.len() {
        return ExecutionResult::failure(
            format!(
                "argument count mismatch: expected {}, got {}",
                func.params.len(),
                call_args_inputs.len()
            ),
            tx_digest,
        );
    }

    let mut vm_args: Vec<Value> = Vec::with_capacity(call_args_inputs.len());
    // Track which arg indices are object inputs (for final_args analysis).
    let mut input_object_args: Vec<(usize, &Object)> = Vec::new();

    for (i, (input, (_param_name, param_type))) in
        call_args_inputs.iter().zip(func.params.iter()).enumerate()
    {
        match resolve_arg(input, param_type, &inputs) {
            Ok(v) => {
                if matches!(input, Input::Object(_)) {
                    if let Input::Object(addr) = input {
                        if let Some(obj) = inputs.iter().find(|o| o.address() == addr) {
                            input_object_args.push((i, obj));
                        }
                    }
                }
                vm_args.push(v);
            }
            Err(e) => {
                return ExecutionResult::failure(format!("argument {i}: {e}"), tx_digest);
            }
        }
    }

    // Build executor context and native functions.
    let ctx = Rc::new(RefCell::new(Context::new(*sender, tx_digest)));
    let natives = natives::build_natives(ctx.clone());
    let vm = Vm::new(module, natives);
    let mut gas = GasMeter::new(1_000_000);

    // Execute the function.
    let call_result = match vm.call(fn_name, vm_args, &mut gas) {
        Ok(r) => r,
        Err(meow_vm::error::VmError::Aborted { message, .. }) => {
            return ExecutionResult::failure(message, tx_digest);
        }
        Err(e) => {
            return ExecutionResult::failure(e.to_string(), tx_digest);
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
            tx_digest,
            created,
            changed,
            destroyed,
        ),
        Err(e) => ExecutionResult::new(
            ExecutionStatus::Failure(e),
            tx_digest,
            vec![],
            vec![],
            vec![],
        ),
    }
}

/// Convert a meow-types Object to a meow-vm Value::Object.
///
/// The object content is BCS-serialized `Vec<(String, Value)>`.
fn object_to_vm_value(obj: &Object) -> Value {
    let type_name = match obj.type_() {
        ObjectType::Object(decl) => decl.name().as_ref().to_string(),
        ObjectType::Module => "Module".to_string(),
    };
    let fields: Vec<(String, Value)> =
        bcs::from_bytes(obj.content()).expect("object content must be valid BCS");
    Value::Object { type_name, fields }
}

/// Convert a meow-vm Value::Object back to a meow-types Object.
fn vm_value_to_object(
    val: &Value,
    owner: Address,
    tx_digest: Digest,
    module_addr: &Address,
) -> Object {
    let (type_name, fields) = match val {
        Value::Object { type_name, fields } => (type_name.clone(), fields.clone()),
        _ => panic!("vm_value_to_object: expected Object value"),
    };

    let id = val.object_id().expect("Object must have id field").into();
    let content = bcs::to_bytes(&fields).expect("fields must be serializable");

    let ident =
        meow_types::object::identifier::Identifier::new(type_name.clone()).unwrap_or_else(|_| {
            // Fall back to a sanitized name if the type name isn't a valid Identifier.
            meow_types::object::identifier::Identifier::new("Object").unwrap()
        });
    let decl_ref = ObjectDeclRef::new(*module_addr, ident);

    Object::new(
        id,
        owner,
        tx_digest,
        ObjectVersion::ZERO,
        ObjectType::Object(decl_ref),
        content,
    )
}

// ─── Argument resolution ──────────────────────────────────────────────────────

/// Resolve a single call argument to a VM value.
fn resolve_arg(input: &Input, expected_type: &Type, inputs: &[Object]) -> Result<Value, String> {
    match input {
        Input::Object(addr) => {
            let obj = inputs
                .iter()
                .find(|o| o.address() == addr)
                .ok_or_else(|| format!("input object {} not found", addr))?;
            Ok(object_to_vm_value(obj))
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
            other => Err(format!(
                "Raw input cannot be resolved to type '{}'",
                other.name()
            )),
        },
    }
}

// ─── Object effect collection ─────────────────────────────────────────────────

/// Build the created/changed/destroyed object lists from execution results.
fn collect_object_effects(
    ctx: &Context,
    call_result: &VmCallResult,
    input_objects: &[(usize, &Object)], // (arg_index, input_object)
    module_address: &Address,
    tx_digest: Digest,
) -> Result<(Vec<Object>, Vec<Object>, Vec<Object>), String> {
    let mut created_objects: Vec<Object> = Vec::new();
    let mut changed_objects: Vec<Object> = Vec::new();
    let mut destroyed_objects: Vec<Object> = Vec::new();

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
        let id = obj_val
            .object_id()
            .ok_or("transferred object has no id")?
            .into();

        if ctx.fresh_ids().contains(&id) {
            // Newly created object.
            created_objects.push(vm_value_to_object(
                obj_val,
                *owner,
                tx_digest,
                module_address,
            ));
        } else {
            // Input object that was transferred — report as changed with new owner.
            changed_objects.push(vm_value_to_object(
                obj_val,
                *owner,
                tx_digest,
                module_address,
            ));
        }
    }

    // Collect destroyed objects.
    for obj_val in ctx.destroyed() {
        // Owner is not meaningful for destroyed objects; use zero.
        destroyed_objects.push(vm_value_to_object(
            obj_val,
            Address::ZERO,
            tx_digest,
            module_address,
        ));
    }

    // Collect surviving input objects (those not consumed by transfer/destroy).
    for (arg_idx, input_obj) in input_objects {
        let final_val = match call_result.final_args.get(*arg_idx) {
            Some(Some(v)) => v,
            _ => continue, // consumed (moved to transfer/destroy)
        };

        let id = final_val
            .object_id()
            .ok_or("surviving object has no id")?
            .into();

        // Skip if already handled as a transferred input.
        if transferred_ids.contains(&id) || destroyed_ids.contains(&id) {
            continue;
        }

        // Use the original owner.
        changed_objects.push(vm_value_to_object(
            final_val,
            *input_obj.owner(),
            tx_digest,
            module_address,
        ));
    }

    Ok((created_objects, changed_objects, destroyed_objects))
}
