mod utils;

use std::{collections::HashMap, str::FromStr};

use meow_vm::error::VmError;
use meow_vm_types::{address::Address, module_ref, types::Value};

//
// ─── Basic cross-module function call ───
//

#[test]
fn cross_module_function_call() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let math = utils::compile(
        r#"
            mod math;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
    );

    let caller = utils::compile_with_deps(
        &format!(
            r#"
                mod caller;

                use math@{};

                pub fn double_add(a: u64, b: u64) -> u64 {{
                    math::add(a, b) + math::add(a, b)
                }}
            "#,
            d_addr
        ),
        &[(d_addr, &math)],
    );

    let vm = utils::vm_with_deps(caller, HashMap::from([(d_addr, math)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm
        .call("double_add", vec![Value::U64(3), Value::U64(4)], &mut gas)
        .unwrap();
    assert_eq!(result.return_value, Some(Value::U64(14)));
}

//
// ─── Cross-module struct: constructor function + field access ───
//
// Structs/objects can only be constructed inside the declaring module.
// Cross-module callers must use constructor functions provided by the dep module.
//

#[test]
fn cross_module_struct_via_constructor_and_field_access() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let shapes = utils::compile(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make_point(x: u64, y: u64) -> Point { Point { x: x, y: y } }
            pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
        "#,
    );

    let user = utils::compile_with_deps(
        &format!(
            r#"
                mod user;

                use shapes@{};

                pub fn make_and_read() -> u64 {{
                    let p = shapes::make_point(5, 9);
                    shapes::to_x(p)
                }}
            "#,
            d_addr
        ),
        &[(d_addr, &shapes)],
    );

    let vm = utils::vm_with_deps(user, HashMap::from([(d_addr, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("make_and_read", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(5)));
}

//
// ─── Cross-module field read via getter function ───
//

#[test]
fn cross_module_field_read_via_getter() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let shapes = utils::compile(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make_point(x: u64, y: u64) -> Point { Point { x: x, y: y } }
            pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
        "#,
    );

    let user = utils::compile_with_deps(
        &format!(
            r#"
                mod user;

                use shapes@{};

                pub fn read_x_via_getter() -> u64 {{
                    let p = shapes::make_point(7, 3);
                    shapes::to_x(p)
                }}
            "#,
            d_addr
        ),
        &[(d_addr, &shapes)],
    );

    let vm = utils::vm_with_deps(user, HashMap::from([(d_addr, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("read_x_via_getter", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(7)));
}

//
// ─── Same struct name in two different modules ───
//

#[test]
fn same_struct_name_in_different_modules_are_distinct() {
    let a_addr = Address::from_str("0xFA").unwrap();
    let b_addr = Address::from_str("0xFB").unwrap();

    let mod_a = utils::compile(
        r#"
            mod mod_a;

            pub struct Token { amount: u64 }

            pub fn make_token(amount: u64) -> Token { Token { amount: amount } }
            pub fn to_amount(t: Token) -> u64 { let Token { amount } = t; amount }
        "#,
    );
    let mod_b = utils::compile(
        r#"
            mod mod_b;

            pub struct Token { points: u64 }

            pub fn make_token(points: u64) -> Token { Token { points: points } }
            pub fn to_points(t: Token) -> u64 { let Token { points } = t; points }
        "#,
    );

    let main = utils::compile_with_deps(
        &format!(
            r#"
                mod main;

                use mod_a@{a_addr};
                use mod_b@{b_addr};

                pub fn run() -> u64 {{
                    let ta = mod_a::make_token(100);
                    let tb = mod_b::make_token(42);
                    mod_a::to_amount(ta) + mod_b::to_points(tb)
                }}
            "#,
            a_addr = a_addr,
            b_addr = b_addr
        ),
        &[(a_addr, &mod_a), (b_addr, &mod_b)],
    );

    let vm = utils::vm_with_deps(main, HashMap::from([(a_addr, mod_a), (b_addr, mod_b)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("run", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(142)));
}

//
// ─── Same module name, different address ───
//

#[test]
fn same_module_name_different_address_are_distinct() {
    let a_addr = Address::from_str("0xFA").unwrap();
    let b_addr = Address::from_str("0xFB").unwrap();
    let lib_v1 = utils::compile(
        r#"
            mod lib;

            pub fn version() -> u64 { 1 }
        "#,
    );
    let lib_v2 = utils::compile(
        r#"
            mod lib;

            pub fn version() -> u64 { 2 }
        "#,
    );

    // The caller was compiled against lib at address 0xFA.
    let caller_v1 = utils::compile_with_deps(
        &format!(
            r#"
                mod caller;

                use lib@{};

                pub fn get() -> u64 {{ lib::version() }}
            "#,
            a_addr
        ),
        &[(a_addr, &lib_v1)],
    );

    // Supplying lib_v1 (address 0xFA) returns 1 …
    let vm = utils::vm_with_deps(caller_v1.clone(), HashMap::from([(a_addr, lib_v1)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    assert_eq!(
        vm.call("get", vec![], &mut gas).unwrap().return_value,
        Some(Value::U64(1))
    );

    // … but the wrong module (lib_v2, address 0xFB) can't be resolved.
    let vm_wrong = utils::vm_with_deps(caller_v1, HashMap::from([(b_addr, lib_v2)]));
    let mut gas2 = meow_vm::gas_meter::GasMeter::unlimited();
    assert!(matches!(
        vm_wrong.call("get", vec![], &mut gas2).unwrap_err(),
        VmError::UndefinedFunction(_)
    ));
}

//
// ─── Dep module's internal calls resolve in dep's own context ───
//

#[test]
fn dep_module_internal_calls_stay_in_dep_context() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let math = utils::compile(
        r#"
            mod math;

            fn square(x: u64) -> u64 { x * x }
            pub fn sum_of_squares(a: u64, b: u64) -> u64 { square(a) + square(b) }
        "#,
    );

    let caller = utils::compile_with_deps(
        &format!(
            r#"
                mod caller;

                use math@{};

                pub fn run(a: u64, b: u64) -> u64 {{ math::sum_of_squares(a, b) }}
            "#,
            d_addr
        ),
        &[(d_addr, &math)],
    );

    let vm = utils::vm_with_deps(caller, HashMap::from([(d_addr, math)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm
        .call("run", vec![Value::U64(3), Value::U64(4)], &mut gas)
        .unwrap();
    assert_eq!(result.return_value, Some(Value::U64(25))); // 9 + 16
}

//
// ─── Nested structs across modules ───
//

#[test]
fn cross_module_nested_structs() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let geometry = utils::compile(
        r#"
            mod geometry;

            pub struct Point { x: u64, y: u64 }

            pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
        "#,
    );

    let shapes = utils::compile_with_deps(
        &format!(
            r#"
                mod shapes;

                use geometry@{};

                pub struct Line {{ a: geometry::Point, b: geometry::Point }}

                pub fn x_distance(line: Line) -> u64 {{
                    let Line {{ a, b }} = line;
                    geometry::to_x(b) - geometry::to_x(a)
                }}
            "#,
            d_addr
        ),
        &[(d_addr, &geometry)],
    );

    let point = |x: u64, y: u64| Value::Struct {
        type_name: module_ref::qualify(&d_addr, "Point"),
        fields: vec![
            ("x".to_string(), Value::U64(x)),
            ("y".to_string(), Value::U64(y)),
        ],
    };
    let line = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "Line"),
        fields: vec![
            ("a".to_string(), point(2, 0)),
            ("b".to_string(), point(9, 0)),
        ],
    };

    let vm = utils::vm_with_deps(shapes, HashMap::from([(d_addr, geometry)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("x_distance", vec![line], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(7)));
}

//
// ─── Deeply nested structs across three modules ───
//

#[test]
fn cross_module_deeply_nested_structs() {
    let a_addr = Address::from_str("0xFA").unwrap();
    let b_addr = Address::from_str("0xFB").unwrap();

    let point = utils::compile(
        r#"
            mod point;

            pub struct Point { x: u64, y: u64 }

            pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
        "#,
    );

    let shapes = utils::compile_with_deps(
        &format!(
            r#"
                mod shapes;

                use point@{};

                pub struct Line {{ a: point::Point, b: point::Point }}

                pub fn to_a_x(line: Line) -> u64 {{
                    let Line {{ a, b }} = line;
                    point::to_x(b);
                    point::to_x(a)
                }}
            "#,
            a_addr
        ),
        &[(a_addr, &point)],
    );

    let geometry = utils::compile_with_deps(
        &format!(
            r#"
                mod geometry;

                use shapes@{};

                pub struct Rect {{ l1: shapes::Line, l2: shapes::Line }}

                pub fn left_top_x(rect: Rect) -> u64 {{
                    let Rect {{ l1, l2 }} = rect;
                    shapes::to_a_x(l2);
                    shapes::to_a_x(l1)
                }}
            "#,
            b_addr,
        ),
        &[(a_addr, &point), (b_addr, &shapes)],
    );

    let mk_point = |x: u64, y: u64| Value::Struct {
        type_name: module_ref::qualify(&a_addr, "Point"),
        fields: vec![
            ("x".to_string(), Value::U64(x)),
            ("y".to_string(), Value::U64(y)),
        ],
    };
    let mk_line = |a, b| Value::Struct {
        type_name: module_ref::qualify(&b_addr, "Line"),
        fields: vec![("a".to_string(), a), ("b".to_string(), b)],
    };
    let rect = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "Rect"),
        fields: vec![
            ("l1".to_string(), mk_line(mk_point(3, 0), mk_point(9, 0))),
            ("l2".to_string(), mk_line(mk_point(0, 0), mk_point(0, 0))),
        ],
    };

    let vm = utils::vm_with_deps(geometry, HashMap::from([(a_addr, point), (b_addr, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("left_top_x", vec![rect], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(3)));
}

//
// ─── Chained calls: caller → dep A → dep B ───
//

#[test]
fn chained_cross_module_calls() {
    let a_addr = Address::from_str("0xFA").unwrap();
    let base = utils::compile(
        r#"
            mod base;

            pub fn one() -> u64 { 1 }
        "#,
    );

    let b_addr = Address::from_str("0xFB").unwrap();
    let mid = utils::compile_with_deps(
        &format!(
            r#"
                mod mid;

                use base@{};

                pub fn two() -> u64 {{ base::one() + base::one() }}
            "#,
            a_addr
        ),
        &[(a_addr, &base)],
    );

    let top = utils::compile_with_deps(
        &format!(
            r#"
                mod top;

                use mid@{};

                pub fn four() -> u64 {{ mid::two() + mid::two() }}
            "#,
            b_addr
        ),
        &[(a_addr, &base), (b_addr, &mid)],
    );

    let vm = utils::vm_with_deps(top, HashMap::from([(b_addr, mid), (a_addr, base)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("four", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(4)));
}

//
// ─── Module aliases (`use mod@addr as alias`) ───
//

#[test]
fn alias_used_for_cross_module_function_call() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let math = utils::compile(
        r#"
            mod math;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
    );

    let caller = utils::compile_with_deps(
        &format!(
            r#"
                mod caller;

                use math@{} as m;

                pub fn run(a: u64, b: u64) -> u64 {{ m::add(a, b) }}
            "#,
            d_addr
        ),
        &[(d_addr, &math)],
    );

    let vm = utils::vm_with_deps(caller, HashMap::from([(d_addr, math)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm
        .call("run", vec![Value::U64(10), Value::U64(3)], &mut gas)
        .unwrap();
    assert_eq!(result.return_value, Some(Value::U64(13)));
}

#[test]
fn alias_used_for_cross_module_struct() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let shapes = utils::compile(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make_point(x: u64, y: u64) -> Point { Point { x: x, y: y } }
            pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
        "#,
    );

    let user = utils::compile_with_deps(
        &format!(
            r#"
                mod user;

                use shapes@{} as geo;

                pub fn run() -> u64 {{
                    let p = geo::make_point(5, 9);
                    geo::to_x(p)
                }}
            "#,
            d_addr
        ),
        &[(d_addr, &shapes)],
    );

    let vm = utils::vm_with_deps(user, HashMap::from([(d_addr, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("run", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(5)));
}

#[test]
fn two_modules_same_name_distinguished_by_alias() {
    let a_addr = Address::from_str("0xFA").unwrap();
    let b_addr = Address::from_str("0xFB").unwrap();

    let math_v1 = utils::compile(
        r#"
            mod math;

            pub fn value() -> u64 { 100 }
        "#,
    );
    let math_v2 = utils::compile(
        r#"
            mod math;

            pub fn value() -> u64 { 200 }
        "#,
    );

    let caller = utils::compile_with_deps(
        &format!(
            r#"
                mod caller;

                use math@{a_addr} as math1;
                use math@{b_addr} as math2;

                pub fn run() -> u64 {{ math1::value() + math2::value() }}
            "#,
            a_addr = a_addr,
            b_addr = b_addr,
        ),
        &[(a_addr, &math_v1), (b_addr, &math_v2)],
    );

    let vm = utils::vm_with_deps(
        caller,
        HashMap::from([(a_addr, math_v1), (b_addr, math_v2)]),
    );
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("run", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(300)));
}
