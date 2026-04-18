mod utils;

use meow_vm_compiler::error::CompilerError;

#[test]
fn object_first_field_must_be_id_address() {
    let src = r#"
        mod test;

        object BadObject { balance: u64, id: address }

        fn make(id: address, balance: u64) {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("first field must be 'id: address'")
    ));
}

#[test]
fn object_id_must_use_fresh_id() {
    let src = r#"
        mod test;

        object Token { id: address, amount: u64 }

        fn bad_mint(id: address, amount: u64) {
            let t = Token { id: id, amount: amount };
            meow_vm_transfer(t, id);
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("'id' field must be initialized with meow_vm_fresh_id()")
    ));
}

#[test]
fn object_cannot_be_returned_from_function() {
    let src = r#"
        mod test;

        object Coin { id: address, balance: u64 }

        fn make(id: address, balance: u64) -> Coin {
            return Coin { id: id, balance: balance };
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot return Object type")
    ));
}
