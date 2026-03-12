use std::{cell::RefCell, rc::Rc};

use crate::context::Context;

use meow_vm::{
    types::Value,
    vm::{NativeFnEntry, NativeResult},
};

/// Constructs the native function table used by the executor.
///
/// Binds each built-in native (`meow_vm_fresh_id`, `meow_vm_transfer`,
/// `meow_vm_destroy`, `meow_vm_sender`) to a closure that captures the shared
/// `Context`, and returns the resulting `Vec<NativeFnEntry>` for registration
/// with the VM.
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

    vec![fresh_id, transfer, destroy, sender]
}
