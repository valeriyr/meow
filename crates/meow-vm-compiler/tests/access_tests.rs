mod utils;

use std::str::FromStr;

use meow_vm_compiler::{Result, error::CompilerError};
use meow_vm_types::{address::Address, module::Module};

//
// ─── Cross-module function visibility ───
//

#[test]
fn pub_fn_call_from_other_module_accepted() {
    with_dep(
        r#"
            mod lib;

            pub fn exposed() -> u64 { 42 }
        "#,
        r#"
            mod caller;

            use lib@0xFD;

            fn run() -> u64 { lib::exposed() }
        "#,
    )
    .expect("pub fn must be callable cross-module");
}

#[test]
fn private_fn_call_from_other_module_rejected() {
    let err = with_dep(
        r#"
            mod lib;

            fn secret() -> u64 { 42 }
        "#,
        r#"
            mod caller;

            use lib@0xFD;

            fn run() -> u64 { lib::secret() }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("function 'lib::secret' is private")),
        "expected private-function error, got: {err:?}"
    );
}

//
// ─── Cross-module struct construction ───
//

#[test]
fn same_module_struct_construction_accepted() {
    utils::compile(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make(x: u64, y: u64) -> Point { Point { x: x, y: y } }
        "#,
    )
    .expect("same-module construction must be accepted");
}

#[test]
fn cross_module_struct_construction_rejected() {
    let err = with_dep(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }
        "#,
        r#"
            mod user;

            use shapes@0xFD;

            fn bad() -> shapes::Point { shapes::Point { x: 1, y: 2 } }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("cannot construct 'shapes::Point' outside its declaring module")),
        "expected cross-module construction error, got: {err:?}"
    );
}

//
// ─── Private struct is not visible cross-module ───
//

#[test]
fn private_struct_not_usable_as_field_type_cross_module() {
    let err = with_dep(
        r#"
            mod lib;

            struct Hidden { x: u64 }
        "#,
        r#"
            mod user;

            use lib@0xFD;

            struct Wrapper { inner: lib::Hidden }

            fn noop() {}
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("field 'inner' references unknown struct 'lib::Hidden'")),
        "expected unknown-struct error for the field type, got: {err:?}"
    );
}

#[test]
fn private_struct_not_usable_as_param_type_cross_module() {
    let err = with_dep(
        r#"
            mod lib;

            struct Hidden { x: u64 }
        "#,
        r#"
            mod user;

            use lib@0xFD;

            fn take(h: lib::Hidden) {}
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("parameter 'h' references unknown struct 'lib::Hidden'")),
        "expected unknown-struct error for the parameter type, got: {err:?}"
    );
}

#[test]
fn private_struct_not_usable_as_return_type_cross_module() {
    let err = with_dep(
        r#"
            mod lib;

            struct Hidden { x: u64 }
        "#,
        r#"
            mod user;

            use lib@0xFD;

            fn make() -> lib::Hidden {}
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("return type references unknown struct 'lib::Hidden'")),
        "expected unknown-struct error for the return type, got: {err:?}"
    );
}

//
// ─── Cross-module field access ───
//
// Fields are always private — there is no `pub field` syntax.
// Use a public getter function to expose field values cross-module.
//

#[test]
fn getter_function_exposes_field_cross_module() {
    with_dep(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make_point(x: u64, y: u64) -> Point { Point { x: x, y: y } }
            pub fn get_x(p: Point) -> (Point, u64) {
                let val = p.x;
                (p, val)
            }
        "#,
        r#"
            mod user;

            use shapes@0xFD;

            fn read_x(p: shapes::Point) -> (shapes::Point, u64) {
                let (p, val) = shapes::get_x(p);
                (p, val)
            }
        "#,
    )
    .expect("getter function pattern must be accepted cross-module");
}

#[test]
fn field_read_from_other_module_rejected() {
    let err = with_dep(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make(x: u64, y: u64) -> Point { Point { x: x, y: y } }
        "#,
        r#"
            mod user;

            use shapes@0xFD;

            fn read_x() -> u64 {
                let p = shapes::make(1, 2);
                p.x
            }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("are private — use a public getter")),
        "expected private-field error, got: {err:?}"
    );
}

//
// ─── Cross-module field write ───
//

#[test]
fn field_write_in_same_module_accepted() {
    utils::compile(
        r#"
            mod shapes;

            struct Point { x: u64, y: u64 }

            fn set_x(p: Point, v: u64) -> Point {
                p.x = v;
                p
            }
        "#,
    )
    .expect("same-module field write must be accepted");
}

#[test]
fn field_write_from_other_module_rejected() {
    let err = with_dep(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make(x: u64, y: u64) -> Point { Point { x: x, y: y } }
        "#,
        r#"
            mod user;

            use shapes@0xFD;

            fn mutate() {
                let p = shapes::make(1, 2);
                p.x = 99;
            }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg)
            if msg.contains("field 'x' of 'shapes::Point' cannot be written from outside")),
        "expected cross-module write error, got: {err:?}"
    );
}

//
// ─── Helper ───
//

fn with_dep(dep_src: &str, src: &str) -> Result<Module> {
    let dep = utils::compile(dep_src).expect("dep must compile");
    let addr = Address::from_str("0xFD").unwrap();

    utils::compile_with_deps(src, &[(addr, &dep)])
}
