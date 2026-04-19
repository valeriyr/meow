mod utils;

use std::str::FromStr;

use meow_vm_compiler::{Compiler, Result, error::CompilerError};
use meow_vm_types::{
    address::Address, config::CompilerConfig, module::Module, natives::NativeSig, types::Type,
};

//
// ─── Struct literals ───
//

#[test]
fn correct_struct_literal_compiles() {
    utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn make(x: u64, y: u64) -> Point {
                Point { x: x, y: y }
            }
        "#,
    )
    .expect("correct struct literal must compile");
}

#[test]
fn wrong_field_type_primitive_mismatch_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn bad(y: u64) -> Point {
                Point { x: true, y: y }
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected u64") && msg.contains("found bool")),
        "expected type mismatch error, got: {err:?}"
    );
}

#[test]
fn struct_literal_missing_field_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn bad() -> Point {
                Point { x: 1 }
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("missing field") && msg.contains("y")),
        "expected missing field error, got: {err:?}"
    );
}

#[test]
fn struct_literal_unknown_field_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn bad() -> Point {
                Point { x: 1, y: 2, z: 3 }
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("unknown field") && msg.contains("z")),
        "expected unknown field error, got: {err:?}"
    );
}

//
// ─── Expressions ───
//

#[test]
fn undefined_variable_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad() {
                let y = x;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("undefined variable")),
        "expected undefined variable error, got: {err:?}"
    );
}

#[test]
fn arithmetic_on_bool_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad(x: bool) -> u64 {
                x + 1
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected u64") && msg.contains("found bool")),
        "expected arithmetic type error, got: {err:?}"
    );
}

#[test]
fn equality_on_structs_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn bad(a: Point, b: Point) -> bool {
                a == b
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("cannot be compared")),
        "expected struct equality error, got: {err:?}"
    );
}

#[test]
fn unary_not_on_non_bool_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad(x: u64) -> bool {
                return !x;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected bool") && msg.contains("found u64")),
        "expected unary not type error, got: {err:?}"
    );
}

#[test]
fn field_access_on_non_struct_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad(x: u64) -> u64 {
                return x.field;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("requires a struct") && msg.contains("found u64")),
        "expected field access on non-struct error, got: {err:?}"
    );
}

#[test]
fn field_access_unknown_field_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn bad(p: Point) -> u64 {
                return p.z;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("no field") && msg.contains("z")),
        "expected unknown field error, got: {err:?}"
    );
}

//
// ─── Function calls ───
//

#[test]
fn correct_function_argument_types_compile() {
    utils::compile(
        r#"
            mod test;

            fn add(a: u64, b: u64) -> u64 { a + b }

            fn run() -> u64 {
                add(1, 2)
            }
        "#,
    )
    .expect("matching argument types must compile");
}

#[test]
fn wrong_function_argument_type_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn add(a: u64, b: u64) -> u64 { a + b }

            fn bad() -> u64 {
                add(1, true)
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected u64") && msg.contains("found bool")),
        "expected argument type error, got: {err:?}"
    );
}

//
// ─── Native functions ───
//

#[test]
fn wrong_field_type_native_return_mismatch_rejected() {
    let new_id_sig = NativeSig {
        name: "meow_vm_new_id".to_string(),
        params: vec![],
        return_type: Some(Type::Struct("meow_object::Id".to_string())),
    };
    with_dep_and_sigs(
        r#"
            mod meow_object;
            pub struct Id { inner: address }
        "#,
        r#"
            mod test;
            use meow_object@0x01;

            struct Bad { id: address }

            fn make() -> Bad {
                Bad { id: meow_vm_new_id() }
            }
        "#,
        &[new_id_sig],
    )
    .expect_err("meow_object::Id cannot be used where address is expected");
}

#[test]
fn correct_native_in_struct_field_compiles() {
    let new_id_sig = NativeSig {
        name: "meow_vm_new_id".to_string(),
        params: vec![],
        return_type: Some(Type::Struct("meow_object::Id".to_string())),
    };
    with_dep_and_sigs(
        r#"
            mod meow_object;
            pub struct Id { inner: address }
        "#,
        r#"
            mod test;
            use meow_object@0x01;

            struct Good { id: meow_object::Id, value: u64 }

            fn make(v: u64) -> Good {
                Good { id: meow_vm_new_id(), value: v }
            }
        "#,
        &[new_id_sig],
    )
    .expect("meow_object::Id field initialised with meow_vm_new_id() must compile");
}

#[test]
fn meow_vm_abort_correct_types_compile() {
    utils::compile(
        r#"
            mod test;

            fn check(x: u64) {
                meow_vm_abort(x > 10, 1, "too big");
            }
        "#,
    )
    .expect("meow_vm_abort with correct types must compile");
}

#[test]
fn meow_vm_abort_wrong_condition_type_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn check(x: u64) {
                meow_vm_abort(x, 1, "too big");
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected bool") && msg.contains("found u64")),
        "expected abort condition type error, got: {err:?}"
    );
}

