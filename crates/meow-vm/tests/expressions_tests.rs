mod utils;

use meow_vm::{error::VmError, gas_meter::GasMeter};
use meow_vm_types::types::Value;

//
// ─── Arithmetic ───
//

#[test]
fn add() {
    assert_eq!(
        utils::run(ADD_SRC, "add", vec![Value::U64(3), Value::U64(4)]),
        Some(Value::U64(7))
    );
}

#[test]
fn sub() {
    assert_eq!(
        utils::run(SUB_SRC, "sub", vec![Value::U64(10), Value::U64(3)]),
        Some(Value::U64(7))
    );
}

#[test]
fn mul() {
    assert_eq!(
        utils::run(MUL_SRC, "mul", vec![Value::U64(6), Value::U64(7)]),
        Some(Value::U64(42))
    );
}

#[test]
fn div() {
    assert_eq!(
        utils::run(DIV_SRC, "div", vec![Value::U64(20), Value::U64(4)]),
        Some(Value::U64(5))
    );
}

#[test]
fn modulo() {
    assert_eq!(
        utils::run(REM_SRC, "rem", vec![Value::U64(10), Value::U64(3)]),
        Some(Value::U64(1))
    );
}

#[test]
fn modulo_zero_dividend() {
    assert_eq!(
        utils::run(REM_SRC, "rem", vec![Value::U64(0), Value::U64(7)]),
        Some(Value::U64(0))
    );
}

#[test]
fn modulo_exact_division() {
    assert_eq!(
        utils::run(REM_SRC, "rem", vec![Value::U64(12), Value::U64(4)]),
        Some(Value::U64(0))
    );
}

#[test]
fn modulo_by_one() {
    assert_eq!(
        utils::run(REM_SRC, "rem", vec![Value::U64(999), Value::U64(1)]),
        Some(Value::U64(0))
    );
}

#[test]
fn modulo_in_grouped_expression() {
    // `(a + b) % c` using explicit parentheses for grouping.
    assert_eq!(
        utils::run(
            GROUP_EXPR_SRC_WITH_PARENTHESES,
            "f",
            vec![Value::U64(7), Value::U64(8), Value::U64(5)]
        ),
        Some(Value::U64(0)) // (7 + 8) % 5 = 15 % 5 = 0
    );
}

#[test]
fn modulo_precedence_matches_mul() {
    // `a + b % c` should be `a + (b % c)`, same precedence as `*` and `/`.
    assert_eq!(
        utils::run(
            GROUP_EXPR_SRC_WITHOUT_PARENTHESES,
            "f",
            vec![Value::U64(10), Value::U64(7), Value::U64(4)]
        ),
        Some(Value::U64(13)) // 10 + (7 % 4) = 10 + 3 = 13
    );
}

