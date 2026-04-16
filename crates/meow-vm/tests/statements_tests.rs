mod utils;

use meow_vm::gas_meter::GasMeter;
use meow_vm_types::types::Value;

//
// ─── let ───
//

#[test]
fn let_binding() {
    let src = r#"
        module test;
        pub fn compute(x: u64): u64 { let a = x + 1; let b = a * 2; return b; }
    "#;
    assert_eq!(
        utils::run(src, "compute", vec![Value::U64(4)]),
        Some(Value::U64(10))
    );
}

//
// ─── if ───
//

#[test]
fn if_branch_taken() {
    let src = r#"
        module test;
        pub fn max(a: u64, b: u64): u64 { if a > b { return a; } return b; }
    "#;
    assert_eq!(
        utils::run(src, "max", vec![Value::U64(10), Value::U64(5)]),
        Some(Value::U64(10))
    );
    assert_eq!(
        utils::run(src, "max", vec![Value::U64(3), Value::U64(8)]),
        Some(Value::U64(8))
    );
}

#[test]
fn if_mutates_local() {
    let src = r#"
        module test;
        pub fn clamp(x: u64, max: u64): u64 {
            let result = x;
            if x > max { result = max; }
            return result;
        }
    "#;
    assert_eq!(
        utils::run(src, "clamp", vec![Value::U64(15), Value::U64(10)]),
        Some(Value::U64(10))
    );
    assert_eq!(
        utils::run(src, "clamp", vec![Value::U64(5), Value::U64(10)]),
        Some(Value::U64(5))
    );
}

//
// ─── if / else ───
//

#[test]
fn if_else_branches() {
    let src = r#"
        module test;
        pub fn classify(x: u64): u64 { if x > 10 { return 1; } else { return 0; } }
    "#;
    assert_eq!(
        utils::run(src, "classify", vec![Value::U64(20)]),
        Some(Value::U64(1))
    );
    assert_eq!(
        utils::run(src, "classify", vec![Value::U64(5)]),
        Some(Value::U64(0))
    );
}

#[test]
fn if_else_with_let_in_both_branches() {
    let src = r#"
        module test;

        pub fn abs_diff(a: u64, b: u64): u64 {
            if a > b { return a - b; } else { return b - a; }
        }
    "#;
    assert_eq!(
        utils::run(src, "abs_diff", vec![Value::U64(10), Value::U64(3)]),
        Some(Value::U64(7))
    );
    assert_eq!(
        utils::run(src, "abs_diff", vec![Value::U64(3), Value::U64(10)]),
        Some(Value::U64(7))
    );
}

//
// ─── Function calls ───
//

#[test]
fn function_call_chain() {
    let src = r#"
        module test;

        pub fn double(n: u64): u64 { return n * 2; }
        pub fn quad(n: u64): u64 { return double(double(n)); }
    "#;
    assert_eq!(
        utils::run(src, "quad", vec![Value::U64(3)]),
        Some(Value::U64(12))
    );
}

//
// ─── Void functions ───
//

#[test]
fn void_function_returns_none() {
    let src = r#"
        module test;
        pub fn do_nothing() {}
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();
    let r = vm.call("do_nothing", vec![], &mut gas).unwrap();
    assert_eq!(r.return_value, None);
    assert!(r.final_args.is_empty());
}

#[test]
fn void_call_as_statement_does_not_corrupt_stack() {
    let src = r#"
        module test;

        pub fn noop() {}
        pub fn compute(x: u64): u64 { noop(); return x * 2; }
    "#;
    assert_eq!(
        utils::run(src, "compute", vec![Value::U64(5)]),
        Some(Value::U64(10))
    );
}