#[test]
fn native_wrong_arg_count_rejected() {
    let sig = NativeSig {
        name: "my_fn".to_string(),
        params: vec![Some(Type::U64)],
        return_type: None,
    };
    let err = Compiler::compile(
        r#"mod test; fn bad() { my_fn(1, 2); }"#,
        &[],
        &[sig],
        CompilerConfig::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected 1 argument(s)") && msg.contains("found 2")),
        "expected wrong arg count error, got: {err:?}"
    );
}

#[test]
fn native_any_struct_param_with_primitive_rejected() {
    let sig = NativeSig {
        name: "my_fn".to_string(),
        params: vec![None],
        return_type: None,
    };
    let err = Compiler::compile(
        r#"mod test; fn bad() { my_fn(42); }"#,
        &[],
        &[sig],
        CompilerConfig::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected a struct") && msg.contains("found u64")),
        "expected any-struct param type error, got: {err:?}"
    );
}

//
// ─── Statements ───
//

#[test]
fn void_result_in_let_binding_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn g() {}

            fn bad() {
                let x = g();
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("void")),
        "expected void-in-let error, got: {err:?}"
    );
}

#[test]
fn wrong_reassignment_type_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad() -> u64 {
                let x = 1;
                x = true;
                x
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected u64") && msg.contains("found bool")),
        "expected reassignment type error, got: {err:?}"
    );
}

#[test]
fn reassign_to_undefined_variable_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad() {
                x = 42;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("assignment to undefined variable")),
        "expected undefined variable error, got: {err:?}"
    );
}

#[test]
fn wrong_return_type_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn f() -> u64 {
                return true;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected u64") && msg.contains("found bool")),
        "expected return type error, got: {err:?}"
    );
}

#[test]
fn return_without_value_in_typed_fn_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn f() -> u64 {
                return;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("return without value")),
        "expected return without value error, got: {err:?}"
    );
}

#[test]
fn field_assign_correct_type_compiles() {
    utils::compile(
        r#"
            mod test;

            struct Counter { value: u64 }

            fn inc(c: Counter) -> Counter {
                c.value = c.value + 1;
                c
            }
        "#,
    )
    .expect("field assign with correct type must compile");
}

#[test]
fn field_assign_wrong_type_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            struct Counter { value: u64 }

            fn bad(c: Counter) -> Counter {
                c.value = true;
                c
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected u64") && msg.contains("found bool")),
        "expected field assign type error, got: {err:?}"
    );
}

//
// ─── Control flow ───
//

#[test]
fn correct_if_condition_compiles() {
    utils::compile(
        r#"
            mod test;

            fn check(x: u64) -> bool {
                let result = false;
                if x > 0 {
                    result = true;
                }
                result
            }
        "#,
    )
    .expect("boolean if condition must compile");
}

#[test]
fn wrong_if_condition_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad(x: u64) {
                if 42 {
                    let y = 1;
                }
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected bool") && msg.contains("found u64")),
        "expected if-condition type error, got: {err:?}"
    );
}

#[test]
fn if_else_compiles() {
    utils::compile(
        r#"
            mod test;

            fn sign(x: u64) -> bool {
                let result = false;
                if x > 0 {
                    result = true;
                } else {
                    result = false;
                }
                result
            }
        "#,
    )
    .expect("if-else with matching branch types must compile");
}

//
// ─── Destructuring ───
//

#[test]
fn let_struct_compiles() {
    utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn sum(p: Point) -> u64 {
                let Point { x, y } = p;
                x + y
            }
        "#,
    )
    .expect("struct destructuring with valid fields must compile");
}

#[test]
fn let_struct_unknown_field_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            struct Point { x: u64, y: u64 }

            fn bad(p: Point) -> u64 {
                let Point { x, z } = p;
                x
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("unknown field") && msg.contains("z")),
        "expected unknown field error in struct destructuring, got: {err:?}"
    );
}

#[test]
fn let_tuple_compiles() {
    utils::compile(
        r#"
            mod test;

            fn swap(a: u64, b: u64) -> (u64, u64) { (b, a) }

            fn run() -> u64 {
                let (x, y) = swap(1, 2);
                x
            }
        "#,
    )
    .expect("tuple destructuring must compile");
}

#[test]
fn let_tuple_on_non_tuple_rejected() {
    let err = utils::compile(
        r#"
            mod test;

            fn bad() {
                let x = 42;
                let (a, b) = x;
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected a tuple") && msg.contains("found u64")),
        "expected tuple destructuring type error, got: {err:?}"
    );
}

//
// ─── Cross-module ───
//

#[test]
fn cross_module_arg_type_mismatch_rejected() {
    let err = with_dep(
        r#"
            mod math;
            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
        r#"
            mod user;
            use math@0x01;

            fn bad() -> u64 {
                math::add(1, true)
            }
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("expected u64") && msg.contains("found bool")),
        "expected cross-module arg type error, got: {err:?}"
    );
}

//
// ─── Helpers ───
//

fn with_dep(dep_src: &str, src: &str) -> Result<Module> {
    with_dep_and_sigs(dep_src, src, &[])
}

fn with_dep_and_sigs(dep_src: &str, src: &str, native_sigs: &[NativeSig]) -> Result<Module> {
    let dep = utils::compile(dep_src).expect("dep module must compile");
    let addr = Address::from_str("0x01").unwrap();
    Compiler::compile(src, &[(addr, &dep)], native_sigs, CompilerConfig::default())
}
