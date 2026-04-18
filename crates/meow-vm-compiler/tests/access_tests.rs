mod utils;

use std::str::FromStr;

use meow_vm_compiler::{Result, error::CompilerError};
use meow_vm_types::{address::Address, module::Module};

//
// ─── Private function call rejected ───
//

#[test]
fn private_fn_call_from_other_module_rejected() {
    let err = with_dep(
        r#"
            mod lib;
            fn secret() -> u64 { return 42; }
        "#,
        r#"
            mod caller;
            use lib@0x01;
            fn run() -> u64 { return lib::secret(); }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("private")),
        "expected private-function error, got: {err:?}"
    );
}

#[test]
fn pub_fn_call_from_other_module_accepted() {
    with_dep(
        r#"
            mod lib;
            pub fn exposed() -> u64 { return 42; }
        "#,
        r#"
            mod caller;
            use lib@0x01;
            fn run() -> u64 { return lib::exposed(); }
        "#,
    )
    .expect("pub fn must be callable cross-module");
}

//
// ─── Cross-module struct construction rejected ───
//

#[test]
fn cross_module_struct_construction_rejected() {
    let err = with_dep(
        r#"
            mod shapes;
            pub struct Point { pub x: u64, pub y: u64 }
        "#,
        r#"
            mod user;
            use shapes@0x01;
            fn bad() -> shapes::Point { return shapes::Point { x: 1, y: 2 }; }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("cannot construct")),
        "expected cross-module construction error, got: {err:?}"
    );
}

#[test]
fn same_module_struct_construction_accepted() {
    utils::compile(
        r#"
            mod shapes;
            pub struct Point { pub x: u64, pub y: u64 }
            pub fn make(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }
        "#,
    )
    .expect("same-module construction must be accepted");
}

//
// ─── Cross-module object construction rejected ───
//

#[test]
fn cross_module_object_construction_rejected() {
    let err = with_dep(
        r#"
            mod coin;
            pub object Coin { id: address, pub balance: u64 }
        "#,
        r#"
            mod bad;
            use coin@0x01;
            fn forge(id: address) {
                let c = coin::Coin { id: id, balance: 1000 };
            }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("cannot construct")),
        "expected cross-module construction error, got: {err:?}"
    );
}

//
// ─── Private struct is not visible cross-module ───
//

#[test]
fn private_struct_not_usable_as_field_type_cross_module() {
    // A private struct from a dep is filtered from dep_structs — using it as a
    // struct field type in a dependent module is rejected as an unknown type.
    let err = with_dep(
        r#"
            mod lib;
            struct Hidden { x: u64 }
        "#,
        r#"
            mod user;
            use lib@0x01;
            struct Wrapper { inner: lib::Hidden }
            fn noop() {}
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("unknown")),
        "expected unknown-type error for private struct field, got: {err:?}"
    );
}

//
// ─── Private field read rejected ───
//

#[test]
fn private_field_read_from_other_module_rejected() {
    let err = with_dep(
        r#"
            mod shapes;
            pub struct Point { pub x: u64, y: u64 }
            pub fn make(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }
        "#,
        r#"
            mod user;
            use shapes@0x01;
            fn read_y() -> u64 {
                let p = shapes::make(1, 2);
                return p.y;
            }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("private")),
        "expected private-field-read error, got: {err:?}"
    );
}

#[test]
fn pub_field_read_from_other_module_accepted() {
    with_dep(
        r#"
            mod shapes;
            pub struct Point { pub x: u64, y: u64 }
            pub fn make(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }
        "#,
        r#"
            mod user;
            use shapes@0x01;
            fn read_x() -> u64 {
                let p = shapes::make(3, 7);
                return p.x;
            }
        "#,
    )
    .expect("pub field must be readable cross-module");
}

#[test]
fn private_field_read_on_object_from_other_module_rejected() {
    let err = with_dep(
        r#"
            mod coin;
            pub object Coin { id: address, balance: u64 }
        "#,
        r#"
            mod user;
            use coin@0x01;
            fn read_balance(c: coin::Coin) -> u64 {
                return c.balance;
            }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("private")),
        "expected private-field-read error on object, got: {err:?}"
    );
}

#[test]
fn pub_field_read_on_object_from_other_module_accepted() {
    with_dep(
        r#"
            mod coin;
            pub object Coin { id: address, pub balance: u64 }
        "#,
        r#"
            mod user;
            use coin@0x01;
            fn read_balance(c: coin::Coin) -> u64 {
                return c.balance;
            }
        "#,
    )
    .expect("pub field on object must be readable cross-module");
}

//
// ─── pub field in private struct/object rejected ───
//

#[test]
fn pub_field_in_private_struct_rejected() {
    let err = utils::compile(
        r#"
            mod shapes;
            struct Hidden { pub x: u64 }
            fn noop() {}
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("pub") && msg.contains("private")),
        "expected pub-field-in-private-struct error, got: {err:?}"
    );
}

#[test]
fn pub_field_in_private_object_rejected() {
    let err = utils::compile(
        r#"
            mod coin;
            object Coin { id: address, pub balance: u64 }
            fn noop() {}
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("pub") && msg.contains("private")),
        "expected pub-field-in-private-object error, got: {err:?}"
    );
}

//
// ─── Cross-module field write rejected (always) ───
//

#[test]
fn pub_field_write_from_other_module_rejected() {
    // Even `pub` fields cannot be written from outside the declaring module.
    let err = with_dep(
        r#"
            mod shapes;
            pub struct Point { pub x: u64, pub y: u64 }
            pub fn make(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }
        "#,
        r#"
            mod user;
            use shapes@0x01;
            fn mutate() {
                let p = shapes::make(1, 2);
                p.x = 99;
            }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("cannot be written")),
        "expected cross-module write error, got: {err:?}"
    );
}

#[test]
fn field_write_in_same_module_accepted() {
    utils::compile(
        r#"
            mod shapes;
            struct Point { x: u64, y: u64 }
            fn set_x(p: Point, v: u64) -> Point {
                p.x = v;
                return p;
            }
        "#,
    )
    .expect("same-module field write must be accepted");
}

//
// ─── Object `id` field is immutable everywhere ───
//

#[test]
fn object_id_write_in_same_module_rejected() {
    let err = utils::compile(
        r#"
            mod coin;
            object Coin { id: address, balance: u64 }
            fn bad(c: Coin, new_id: address) {
                c.id = new_id;
            }
        "#,
    )
    .unwrap_err();

    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("immutable")),
        "expected id-immutability error, got: {err:?}"
    );
}

//
// ─── Comments are stripped before parsing ───
//

#[test]
fn line_comment_inside_function_body() {
    utils::compile(
        r#"
            mod comments;
            // This is a top-level comment
            pub fn add(a: u64, b: u64) -> u64 {
                // add the two values and return
                return a + b; // inline comment
            }
        "#,
    )
    .expect("comments must be stripped before parsing");
}

#[test]
fn comment_with_slashes_in_string_not_stripped() {
    utils::compile(
        r#"
            mod comments;
            pub fn url() -> string { return "https://example.com"; }
        "#,
    )
    .expect("slashes inside string literals must not be treated as comments");
}

//
// ─── Helper ───
//

/// Compile a dep module, then compile `src` with that dep at address `0x01`.
fn with_dep(dep_src: &str, src: &str) -> Result<Module> {
    let dep = utils::compile(dep_src).expect("dep must compile");
    let addr = Address::from_str("0x01").unwrap();
    utils::compile_with_deps(src, &[(addr, &dep)])
}
