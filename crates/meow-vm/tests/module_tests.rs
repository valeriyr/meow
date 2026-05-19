mod utils;

use std::{collections::HashMap, str::FromStr};

use meow_vm::error::VmError;
use meow_vm_types::{address::Address, module_ref, types::Value};

//
// ─── Basic cross-module function call ───
//

#[test]
fn cross_module_function_call() {
    let a1 = Address::from_str("0x01").unwrap();
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
            a1
        ),
        &[(a1, &math)],
    );

    let vm = utils::vm_with_deps(caller, HashMap::from([(a1, math)]));
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
    let a10 = Address::from_str("0x10").unwrap();
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
            a10
        ),
        &[(a10, &shapes)],
    );

    let vm = utils::vm_with_deps(user, HashMap::from([(a10, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("make_and_read", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(5)));
}

//
// ─── Cross-module field read via getter function ───
//

#[test]
fn cross_module_field_read_via_getter() {
    let a10 = Address::from_str("0x10").unwrap();
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
            a10
        ),
        &[(a10, &shapes)],
    );

    let vm = utils::vm_with_deps(user, HashMap::from([(a10, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("read_x_via_getter", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(7)));
}

//
// ─── Same struct name in two different modules ───
//

#[test]
fn same_struct_name_in_different_modules_are_distinct() {
    let aa0 = Address::from_str("0xA0").unwrap();
    let ab0 = Address::from_str("0xB0").unwrap();

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

                use mod_a@{};
                use mod_b@{};

                pub fn run() -> u64 {{
                    let ta = mod_a::make_token(100);
                    let tb = mod_b::make_token(42);
                    mod_a::to_amount(ta) + mod_b::to_points(tb)
                }}
            "#,
            aa0, ab0
        ),
        &[(aa0, &mod_a), (ab0, &mod_b)],
    );

    let vm = utils::vm_with_deps(main, HashMap::from([(aa0, mod_a), (ab0, mod_b)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("run", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(142)));
}

//
// ─── Same module name, different address ───
//

#[test]
fn same_module_name_different_address_are_distinct() {
    let a01 = Address::from_str("0x01").unwrap();
    let a02 = Address::from_str("0x02").unwrap();
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

    // The caller was compiled against lib at address 0x01.
    let caller_v1 = utils::compile_with_deps(
        &format!(
            r#"
                mod caller;

                use lib@{};

                pub fn get() -> u64 {{ lib::version() }}
            "#,
            a01
        ),
        &[(a01, &lib_v1)],
    );

    // Supplying lib_v1 (address 0x01) returns 1 …
    let vm = utils::vm_with_deps(caller_v1.clone(), HashMap::from([(a01, lib_v1)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    assert_eq!(
        vm.call("get", vec![], &mut gas).unwrap().return_value,
        Some(Value::U64(1))
    );

    // … but the wrong module (lib_v2, address 0x02) can't be resolved.
    let vm_wrong = utils::vm_with_deps(caller_v1, HashMap::from([(a02, lib_v2)]));
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
    let a20 = Address::from_str("0x20").unwrap();
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
            a20
        ),
        &[(a20, &math)],
    );

    let vm = utils::vm_with_deps(caller, HashMap::from([(a20, math)]));
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
    let a30 = Address::from_str("0x30").unwrap();
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
            a30
        ),
        &[(a30, &geometry)],
    );

    let point = |x: u64, y: u64| Value::Struct {
        type_name: module_ref::qualify(&a30, "Point"),
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

    let vm = utils::vm_with_deps(shapes, HashMap::from([(a30, geometry)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("x_distance", vec![line], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(7)));
}

//
// ─── Deeply nested structs across three modules ───
//

#[test]
fn cross_module_deeply_nested_structs() {
    let a10 = Address::from_str("0x10").unwrap();
    let a20 = Address::from_str("0x20").unwrap();

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
            a10
        ),
        &[(a10, &point)],
    );

    let geometry = utils::compile_with_deps(
        &format!(
            r#"
                mod geometry;

                use shapes@{a20};

                pub struct Rect {{ l1: shapes::Line, l2: shapes::Line }}

                pub fn left_top_x(rect: Rect) -> u64 {{
                    let Rect {{ l1, l2 }} = rect;
                    shapes::to_a_x(l2);
                    shapes::to_a_x(l1)
                }}
            "#,
            a20 = a20,
        ),
        &[(a10, &point), (a20, &shapes)],
    );

    let mk_point = |x: u64, y: u64| Value::Struct {
        type_name: module_ref::qualify(&a10, "Point"),
        fields: vec![
            ("x".to_string(), Value::U64(x)),
            ("y".to_string(), Value::U64(y)),
        ],
    };
    let mk_line = |a, b| Value::Struct {
        type_name: module_ref::qualify(&a20, "Line"),
        fields: vec![("a".to_string(), a), ("b".to_string(), b)],
    };
    let rect = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "Rect"),
        fields: vec![
            ("l1".to_string(), mk_line(mk_point(3, 0), mk_point(9, 0))),
            ("l2".to_string(), mk_line(mk_point(0, 0), mk_point(0, 0))),
        ],
    };

    let vm = utils::vm_with_deps(geometry, HashMap::from([(a10, point), (a20, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("left_top_x", vec![rect], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(3)));
}

//
// ─── Chained calls: caller → dep A → dep B ───
//

#[test]
fn chained_cross_module_calls() {
    let a60 = Address::from_str("0x60").unwrap();
    let base = utils::compile(
        r#"
            mod base;

            pub fn one() -> u64 { 1 }
        "#,
    );

    let a61 = Address::from_str("0x61").unwrap();
    let mid = utils::compile_with_deps(
        &format!(
            r#"
                mod mid;

                use base@{};

                pub fn two() -> u64 {{ base::one() + base::one() }}
            "#,
            a60
        ),
        &[(a60, &base)],
    );

    let top = utils::compile_with_deps(
        &format!(
            r#"
                mod top;

                use mid@{};

                pub fn four() -> u64 {{ mid::two() + mid::two() }}
            "#,
            a61
        ),
        &[(a60, &base), (a61, &mid)],
    );

    let vm = utils::vm_with_deps(top, HashMap::from([(a61, mid), (a60, base)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("four", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(4)));
}

//
// ─── Module aliases (`use mod@addr as alias`) ───
//

#[test]
fn alias_used_for_cross_module_function_call() {
    let a1 = Address::from_str("0x01").unwrap();
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
            a1
        ),
        &[(a1, &math)],
    );

    let vm = utils::vm_with_deps(caller, HashMap::from([(a1, math)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm
        .call("run", vec![Value::U64(10), Value::U64(3)], &mut gas)
        .unwrap();
    assert_eq!(result.return_value, Some(Value::U64(13)));
}

#[test]
fn alias_used_for_cross_module_struct() {
    let a10 = Address::from_str("0x10").unwrap();
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
            a10
        ),
        &[(a10, &shapes)],
    );

    let vm = utils::vm_with_deps(user, HashMap::from([(a10, shapes)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("run", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(5)));
}

#[test]
fn two_modules_same_name_distinguished_by_alias() {
    let a01 = Address::from_str("0x01").unwrap();
    let a02 = Address::from_str("0x02").unwrap();

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

                use math@{a01} as math1;
                use math@{a02} as math2;

                pub fn run() -> u64 {{ math1::value() + math2::value() }}
            "#,
            a01 = a01,
            a02 = a02,
        ),
        &[(a01, &math_v1), (a02, &math_v2)],
    );

    let vm = utils::vm_with_deps(caller, HashMap::from([(a01, math_v1), (a02, math_v2)]));
    let mut gas = meow_vm::gas_meter::GasMeter::unlimited();
    let result = vm.call("run", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(300)));
}
