use std::{cell::RefCell, rc::Rc};

use meow_types::system_framework::meow_object::{
    self, MEOW_OBJECT_ID_BYTECODE_TYPE_NAME, MeowObjectId,
};
use meow_vm_bytecode_verifier::natives::{NativeParam, NativeSignature};
use meow_vm_types::{
    convert,
    natives::{NativeFnEntry, NativeResult, NativeSig},
    types::Type,
};

use crate::{Value, context::Context};

/// Returns the verifier signatures for the adapter-supplied native functions.
///
/// These are passed to [`meow_vm_bytecode_verifier::verify`] so the verifier
/// can type-check call sites for adapter natives.
pub fn adapter_native_signatures() -> Vec<NativeSignature> {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());
    vec![
        NativeSignature::new("meow_vm_fresh_id", vec![], Some(id_type.clone())),
        NativeSignature::new(
            "meow_vm_transfer",
            vec![NativeParam::AnyStruct, NativeParam::Concrete(Type::Address)],
            None,
        ),
        NativeSignature::new(
            "meow_vm_destroy",
            vec![NativeParam::Concrete(id_type)],
            None,
        ),
        NativeSignature::new("meow_vm_sender", vec![], Some(Type::Address)),
        NativeSignature::new("meow_vm_rand", vec![], Some(Type::U64)),
        NativeSignature::new("meow_vm_timestamp", vec![], Some(Type::U64)),
    ]
}

/// Returns the compiler-level native signatures for adapter-supplied native functions.
///
/// These are passed to [`meow_vm_compiler::Compiler::compile`] so the type checker
/// can validate call sites for adapter natives in source code.
pub fn adapter_native_sigs_for_compiler() -> Vec<NativeSig> {
    let id_type = Type::Struct("meow_object::Id".to_string());
    vec![
        NativeSig {
            name: "meow_vm_fresh_id".to_string(),
            params: vec![],
            return_type: Some(id_type.clone()),
        },
        NativeSig {
            name: "meow_vm_transfer".to_string(),
            params: vec![None, Some(Type::Address)],
            return_type: None,
        },
        NativeSig {
            name: "meow_vm_destroy".to_string(),
            params: vec![Some(id_type)],
            return_type: None,
        },
        NativeSig {
            name: "meow_vm_sender".to_string(),
            params: vec![],
            return_type: Some(Type::Address),
        },
        NativeSig {
            name: "meow_vm_rand".to_string(),
            params: vec![],
            return_type: Some(Type::U64),
        },
        NativeSig {
            name: "meow_vm_timestamp".to_string(),
            params: vec![],
            return_type: Some(Type::U64),
        },
    ]
}

