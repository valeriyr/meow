mod utils;

use meow_vm_compiler::error::CompilerError;

#[test]
fn struct_field_can_be_string_type() {
    let src = r#"
        module test;

        struct Msg { text: string }

        fn make(text: string): Msg { return Msg { text: text }; }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_field_can_be_another_struct_type() {
    let src = r#"
        module test;

        struct Inner { x: u64 }
        struct Outer { inner: Inner, y: bool }

        fn noop() {}
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_field_unknown_type_rejected() {
    let src = r#"
        module test;

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
        module test;

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
        module test;

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
fn struct_field_cannot_be_an_object_type() {
    let src = r#"
        module test;

        object Token { id: address, amount: u64 }
        struct Wrapper { tok: Token }

        fn make(id: address, amount: u64) {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("which is an object")
    ));
}
