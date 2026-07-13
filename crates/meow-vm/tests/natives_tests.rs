mod utils;

use std::collections::HashMap;

use meow_vm::error::VmError;
use meow_vm::gas_meter::GasMeter;
use meow_vm_types::address::Address;
use meow_vm_types::bytecode::Instruction;
use meow_vm_types::module::{Function, Module};
use meow_vm_types::module_ref;
use meow_vm_types::natives::{NativeFnEntry, NativeParam, NativeResult};
use meow_vm_types::types::{FieldDef, StructDef, Type, Value};

//
// ─── Native function calls ───
//

#[test]
fn native_returns_value() {
    let src = r#"
        mod test;

        pub fn compute(a: u64, b: u64) -> u64 { let sum = add_native(a, b); sum }
    "#;
    let vm = utils::vm_with_natives(src, vec![test_add_native()]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("compute", vec![Value::U64(3), Value::U64(4)], &mut gas)
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(7)));
}

#[test]
fn void_native_does_not_leave_stack_garbage() {
    // The compiler emits Pop after expression-statement calls. A void native
    // must push Void (not nothing) or the Pop would underflow the stack.
    let src = r#"
        mod test;

        pub fn run_side_effect(x: u64) -> u64 { log_native(x); x + 1 }
    "#;
    let vm = utils::vm_with_natives(src, vec![test_log_native()]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("run_side_effect", vec![Value::U64(10)], &mut gas)
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(11)));
}

//
// ─── NativeResult::Error ───
//

#[test]
fn native_error_propagates_as_vm_native_error() {
    let src = r#"
        mod test;

        pub fn run() { fail_native(); }
    "#;
    let native = NativeFnEntry {
        name: "fail_native".to_string(),
        params: vec![],
        return_type: None,
        gas_cost: 0,
        func: Box::new(|_| NativeResult::Error("something broke".to_string())),
    };
    let vm = utils::vm_with_natives(src, vec![native]);
    let mut gas = GasMeter::unlimited();
    let err = vm.call("run", vec![], &mut gas).unwrap_err();
    assert!(
        matches!(&err, VmError::NativeError(msg) if msg == "something broke"),
        "NativeResult::Error must surface as VmError::NativeError, got: {err:?}"
    );
}

//
// ─── meow_vm_abort ───
//

#[test]
fn builtin_abort_triggers_on_false_condition() {
    // meow_vm_abort(condition, code, msg): aborts when condition is FALSE (assert semantics).
    let src = r#"
        mod test;

        pub fn check(x: u64) { meow_vm_abort(x != 0, 42, "must not be zero"); }
    "#;
    let vm = utils::vm_with_source(src);
    let mut gas = GasMeter::unlimited();

    // x=0: x!=0 = false → abort
    assert!(matches!(
        vm.call("check", vec![Value::U64(0)], &mut gas).unwrap_err(),
        VmError::Aborted { code: 42, ref message } if message == "must not be zero"
    ));

    // x=1: x!=0 = true → no abort
    assert!(
        vm.call("check", vec![Value::U64(1)], &mut GasMeter::unlimited())
            .is_ok()
    );
}

#[test]
fn abort_can_be_overridden_by_custom_native() {
    let src = r#"
        mod test;

        pub fn check(x: u64) { meow_vm_abort(x != 0, 99, "overridden"); }
    "#;
    let vm = utils::vm_with_natives(src, vec![doubling_abort_native()]);
    let mut gas = GasMeter::unlimited();

    let err = vm.call("check", vec![Value::U64(0)], &mut gas).unwrap_err();
    assert!(matches!(&err, VmError::Aborted { code: 198, message } if message == "custom"));
}

//
// ─── Move semantics ───
//

