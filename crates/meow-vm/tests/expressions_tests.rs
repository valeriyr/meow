mod utils;

use meow_vm::{Vm, error::VmError, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_types::types::Value;
use utils::{compile, run};

//
// ─── Arithmetic ───
//

#[test]
fn add() {
    assert_eq!(
        run(
            "fn add(a: u64, b: u64): u64 { return a + b; }",
            "add",
            vec![Value::U64(3), Value::U64(4)]
        ),
        Some(Value::U64(7))
    );
}

#[test]
fn sub() {
    assert_eq!(
        run(
            "fn sub(a: u64, b: u64): u64 { return a - b; }",
            "sub",
            vec![Value::U64(10), Value::U64(3)]
        ),
        Some(Value::U64(7))
    );
}

#[test]
fn mul() {
    assert_eq!(
        run(
            "fn mul(a: u64, b: u64): u64 { return a * b; }",
            "mul",
            vec![Value::U64(6), Value::U64(7)]
        ),
        Some(Value::U64(42))
    );
}

#[test]
fn div() {
    assert_eq!(
        run(
            "fn div(a: u64, b: u64): u64 { return a / b; }",
            "div",
            vec![Value::U64(20), Value::U64(4)]
        ),
        Some(Value::U64(5))
    );
}

#[test]
fn division_by_zero() {
    let vm = Vm::new(
        compile("fn div(a: u64, b: u64): u64 { return a / b; }"),
        vec![],
        GasSchedule::default(),
    );
    let mut gas = GasMeter::unlimited();
    let err = vm
        .call("div", vec![Value::U64(10), Value::U64(0)], &mut gas)
        .unwrap_err();
    assert!(matches!(err, VmError::DivisionByZero));
}

//
// ─── Comparisons ───
//

#[test]
fn eq() {
    let src = "fn eq(a: u64, b: u64): bool { return a == b; }";
    assert_eq!(
        run(src, "eq", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "eq", vec![Value::U64(5), Value::U64(6)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn ne() {
    let src = "fn ne(a: u64, b: u64): bool { return a != b; }";
    assert_eq!(
        run(src, "ne", vec![Value::U64(5), Value::U64(6)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "ne", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn lt() {
    let src = "fn lt(a: u64, b: u64): bool { return a < b; }";
    assert_eq!(
        run(src, "lt", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "lt", vec![Value::U64(5), Value::U64(3)]),
        Some(Value::Bool(false))
    );
    assert_eq!(
        run(src, "lt", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn le() {
    let src = "fn le(a: u64, b: u64): bool { return a <= b; }";
    assert_eq!(
        run(src, "le", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "le", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "le", vec![Value::U64(6), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn gt() {
    let src = "fn gt(a: u64, b: u64): bool { return a > b; }";
    assert_eq!(
        run(src, "gt", vec![Value::U64(5), Value::U64(3)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "gt", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(false))
    );
    assert_eq!(
        run(src, "gt", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn ge() {
    let src = "fn ge(a: u64, b: u64): bool { return a >= b; }";
    assert_eq!(
        run(src, "ge", vec![Value::U64(5), Value::U64(3)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "ge", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "ge", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

//
// ─── Boolean logic ───
//

#[test]
fn bool_and() {
    let src = "fn f(a: bool, b: bool): bool { return a && b; }";
    assert_eq!(
        run(src, "f", vec![Value::Bool(true), Value::Bool(true)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "f", vec![Value::Bool(true), Value::Bool(false)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn bool_or() {
    let src = "fn f(a: bool, b: bool): bool { return a || b; }";
    assert_eq!(
        run(src, "f", vec![Value::Bool(false), Value::Bool(true)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(src, "f", vec![Value::Bool(false), Value::Bool(false)]),
        Some(Value::Bool(false))
    );
}
