//! Native functions registered before each VM call to give contracts access to chain operations.
//!
//! Two representations are provided: full runtime entries with closures for execution, and
//! lightweight signatures for type-checking at compile and verify time.

use std::{cell::RefCell, rc::Rc};

use meow_types::{
    config,
    system_framework::meow_object::{self, MEOW_OBJECT_ID_BYTECODE_TYPE_NAME, MeowObjectId},
};
use meow_vm_types::{
    convert,
    natives::{NativeFnEntry, NativeParam, NativeResult, NativeSig},
    types::Type,
};

use crate::{Value, context::Context};

/// Returns the native signatures for all adapter-supplied native functions.
///
/// Used by the bytecode verifier ([`meow_vm_bytecode_verifier::verify`]) — uses the
/// bytecode-qualified `Id` type name.
pub fn adapter_native_sigs() -> Vec<NativeSig> {
    native_sigs_with_id_type(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME)
}

/// Returns the native signatures for use by the compiler's type checker.
///
/// Uses the source-level `meow_object::Id` type name so the type checker can match
/// it against source annotations.
pub fn adapter_native_sigs_for_compiler() -> Vec<NativeSig> {
    native_sigs_with_id_type("meow_object::Id")
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
            name: config::NATIVE_FN_FRESH_ID.to_string(),
            params: vec![],
            return_type: Some(id_type.clone()),
            gas_cost: 10, // 10 gas — ID derivation involves a hash computation
            func: Box::new(move |_| {
                let address = c.borrow_mut().next_fresh_id();
                let id = MeowObjectId::from(address);
                NativeResult::Return(Some(id.into()))
            }),
        }
    };

    let transfer = {
        let c = ctx.clone();
        NativeFnEntry {
            name: config::NATIVE_FN_TRANSFER.to_string(),
            params: vec![
                NativeParam::LocalStruct,
                NativeParam::Concrete(Type::Address),
            ],
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
                if meow_object::object_address(&obj_val).is_none() {
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
            name: config::NATIVE_FN_DESTROY.to_string(),
            params: vec![NativeParam::Concrete(id_type.clone())],
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
            name: config::NATIVE_FN_SENDER.to_string(),
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
            name: config::NATIVE_FN_RAND.to_string(),
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
            name: config::NATIVE_FN_TIMESTAMP.to_string(),
            params: vec![],
            return_type: Some(Type::U64),
            gas_cost: 1, // 1 gas — cheap lookup of a pre-loaded context field
            func: Box::new(move |_| NativeResult::Return(Some(Value::U64(c.borrow().timestamp())))),
        }
    };

    vec![fresh_id, transfer, destroy, sender, rand, timestamp]
}

/// Builds the full native signature list using `id_type_name` as the struct type name for
/// `meow_object::Id` parameters and return types. The verifier passes the bytecode-qualified
/// name; the compiler passes the source-level name.
fn native_sigs_with_id_type(id_type_name: &str) -> Vec<NativeSig> {
    let id_type = Type::Struct(id_type_name.to_string());
    vec![
        NativeSig {
            name: config::NATIVE_FN_FRESH_ID.to_string(),
            params: vec![],
            return_type: Some(id_type.clone()),
        },
        NativeSig {
            name: config::NATIVE_FN_TRANSFER.to_string(),
            params: vec![
                NativeParam::LocalStruct,
                NativeParam::Concrete(Type::Address),
            ],
            return_type: None,
        },
        NativeSig {
            name: config::NATIVE_FN_DESTROY.to_string(),
            params: vec![NativeParam::Concrete(id_type)],
            return_type: None,
        },
        NativeSig {
            name: config::NATIVE_FN_SENDER.to_string(),
            params: vec![],
            return_type: Some(Type::Address),
        },
        NativeSig {
            name: config::NATIVE_FN_RAND.to_string(),
            params: vec![],
            return_type: Some(Type::U64),
        },
        NativeSig {
            name: config::NATIVE_FN_TIMESTAMP.to_string(),
            params: vec![],
            return_type: Some(Type::U64),
        },
    ]
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use meow_types::{address::Address, digest::Digest};

    use super::*;
    use crate::{context::Context, external_context::DEFAULT_RAND_SEED};

    #[test]
    fn compiler_and_verifier_sigs_use_correct_id_type_names() {
        let verifier_sigs = adapter_native_sigs();
        let compiler_sigs = adapter_native_sigs_for_compiler();

        let verifier_id = verifier_sigs
            .iter()
            .find(|s| s.name == config::NATIVE_FN_FRESH_ID)
            .and_then(|s| s.return_type.as_ref())
            .expect("meow_vm_fresh_id must have a return type in verifier sigs");
        assert_eq!(
            *verifier_id,
            Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string()),
            "verifier sigs must use the bytecode-qualified Id type name"
        );

        let compiler_id = compiler_sigs
            .iter()
            .find(|s| s.name == config::NATIVE_FN_FRESH_ID)
            .and_then(|s| s.return_type.as_ref())
            .expect("meow_vm_fresh_id must have a return type in compiler sigs");
        assert_eq!(
            *compiler_id,
            Type::Struct("meow_object::Id".to_string()),
            "compiler sigs must use the source-level meow_object::Id type name"
        );
    }

    #[test]
    fn all_native_collections_are_consistent() {
        let ctx = Rc::new(RefCell::new(Context::new(
            Address::ZERO,
            Digest::ZERO,
            DEFAULT_RAND_SEED,
            0,
        )));
        let entries = build_natives(ctx);

        let assert_consistent = |sigs: Vec<NativeSig>, label: &str| {
            let sig_names: Vec<&str> = sigs.iter().map(|s| s.name.as_str()).collect();
            let entry_names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
            assert_eq!(
                sig_names, entry_names,
                "{label} and build_natives must list functions in the same order"
            );
            for (sig, entry) in sigs.iter().zip(entries.iter()) {
                assert_eq!(
                    sig.params.len(),
                    entry.params.len(),
                    "{label}: sig/VM param count mismatch for '{}'",
                    entry.name
                );
                assert_eq!(
                    sig.return_type.is_some(),
                    entry.return_type.is_some(),
                    "{label}: sig/VM return type presence mismatch for '{}'",
                    entry.name
                );
            }
        };

        assert_consistent(adapter_native_sigs(), "adapter_native_sigs");
        assert_consistent(
            adapter_native_sigs_for_compiler(),
            "adapter_native_sigs_for_compiler",
        );
    }
}
