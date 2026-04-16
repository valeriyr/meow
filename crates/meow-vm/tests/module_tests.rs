mod utils;

use std::{collections::HashMap, str::FromStr};

use meow_vm::error::VmError;
use meow_vm_types::{address::Address, types::Value};

//
// ─── Basic cross-module function call ───
//

#[test]
fn cross_module_function_call() {
    let a1 = Address::from_str("0x01").unwrap();
    let math = utils::compile(
        r#"
            module math;

            fn add(a: u64, b: u64): u64 { return a + b; }
        "#,
    );

    let caller = utils::compile_with_deps(
        &format!(
            r#"
                module caller;

                use math@{};

                fn double_add(a: u64, b: u64): u64 {{
                    return math::add(a, b) + math::add(a, b);
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
// ─── Cross-module struct literal and field access ───
//

#[test]
fn cross_module_struct_construction_and_field_access() {
    let a10 = Address::from_str("0x10").unwrap();
    let shapes = utils::compile(
        r#"
            module shapes;

            struct Point { x: u64, y: u64 }

            fn get_x(p: Point): u64 { return p.x; }
        "#,
    );

    let user = utils::compile_with_deps(
        &format!(
            r#"
                module user;

                use shapes@{};

                fn make_and_read(): u64 {{
                    let p = shapes::Point {{ x: 5, y: 9 }};
                    return shapes::get_x(p);
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
// ─── Same struct name in two different modules ───
//

#[test]
fn same_struct_name_in_different_modules_are_distinct() {
    let aa0 = Address::from_str("0xA0").unwrap();
    let ab0 = Address::from_str("0xB0").unwrap();

    let mod_a = utils::compile(
        r#"
            module mod_a;

            struct Token { amount: u64 }

            fn get_amount(t: Token): u64 { return t.amount; }
        "#,
    );
    let mod_b = utils::compile(
        r#"
            module mod_b;

            struct Token { points: u64 }

            fn get_points(t: Token): u64 { return t.points; }
        "#,
    );

    let main = utils::compile_with_deps(
        &format!(
            r#"
                module main;

                use mod_a@{};
                use mod_b@{};

                fn run(): u64 {{
                    let ta = mod_a::Token {{ amount: 100 }};
                    let tb = mod_b::Token {{ points: 42 }};
                    return mod_a::get_amount(ta) + mod_b::get_points(tb);
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
            module lib;

            fn version(): u64 { return 1; }
        "#,
    );
    let lib_v2 = utils::compile(
        r#"
            module lib;

            fn version(): u64 { return 2; }
        "#,
    );

    // The caller was compiled against lib at address 0x01.
    let caller_v1 = utils::compile_with_deps(
        &format!(
            r#"
                module caller;

                use lib@{};

                fn get(): u64 {{ return lib::version(); }}
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
            module math;

            fn square(x: u64): u64 { return x * x; }
            fn sum_of_squares(a: u64, b: u64): u64 { return square(a) + square(b); }
        "#,
    );

    let caller = utils::compile_with_deps(
        &format!(
            r#"
                module caller;

                use math@{};

                fn run(a: u64, b: u64): u64 {{ return math::sum_of_squares(a, b); }}
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
            module geometry;

            struct Point { x: u64, y: u64 }
        "#,
    );

    let shapes = utils::compile_with_deps(
        &format!(
            r#"
                module shapes;

                use geometry@{};

                struct Line {{ a: geometry::Point, b: geometry::Point }}

                fn x_distance(line: Line): u64 {{
                    return line.b.x - line.a.x;
                }}
            "#,
            a30
        ),
        &[(a30, &geometry)],
    );

    let point = |x: u64, y: u64| Value::Struct {
        type_name: "Point".to_string(),
        fields: vec![
            ("x".to_string(), Value::U64(x)),
            ("y".to_string(), Value::U64(y)),
        ],
    };
    let line = Value::Struct {
        type_name: "Line".to_string(),
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
// ─── Chained calls: caller → dep A → dep B ───
//

#[test]
fn chained_cross_module_calls() {
    let a60 = Address::from_str("0x60").unwrap();
    let base = utils::compile(
        r#"
            module base;

            fn one(): u64 { return 1; }
        "#,
    );

    let a61 = Address::from_str("0x61").unwrap();
    let mid = utils::compile_with_deps(
        &format!(
            r#"
                module mid;

                use base@{};

                fn two(): u64 {{ return base::one() + base::one(); }}
            "#,
            a60
        ),
        &[(a60, &base)],
    );

    let top = utils::compile_with_deps(
        &format!(
            r#"
                module top;

                use mid@{};

                fn four(): u64 {{ return mid::two() + mid::two(); }}
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