#[test]
fn use_after_move_is_an_error() {
    // The compiler statically rejects passing a moved struct to a second native call,
    // so this scenario is built from hand-crafted bytecode to verify the VM's own
    // runtime move guard still fires when consuming a moved value via a native.
    let module = Module {
        name: "test".to_string(),
        imports: vec![],
        structs: vec![StructDef {
            name: "Token".to_string(),
            is_public: true,
            fields: vec![FieldDef {
                name: "amount".to_string(),
                ty: Type::U64,
            }],
        }],
        functions: vec![Function {
            name: "consume_twice".to_string(),
            is_public: true,
            params: vec![("tok".to_string(), Type::Struct("Token".to_string()))],
            return_type: None,
            local_count: 1,
            code: vec![
                Instruction::Load(0), // move Token out of slot 0
                Instruction::Call("consume_native".to_string()),
                Instruction::Pop,     // discard the native's Void return
                Instruction::Load(0), // ← use after move
                Instruction::Call("consume_native".to_string()),
                Instruction::Return,
            ],
        }],
    };
    let vm = utils::vm_with_deps_and_natives(
        module,
        HashMap::new(),
        vec![utils::consume_native("consume_native")],
    );
    let mut gas = GasMeter::unlimited();
    let err = vm
        .call("consume_twice", vec![test_token(100)], &mut gas)
        .unwrap_err();
    assert!(
        matches!(&err, VmError::UseAfterMove(msg) if msg == "local slot 0 has already been moved")
    );
}

//
// ─── meow_vm_abort signature enforcement ───
//

#[test]
#[should_panic(expected = "meow_vm_abort override has wrong parameter types")]
fn meow_vm_abort_override_with_wrong_params_panics() {
    let src = r#"
        mod test;
        
        pub fn run() { meow_vm_abort(true, 0, "ok");
    }"#;
    let module = utils::compile(src);
    utils::vm_with_deps_and_natives(
        module,
        HashMap::new(),
        vec![NativeFnEntry {
            name: "meow_vm_abort".to_string(),
            params: vec![NativeParam::Concrete(Type::U64)], // wrong — should be (bool, u64, str)
            return_type: None,
            gas_cost: 0,
            func: Box::new(|_| NativeResult::Return(None)),
        }],
    );
}

#[test]
#[should_panic(expected = "meow_vm_abort override must return void")]
fn meow_vm_abort_override_with_wrong_return_type_panics() {
    let src = r#"mod test; pub fn run() { meow_vm_abort(true, 0, "ok"); }"#;
    let module = utils::compile(src);
    utils::vm_with_deps_and_natives(
        module,
        HashMap::new(),
        vec![NativeFnEntry {
            name: "meow_vm_abort".to_string(),
            params: vec![
                NativeParam::Concrete(Type::Bool),
                NativeParam::Concrete(Type::U64),
                NativeParam::Concrete(Type::Str),
            ],
            return_type: Some(Type::Bool), // wrong — must be void
            gas_cost: 0,
            func: Box::new(|_| NativeResult::Return(None)),
        }],
    );
}

//
// ─── Utility functions ───
//

fn test_token(amount: u64) -> Value {
    Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "Token"),
        fields: vec![("amount".to_string(), Value::U64(amount))],
    }
}

fn test_add_native() -> NativeFnEntry {
    NativeFnEntry {
        name: "add_native".to_string(),
        params: vec![
            NativeParam::Concrete(Type::U64),
            NativeParam::Concrete(Type::U64),
        ],
        return_type: Some(Type::U64),
        gas_cost: 5,
        func: Box::new(|args| {
            NativeResult::Return(Some(Value::U64(
                args[0].as_u64().unwrap() + args[1].as_u64().unwrap(),
            )))
        }),
    }
}

fn test_log_native() -> NativeFnEntry {
    NativeFnEntry {
        name: "log_native".to_string(),
        params: vec![NativeParam::Concrete(Type::U64)],
        return_type: None,
        gas_cost: 1,
        func: Box::new(|_| NativeResult::Return(None)),
    }
}

fn doubling_abort_native() -> NativeFnEntry {
    // Overrides meow_vm_abort; doubles the code to prove this fn ran.
    NativeFnEntry {
        name: "meow_vm_abort".to_string(),
        params: vec![
            NativeParam::Concrete(Type::Bool),
            NativeParam::Concrete(Type::U64),
            NativeParam::Concrete(Type::Str),
        ],
        return_type: None,
        gas_cost: 0,
        func: Box::new(|args| {
            if args[0].as_bool() == Some(false) {
                NativeResult::Abort {
                    code: args[1].as_u64().unwrap_or(0) * 2,
                    message: "custom".into(),
                }
            } else {
                NativeResult::Return(None)
            }
        }),
    }
}
