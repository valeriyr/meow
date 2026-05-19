//! Input resolution — validates and extracts the gas coin, call arguments, and
//! dependency modules from the raw transaction inputs before execution begins.

use std::collections::HashMap;

use meow_types::{
    address::Address,
    object::{
        Object, object_conversion, object_ref::ObjectRef, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::meow_coin,
    transaction::{call::Call, input::Input},
};
use meow_vm_types::{
    address::Address as VmAddress,
    module::{Function, Module},
    types::{Type, Value},
};

use crate::executor::{Result, error::ExecutorError};

/// `(vm_values, object_args)` where `object_args` is the list of input objects.
type ResolvedArgs<'a> = (Vec<Value>, Vec<&'a Object>);

/// Resolve call arguments to VM values, tracking which inputs are object references.
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
    // Track which inputs are object references.
    let mut object_args: Vec<&'a Object> = Vec::new();

    for (i, (input, (_param_name, param_type))) in
        call_args_inputs.iter().zip(func.params.iter()).enumerate()
    {
        match resolve_arg(input, param_type, inputs) {
            Ok(v) => {
                if let Input::Object(object_ref) = input
                    && let Some(obj) = inputs.iter().find(|o| o.address() == object_ref.address())
                {
                    object_args.push(obj);
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
            let obj_address = obj.address();
            if obj.type_() == &ObjectType::Module {
                return Err(format!(
                    "object {} is a module and cannot be used as a call argument",
                    obj_address
                ));
            }
            let obj_version = obj.version();
            if obj_version == &ObjectVersion::MAX {
                return Err(format!(
                    "object {} is at the maximum version and cannot be used as a call argument",
                    obj_address
                ));
            }
            let object_ref_version = object_ref.version();
            if obj_version != object_ref_version {
                return Err(format!(
                    "object {} has invalid version: expected {}, found {}",
                    obj_address, object_ref_version, obj_version
                ));
            }
            let obj_digest = obj.digest();
            let object_ref_digest = object_ref.digest();
            if &obj_digest != object_ref_digest {
                return Err(format!(
                    "object {} has invalid digest: expected {}, found {}",
                    obj_address, object_ref_digest, obj_digest
                ));
            }
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
                Ok(Value::Address(v.into()))
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
    gas_coin_ref: &ObjectRef,
    sender: &Address,
    inputs: &'a [Object],
) -> Result<&'a Object> {
    let gas_coin_address = gas_coin_ref.address();
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
    let gas_coin_version = gas_coin.version();
    if gas_coin_version == &ObjectVersion::MAX {
        return Err(ExecutorError::ObjectAtMaxVersion(*gas_coin_address));
    }
    let gas_coin_ref_version = gas_coin_ref.version();
    if gas_coin_version != gas_coin_ref_version {
        return Err(ExecutorError::InvalidObjectVersion {
            address: *gas_coin_address,
            expected: *gas_coin_ref_version,
            found: *gas_coin_version,
        });
    }
    let gas_coin_digest = gas_coin.digest();
    if &gas_coin_digest != gas_coin_ref.digest() {
        return Err(ExecutorError::InvalidObjectDigest {
            address: *gas_coin_address,
            expected: *gas_coin_ref.digest(),
            found: gas_coin_digest,
        });
    }

    Ok(gas_coin)
}

/// Resolve the main module (identified by `module_address`) from inputs.
///
/// There must be exactly one `ObjectType::Module` object whose on-chain address
/// matches `module_address`. Additional module objects in inputs are treated as
/// dependency modules and resolved separately via [`resolve_dep_modules`].
pub fn resolve_module(
    inputs: &[Object],
    module_address: &Address,
) -> std::result::Result<Module, String> {
    let module_obj = inputs
        .iter()
        .find(|o| o.type_() == &ObjectType::Module && o.address() == module_address)
        .ok_or_else(|| format!("module object at address {module_address} not found in inputs"))?;

    bcs::from_bytes(module_obj.content())
        .map_err(|e| format!("failed to deserialize module at {module_address}: {e}"))
}

/// Resolve the full transitive dependency tree from `inputs`.
///
/// Starting from the main module's `imports`, each deserialized dep module's own
/// `imports` are added to the queue, so the result contains every module that
/// could be reached during execution — not just direct dependencies.
///
/// Diamond dependencies (the same address reachable through two different paths)
/// are allowed and silently deduplicated.
///
/// Every address in the transitive closure must have a corresponding
/// `ObjectType::Module` object in `inputs`; returns an error naming the first
/// missing module.
pub fn resolve_dep_modules(
    inputs: &[Object],
    imports: &[VmAddress],
) -> std::result::Result<HashMap<VmAddress, Module>, String> {
    // Index module objects by address for O(1) lookup during BFS.
    let module_index: HashMap<Address, &Object> = inputs
        .iter()
        .filter(|o| o.type_() == &ObjectType::Module)
        .map(|o| (*o.address(), o))
        .collect();

    let mut deps: HashMap<VmAddress, Module> = HashMap::new();
    let mut queue: Vec<VmAddress> = imports.to_vec();

    while let Some(vm_addr) = queue.pop() {
        if deps.contains_key(&vm_addr) {
            continue; // already resolved — deduplicate diamond dependencies
        }

        let addr = Address::from(vm_addr);
        let dep_obj = module_index
            .get(&addr)
            .ok_or_else(|| format!("missing dependency module at address {addr}"))?;
        let dep: Module = bcs::from_bytes(dep_obj.content())
            .map_err(|e| format!("failed to deserialize dependency module at {addr}: {e}"))?;

        // Enqueue this dep's own imports for transitive resolution.
        for &transitive in &dep.imports {
            if !deps.contains_key(&transitive) {
                queue.push(transitive);
            }
        }

        deps.insert(vm_addr, dep);
    }

    Ok(deps)
}
