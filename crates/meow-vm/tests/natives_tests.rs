mod utils;

use meow_vm::{NativeFnEntry, NativeResult, error::VmError, gas_meter::GasMeter};
use meow_vm_types::{address::Address, types::Value};

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
// ─── meow_vm_abort ───
//

#[test]
fn builtin_abort_triggers_on_false_condition() {
    // meow_vm_abort(condition, code, msg): aborts when condition is FALSE (assert semantics).
    let src = r#"
        mod test;
        pub fn check(x: u64) { meow_vm_abort(x != 0, 42, "must not be zero"); }
    "#;
    let vm = utils::vm_with_natives(src, vec![]);
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
    let vm = utils::vm_with_natives(src, vec![test_doubling_abort_native()]);
    let mut gas = GasMeter::unlimited();

    let err = vm.call("check", vec![Value::U64(0)], &mut gas).unwrap_err();
    assert!(matches!(&err, VmError::Aborted { code: 198, message } if message == "custom"));
}

//
// ─── Move semantics ───
//

#[test]
fn use_after_move_is_an_error() {
    let src = r#"
        mod test;
        object Token { id: address, amount: u64 }

        pub fn consume_twice(tok: Token) { consume_native(tok); consume_native(tok); }
    "#;
    let vm = utils::vm_with_natives(src, vec![utils::consume_native("consume_native")]);
    let mut gas = GasMeter::unlimited();
    let err = vm
        .call("consume_twice", vec![test_token(100)], &mut gas)
        .unwrap_err();
    assert!(
        matches!(&err, VmError::UseAfterMove(msg) if msg == "local slot 0 has already been moved")
    );
}

//
// ─── final_args tracking ───
//

#[test]
fn final_args_holds_primitives_after_call() {
    let src = r#"
        mod test;
        pub fn f(a: u64, b: u64) -> u64 { a + b }
    "#;
    let vm = utils::vm_with_natives(src, vec![]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("f", vec![Value::U64(3), Value::U64(4)], &mut gas)
        .unwrap();
    assert_eq!(r.final_args, vec![Some(Value::U64(3)), Some(Value::U64(4))]);
}

#[test]
fn final_args_is_none_for_consumed_object() {
    let src = r#"
        mod test;
        object Token { id: address, amount: u64 }

        pub fn consume(tok: Token) { consume_native(tok); }
    "#;
    let vm = utils::vm_with_natives(src, vec![utils::consume_native("consume_native")]);
    let mut gas = GasMeter::unlimited();
    let r = vm.call("consume", vec![test_token(50)], &mut gas).unwrap();
    assert_eq!(r.final_args, vec![None]);
}

#[test]
fn final_args_is_some_for_surviving_object() {
    // An object that is neither transferred nor destroyed keeps its slot — final_args holds it.
    let src = r#"
        mod test;
        object Token { id: address, amount: u64 }

        pub fn read_amount(tok: Token) -> u64 { tok.amount }
    "#;
    let vm = utils::vm_with_natives(src, vec![]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("read_amount", vec![test_token(77)], &mut gas)
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(77)));
    assert_eq!(r.final_args, vec![Some(test_token(77))]);
}

#[test]
fn final_args_reflects_mutations_on_surviving_object() {
    // A mutated-but-not-consumed object surfaces with its updated field values.
    let src = r#"
        mod test;
        object Token { id: address, amount: u64 }

        pub fn double_amount(tok: Token) { tok.amount = tok.amount * 2; }
    "#;
    let vm = utils::vm_with_natives(src, vec![]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("double_amount", vec![test_token(30)], &mut gas)
        .unwrap();
    assert_eq!(r.final_args, vec![Some(test_token(60))]);
}

//
// ─── Utility functions ───
//

fn test_token(amount: u64) -> Value {
    Value::Object {
        type_name: "Token".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(Address::ZERO)),
            ("amount".to_string(), Value::U64(amount)),
        ],
    }
}

fn test_add_native() -> NativeFnEntry {
    NativeFnEntry {
        name: "add_native".to_string(),
        param_count: 2,
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
        param_count: 1,
        gas_cost: 1,
        func: Box::new(|_| NativeResult::Return(None)),
    }
}

fn test_doubling_abort_native() -> NativeFnEntry {
    // Overrides meow_vm_abort; doubles the code to prove this fn ran.
    NativeFnEntry {
        name: "meow_vm_abort".to_string(),
        param_count: 3,
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
