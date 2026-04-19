mod utils;

use meow_vm_compiler::error::CompilerError;

#[test]
fn struct_field_can_be_string_type() {
    let src = r#"
        mod test;

        struct Msg { text: string }

        fn make(text: string) -> Msg { Msg { text: text } }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_field_can_be_another_struct_type() {
    let src = r#"
        mod test;

        struct Inner { x: u64 }
        struct Outer { inner: Inner, y: bool }

        fn noop() {}
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_field_unknown_type_rejected() {
    let src = r#"
        mod test;

        struct Wrapper { item: NonExistent }

        fn noop() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("unknown struct 'NonExistent'")
    ));
}

#[test]
fn struct_cycle_direct_rejected() {
    let src = r#"
        mod test;

        struct A { b: B }
        struct B { a: A }

        fn noop() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cycle")
    ));
}

#[test]
fn struct_cycle_indirect_rejected() {
    // A → B → C → A
    let src = r#"
        mod test;

        struct A { b: B }
        struct B { c: C }
        struct C { a: A }

        fn noop() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cycle")
    ));
}

#[test]
fn struct_id_field_is_arbitrary() {
    // Unlike objects, structs have no special `id` field rule.
    // A struct may have a field named `id` of any type, assigned to any value.
    let src = r#"
        mod test;

        struct Receipt { id: u64, amount: u64 }

        pub fn make(id: u64, amount: u64) -> Receipt { Receipt { id: id, amount: amount } }
        pub fn get_id(r: Receipt) -> u64 { r.id }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_destructuring_compiles() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn sum(p: Point) -> u64 {
            let Point { x, y } = p;
            x + y
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_destructuring_unknown_field_rejected() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn bad(p: Point) -> u64 {
            let Point { x, z } = p;
            x + z
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("unknown field 'z'")
    ));
}

#[test]
fn struct_destructuring_missing_field_rejected() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn bad(p: Point) -> u64 {
            let Point { x } = p;
            x
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("missing binding for field 'y'") && msg.contains("..")
    ));
}

#[test]
fn struct_destructuring_cannot_discard_struct_field() {
    let src = r#"
        mod test;

        struct Inner { value: u64 }
        struct Outer { name: u64, data: Inner }

        pub fn bad(o: Outer) -> u64 {
            let Outer { name, .. } = o;
            name
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot discard") && msg.contains("'data'")
    ));
}
