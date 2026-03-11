use meow_vm::compiler::{Compiler, error::CompilerError};

//
// Object rules.
//

#[test]
fn object_first_field_must_be_id_address() {
    let src = r#"
        object BadObject { balance: u64, id: address }

        fn make(id: address, balance: u64) {}
    "#;
    assert!(matches!(
        Compiler::compile("test", src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("first field must be 'id: address'")
    ));
}

#[test]
fn object_id_must_use_fresh_id() {
    let src = r#"
        object Token { id: address, amount: u64 }

        fn bad_mint(id: address, amount: u64) {
            let t = Token { id: id, amount: amount };
            meow_vm_transfer(t, id);
        }
    "#;
    assert!(matches!(
        Compiler::compile("test", src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("'id' field must be initialized with meow_vm_fresh_id()")
    ));
}

#[test]
fn object_cannot_be_returned_from_function() {
    let src = r#"
        object Coin { id: address, balance: u64 }

        fn make(id: address, balance: u64): Coin {
            return Coin { id: id, balance: balance };
        }
    "#;
    assert!(matches!(
        Compiler::compile("test", src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot return Object type")
    ));
}

//
// Struct rules.
//

#[test]
fn struct_field_can_be_string_type() {
    let src = r#"
        struct Msg { text: string }

        fn make(text: string): Msg { return Msg { text: text }; }
    "#;
    assert!(Compiler::compile("test", src).is_ok());
}

#[test]
fn struct_field_cannot_be_an_object_type() {
    let src = r#"
        object Token { id: address, amount: u64 }

        struct Wrapper { tok: Token }

        fn make(id: address, amount: u64) {}
    "#;
    assert!(matches!(
        Compiler::compile("test", src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("non-primitive type")
    ));
}
