use std::{cell::RefCell, rc::Rc};

use meow_vm::{NativeFnEntry, NativeResult};
use meow_vm_bytecode_verifier::natives::{NativeParam, NativeSignature};
use meow_vm_types::types::Type;

use crate::{Value, context::Context};

/// Returns the verifier signatures for the adapter-supplied native functions.
///
/// These are passed to [`meow_vm_bytecode_verifier::BytecodeVerifier::new`] so
/// the verifier can type-check call sites for adapter natives.
pub fn adapter_native_signatures() -> Vec<NativeSignature> {
    vec![
        NativeSignature::new("meow_vm_fresh_id", vec![], Some(Type::Address)),
        NativeSignature::new(
            "meow_vm_transfer",
            vec![NativeParam::AnyObject, NativeParam::Concrete(Type::Address)],
            None,
        ),
        NativeSignature::new("meow_vm_destroy", vec![NativeParam::AnyObject], None),
        NativeSignature::new("meow_vm_sender", vec![], Some(Type::Address)),
        NativeSignature::new("meow_vm_rand", vec![], Some(Type::U64)),
        NativeSignature::new("meow_vm_timestamp", vec![], Some(Type::U64)),
    ]
}

/// Constructs the native function table used by the executor.
///
/// Binds each built-in native to a closure that captures the shared `Context`,
/// and returns the resulting `Vec<NativeFnEntry>` for registration with the VM.
pub fn build_natives(ctx: Rc<RefCell<Context>>) -> Vec<NativeFnEntry> {
    let fresh_id = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_fresh_id".to_string(),
            param_count: 0,
            gas_cost: 10, // 10 gas — ID derivation involves a hash computation
            func: Box::new(move |_| {
                NativeResult::Return(Some(Value::Address(c.borrow_mut().next_fresh_id().into())))
            }),
        }
    };

    let transfer = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_transfer".to_string(),
            param_count: 2,
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
                if !obj_val.is_object() {
                    return NativeResult::Error(format!(
                        "meow_vm_transfer: expected Object, got {}",
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
            param_count: 1,
            gas_cost: 10, // 10 gas — object deletion is cheaper than transfer (no owner update)
            func: Box::new(move |args| {
                let obj_val = args.into_iter().next().unwrap();
                if !obj_val.is_object() {
                    return NativeResult::Error(format!(
                        "meow_vm_destroy: expected Object, got {}",
                        obj_val.type_name()
                    ));
                }
                c.borrow_mut().destroy(obj_val);
                NativeResult::Return(None)
            }),
        }
    };

    let sender = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_sender".to_string(),
            param_count: 0,
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
            param_count: 0,
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
            param_count: 0,
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
    fn adapter_signatures_match_build_natives() {
        let ctx = Rc::new(RefCell::new(Context::new(
            Address::ZERO,
            Digest::ZERO,
            DEFAULT_RAND_SEED,
            0,
        )));
        let sigs = adapter_native_signatures();
        let entries = build_natives(ctx);

        let sig_names: Vec<&str> = sigs.iter().map(|s| s.name.as_str()).collect();
        let entry_names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            sig_names, entry_names,
            "adapter_native_signatures and build_natives must list functions in the same order"
        );

        for (sig, entry) in sigs.iter().zip(entries.iter()) {
            assert_eq!(
                sig.params.len(),
                entry.param_count,
                "param count mismatch for '{}': verifier expects {}, VM expects {}",
                sig.name,
                sig.params.len(),
                entry.param_count,
            );
        }
    }
}