/// Constructs the native function table used by the executor.
///
/// Binds each built-in native to a closure that captures the shared `Context`,
/// and returns the resulting `Vec<NativeFnEntry>` for registration with the VM.
pub fn build_natives(ctx: Rc<RefCell<Context>>) -> Vec<NativeFnEntry> {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());

    let fresh_id = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_fresh_id".to_string(),
            params: vec![],
            return_type: Some(id_type.clone()),
            gas_cost: 10, // 10 gas — ID derivation involves a hash computation
            func: Box::new(move |_| {
                let address = c.borrow_mut().next_fresh_id();
                let id = MeowObjectId::from(address);
                NativeResult::Return(Some(id.to_qualified_vm_value()))
            }),
        }
    };

    let transfer = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_transfer".to_string(),
            params: vec![None, Some(Type::Address)],
            return_type: None,
            gas_cost: 20, // 20 gas — object ownership change writes to the execution context
            func: Box::new(move |mut args| {
                // Args arrive in call order: (obj, owner). We pop from reversed stack order.
                let owner_val = args.pop().unwrap();
                let obj_val = args.pop().unwrap();

                let owner = match owner_val {
                    Value::Address(a) => a,
                    other => {
                        return NativeResult::Error(format!(
                            "meow_vm_transfer: expected address for owner, got {}",
                            other.type_name()
                        ));
                    }
                };
                if meow_object::object_id(&obj_val).is_none() {
                    return NativeResult::Error(format!(
                        "meow_vm_transfer: expected struct with id: meow_object::Id as first field, got {}",
                        obj_val.type_name()
                    ));
                }
                c.borrow_mut().transfer(obj_val, owner.into());
                NativeResult::Return(None)
            }),
        }
    };

    let destroy = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_destroy".to_string(),
            params: vec![Some(id_type.clone())],
            return_type: None,
            gas_cost: 10, // 10 gas — object deletion is cheaper than transfer (no owner update)
            func: Box::new(move |args| {
                let id_val = args.into_iter().next().unwrap();
                let id = convert::value_to_rust::<MeowObjectId>(&id_val);

                match id {
                    Ok(id) => {
                        c.borrow_mut().destroy(*id.inner());
                        NativeResult::Return(None)
                    }
                    Err(_) => NativeResult::Error(format!(
                        "meow_vm_destroy: expected meow_object::Id struct with inner: address, got {}",
                        id_val.type_name()
                    )),
                }
            }),
        }
    };

    let sender = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_sender".to_string(),
            params: vec![],
            return_type: Some(Type::Address),
            gas_cost: 1, // 1 gas — cheap lookup of a pre-loaded context field
            func: Box::new(move |_| {
                NativeResult::Return(Some(Value::Address(c.borrow().sender().into())))
            }),
        }
    };

    let rand = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_rand".to_string(),
            params: vec![],
            return_type: Some(Type::U64),
            gas_cost: 10, // 10 gas — random generation involves a hash computation
            func: Box::new(move |_| {
                NativeResult::Return(Some(Value::U64(c.borrow_mut().next_rand())))
            }),
        }
    };

    let timestamp = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_timestamp".to_string(),
            params: vec![],
            return_type: Some(Type::U64),
            gas_cost: 1, // 1 gas — cheap lookup of a pre-loaded context field
            func: Box::new(move |_| NativeResult::Return(Some(Value::U64(c.borrow().timestamp())))),
        }
    };

    vec![fresh_id, transfer, destroy, sender, rand, timestamp]
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use meow_types::{address::Address, digest::Digest};

    use super::*;
    use crate::{context::Context, external_context::DEFAULT_RAND_SEED};

    #[test]
    fn all_native_collections_are_consistent() {
        let ctx = Rc::new(RefCell::new(Context::new(
            Address::ZERO,
            Digest::ZERO,
            DEFAULT_RAND_SEED,
            0,
        )));
        let verifier_sigs = adapter_native_signatures();
        let compiler_sigs = adapter_native_sigs_for_compiler();
        let entries = build_natives(ctx);

        let verifier_names: Vec<&str> = verifier_sigs.iter().map(|s| s.name.as_str()).collect();
        let compiler_names: Vec<&str> = compiler_sigs.iter().map(|s| s.name.as_str()).collect();
        let entry_names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            verifier_names, entry_names,
            "verifier sigs and build_natives must list functions in the same order"
        );
        assert_eq!(
            compiler_names, entry_names,
            "compiler sigs and build_natives must list functions in the same order"
        );

        for ((vsig, csig), entry) in verifier_sigs
            .iter()
            .zip(compiler_sigs.iter())
            .zip(entries.iter())
        {
            assert_eq!(
                vsig.params.len(),
                entry.params.len(),
                "verifier/VM param count mismatch for '{}'",
                entry.name
            );
            assert_eq!(
                csig.params.len(),
                entry.params.len(),
                "compiler/VM param count mismatch for '{}'",
                entry.name
            );
            assert_eq!(
                vsig.return_type.is_some(),
                entry.return_type.is_some(),
                "verifier/VM return type presence mismatch for '{}'",
                entry.name
            );
            assert_eq!(
                csig.return_type.is_some(),
                entry.return_type.is_some(),
                "compiler/VM return type presence mismatch for '{}'",
                entry.name
            );
        }
    }
}