#[test]
fn modulo_in_condition() {
    // Modulo result used in a boolean comparison — typical even/odd check.
    assert_eq!(
        utils::run(EVEN_SRC, "is_even", vec![Value::U64(8)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(EVEN_SRC, "is_even", vec![Value::U64(9)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn division_by_zero() {
    let vm = utils::vm(utils::compile(DIV_SRC));
    let mut gas = GasMeter::unlimited();
    let err = vm
        .call("div", vec![Value::U64(10), Value::U64(0)], &mut gas)
        .unwrap_err();
    assert!(matches!(err, VmError::DivisionByZero));
}

#[test]
fn modulo_by_zero() {
    let vm = utils::vm(utils::compile(REM_SRC));
    let mut gas = GasMeter::unlimited();
    let err = vm
        .call("rem", vec![Value::U64(10), Value::U64(0)], &mut gas)
        .unwrap_err();
    assert!(matches!(err, VmError::DivisionByZero));
}

//
// ─── Comparisons ───
//

#[test]
fn eq() {
    assert_eq!(
        utils::run(EQUAL_SRC, "eq", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(EQUAL_SRC, "eq", vec![Value::U64(5), Value::U64(6)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn ne() {
    assert_eq!(
        utils::run(NOT_EQUAL_SRC, "ne", vec![Value::U64(5), Value::U64(6)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(NOT_EQUAL_SRC, "ne", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn lt() {
    assert_eq!(
        utils::run(LT_SRC, "lt", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(LT_SRC, "lt", vec![Value::U64(5), Value::U64(3)]),
        Some(Value::Bool(false))
    );
    assert_eq!(
        utils::run(LT_SRC, "lt", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn le() {
    assert_eq!(
        utils::run(LE_SRC, "le", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(LE_SRC, "le", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(LE_SRC, "le", vec![Value::U64(6), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn gt() {
    assert_eq!(
        utils::run(GT_SRC, "gt", vec![Value::U64(5), Value::U64(3)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(GT_SRC, "gt", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(false))
    );
    assert_eq!(
        utils::run(GT_SRC, "gt", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn ge() {
    assert_eq!(
        utils::run(GE_SRC, "ge", vec![Value::U64(5), Value::U64(3)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(GE_SRC, "ge", vec![Value::U64(5), Value::U64(5)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(GE_SRC, "ge", vec![Value::U64(3), Value::U64(5)]),
        Some(Value::Bool(false))
    );
}

//
// ─── Boolean logic ───
//

#[test]
fn bool_and() {
    assert_eq!(
        utils::run(AND_SRC, "f", vec![Value::Bool(true), Value::Bool(true)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(AND_SRC, "f", vec![Value::Bool(true), Value::Bool(false)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn bool_or() {
    assert_eq!(
        utils::run(OR_SRC, "f", vec![Value::Bool(false), Value::Bool(true)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(OR_SRC, "f", vec![Value::Bool(false), Value::Bool(false)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn bool_not() {
    assert_eq!(
        utils::run(NOT_SRC, "f", vec![Value::Bool(true)]),
        Some(Value::Bool(false))
    );
    assert_eq!(
        utils::run(NOT_SRC, "f", vec![Value::Bool(false)]),
        Some(Value::Bool(true))
    );
}

#[test]
fn bool_not_double_negation() {
    assert_eq!(
        utils::run(DOUBLE_NOT_SRC, "f", vec![Value::Bool(true)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(DOUBLE_NOT_SRC, "f", vec![Value::Bool(false)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn bool_not_in_condition() {
    // `if !flag { 0 } else { 1 }` — not used in a guard expression
    assert_eq!(
        utils::run(NOT_COND_SRC, "f", vec![Value::Bool(false)]),
        Some(Value::U64(99))
    );
    assert_eq!(
        utils::run(NOT_COND_SRC, "f", vec![Value::Bool(true)]),
        Some(Value::U64(0))
    );
}

//
// ─── Operator precedence ───
//

#[test]
fn mul_before_add() {
    // 2 + 3 * 4 must equal 14, not 20
    let src = r#"
        mod test;

        pub fn f() -> u64 { 2 + 3 * 4 }
    "#;
    assert_eq!(utils::run(src, "f", vec![]), Some(Value::U64(14)));
}

#[test]
fn mul_before_sub() {
    // 10 - 2 * 3 must equal 4, not 24
    let src = r#"
        mod test;

        pub fn f() -> u64 { 10 - 2 * 3 }
    "#;
    assert_eq!(utils::run(src, "f", vec![]), Some(Value::U64(4)));
}

#[test]
fn sub_is_left_associative() {
    // 10 - 3 - 2 must equal 5 (left-to-right), not 9
    let src = r#"
        mod test;

        pub fn f() -> u64 { 10 - 3 - 2 }
    "#;
    assert_eq!(utils::run(src, "f", vec![]), Some(Value::U64(5)));
}

#[test]
fn parens_override_precedence() {
    // (2 + 3) * 4 must equal 20
    let src = r#"
        mod test;

        pub fn f() -> u64 { (2 + 3) * 4 }
    "#;
    assert_eq!(utils::run(src, "f", vec![]), Some(Value::U64(20)));
}

//
// ─── u64 wrapping arithmetic ───
//

#[test]
fn add_wraps_on_overflow() {
    let src = r#"
        mod test;

        pub fn f() -> u64 { 18446744073709551615 + 1 }
    "#;
    assert_eq!(utils::run(src, "f", vec![]), Some(Value::U64(0)));
}

#[test]
fn sub_wraps_on_underflow() {
    let src = r#"
        mod test;

        pub fn f() -> u64 { 0 - 1 }
    "#;
    assert_eq!(utils::run(src, "f", vec![]), Some(Value::U64(u64::MAX)));
}

#[test]
fn mul_wraps_on_overflow() {
    let src = r#"
        mod test;

        pub fn f() -> u64 { 9223372036854775808 * 2 }
    "#;
    assert_eq!(utils::run(src, "f", vec![]), Some(Value::U64(0)));
}

//
// ─── Utilities ───
//

const ADD_SRC: &str = r#"
        mod math;

        pub fn add(a: u64, b: u64) -> u64 { a + b }
    "#;

const SUB_SRC: &str = r#"
        mod math;

        pub fn sub(a: u64, b: u64) -> u64 { a - b }
    "#;

const MUL_SRC: &str = r#"
        mod math;

        pub fn mul(a: u64, b: u64) -> u64 { a * b }
    "#;

const DIV_SRC: &str = r#"
        mod math;

        pub fn div(a: u64, b: u64) -> u64 { a / b }
    "#;

const REM_SRC: &str = r#"
        mod math;

        pub fn rem(a: u64, b: u64) -> u64 { a % b }
    "#;

const GROUP_EXPR_SRC_WITH_PARENTHESES: &str = r#"
        mod math;

        pub fn f(a: u64, b: u64, c: u64) -> u64 { (a + b) % c }
    "#;

const GROUP_EXPR_SRC_WITHOUT_PARENTHESES: &str = r#"
        mod math;

        pub fn f(a: u64, b: u64, c: u64) -> u64 { a + b % c }
    "#;

const EVEN_SRC: &str = r#"
        mod math;

        pub fn is_even(n: u64) -> bool { n % 2 == 0 }
    "#;

const EQUAL_SRC: &str = r#"
        mod math;

        pub fn eq(a: u64, b: u64) -> bool { a == b }
    "#;

const NOT_EQUAL_SRC: &str = r#"
        mod math;

        pub fn ne(a: u64, b: u64) -> bool { a != b }
    "#;

const LT_SRC: &str = r#"
        mod math;

        pub fn lt(a: u64, b: u64) -> bool { a < b }
    "#;

const LE_SRC: &str = r#"
        mod math;

        pub fn le(a: u64, b: u64) -> bool { a <= b }
    "#;

const GT_SRC: &str = r#"
        mod math;

        pub fn gt(a: u64, b: u64) -> bool { a > b }
    "#;

const GE_SRC: &str = r#"
        mod math;

        pub fn ge(a: u64, b: u64) -> bool { a >= b }
    "#;

const AND_SRC: &str = r#"
        mod math;

        pub fn f(a: bool, b: bool) -> bool { a && b }
    "#;

const OR_SRC: &str = r#"
        mod math;

        pub fn f(a: bool, b: bool) -> bool { a || b }
    "#;

const NOT_SRC: &str = r#"
        mod math;

        pub fn f(a: bool) -> bool { !a }
    "#;

const DOUBLE_NOT_SRC: &str = r#"
        mod math;

        pub fn f(a: bool) -> bool { !!a }
    "#;

const NOT_COND_SRC: &str = r#"
        mod math;

        pub fn f(flag: bool) -> u64 {
            if !flag { return 99; }
            0
        }
    "#;
