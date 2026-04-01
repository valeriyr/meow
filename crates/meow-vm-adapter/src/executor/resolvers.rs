use meow_types::{
    address::Address,
    object::{Object, object_conversion, object_type::ObjectType, object_version::ObjectVersion},
    system_framework::meow_coin,
    transaction::call::{Call, Input},
};
use meow_vm_types::{
    module::{Function, Module},
    types::{Type, Value},
};

use crate::executor::{Result, error::ExecutorError};

/// `(vm_values, object_args)` where `object_args` is `(arg_index, input_object)` pairs.
type ResolvedArgs<'a> = (Vec<Value>, Vec<(usize, &'a Object)>);

/// Resolve call arguments to VM values, tracking which ones are object inputs for later analysis.
pub fn resolve_args<'a>(
    call: &Call,
    func: &Function,
    inputs: &'a [Object],
) -> std::result::Result<ResolvedArgs<'a>, String> {
    // Resolve call arguments to VM values.
    let call_args_inputs = call.arguments();
    if call_args_inputs.len() != func.params.len() {
        return Err(format!(
            "argument count mismatch: expected {}, got {}",
            func.params.len(),
            call_args_inputs.len()
        ));
    }

    let mut vm_args: Vec<Value> = Vec::with_capacity(call_args_inputs.len());
    // Track which arg indices are object inputs (for final_args analysis).
    let mut object_args: Vec<(usize, &'a Object)> = Vec::new();

    for (i, (input, (_param_name, param_type))) in
        call_args_inputs.iter().zip(func.params.iter()).enumerate()
    {
        match resolve_arg(input, param_type, inputs) {
            Ok(v) => {
                if let Input::Object(object_ref) = input
                    && let Some(obj) = inputs.iter().find(|o| o.address() == object_ref.address())
                {
                    object_args.push((i, obj));
                }
                vm_args.push(v);
            }
            Err(e) => {
                return Err(format!("argument {i}: {e}"));
            }
        }
    }

    Ok((vm_args, object_args))
}

/// Resolve a single call argument to a VM value.
pub fn resolve_arg(
    input: &Input,
    expected_type: &Type,
    inputs: &[Object],
) -> std::result::Result<Value, String> {
    match input {
        Input::Object(object_ref) => {
            let obj = inputs
                .iter()
                .find(|o| o.address() == object_ref.address())
                .ok_or_else(|| format!("input object {} not found", object_ref.address()))?;
            Ok(object_conversion::object_to_vm_object_value(obj).map_err(|e| e.to_string())?)
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

/// Resolve the gas coin object from inputs and validate it.
pub fn resolve_gas_coin_object<'a>(
    gas_coin_address: &Address,
    sender: &Address,
    inputs: &'a [Object],
) -> Result<&'a Object> {
    // Locate and validate the gas coin.
    let gas_coin = match inputs.iter().find(|o| o.address() == gas_coin_address) {
        Some(o) => Ok(o),
        None => Err(ExecutorError::GasCoinNotFound),
    }?;
    if !meow_coin::is_meow_coin_object(gas_coin) {
        return Err(ExecutorError::InvalidGasCoin);
    }
    if gas_coin.owner().address() != Some(sender) {
        return Err(ExecutorError::InvalidGasCoinOwner);
    }
    if gas_coin.version() == &ObjectVersion::MAX {
        return Err(ExecutorError::ObjectAtMaxVersion(*gas_coin_address));
    }

    Ok(gas_coin)
}

/// Resolve the module from inputs.
pub fn resolve_module(inputs: &[Object]) -> std::result::Result<Module, String> {
    let modules = inputs
        .iter()
        .filter(|o| matches!(o.type_(), ObjectType::Module))
        .collect::<Vec<_>>();

    if modules.len() != 1 {
        return Err(format!(
            "expected exactly 1 module object in inputs, found {}",
            modules.len()
        ));
    }

    match bcs::from_bytes(modules[0].content()) {
        Ok(m) => Ok(m),
        Err(e) => Err(format!("failed to deserialize module: {e}")),
    }
}
