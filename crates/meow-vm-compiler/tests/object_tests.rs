mod utils;

use meow_vm_compiler::error::CompilerError;

//
// ─── Object field types ───
//

#[test]
fn object_field_can_be_string_type() {
    utils::compile(
        r#"
            mod test;
            object Msg { id: address, text: string }
            fn noop() {}
        "#,
    )
    .expect("string field in object must be accepted");
}

#[test]
fn object_field_can_be_struct_type() {
    utils::compile(
        r#"
            mod test;
            struct Meta { created_at: u64 }
            object Coin { id: address, meta: Meta }
            fn noop() {}
        "#,
    )
    .expect("struct field in object must be accepted");
}

#[test]
fn object_field_unknown_type_rejected() {
    let err = utils::compile(
        r#"
            mod test;
            object Coin { id: address, meta: NonExistent }
            fn noop() {}
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("unknown struct 'NonExistent'")),
        "expected unknown-type error, got: {err:?}"
    );
}

#[test]
fn object_field_cannot_be_another_object() {
    let err = utils::compile(
        r#"
            mod test;
            object Inner { id: address, value: u64 }
            object Outer { id: address, inner: Inner }
            fn noop() {}
        "#,
    )
    .unwrap_err();
    assert!(
        matches!(&err, CompilerError::Message(msg) if msg.contains("which is an object")),
        "expected object-as-field error, got: {err:?}"
    );
}

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
fn object_can_be_returned_from_function() {
    utils::compile(
        r#"
            mod test;

            object Coin { id: address, balance: u64 }

            fn make(balance: u64) -> Coin {
                Coin { id: meow_vm_fresh_id(), balance: balance }
            }
        "#,
    )
    .expect("object return must be allowed");
}

#[test]
fn object_can_be_returned_from_pub_fn() {
    utils::compile(
        r#"
            mod test;

            object Coin { id: address, balance: u64 }

            pub fn mint(balance: u64) -> Coin {
                Coin { id: meow_vm_fresh_id(), balance: balance }
            }
        "#,
    )
    .expect("pub fn object return must be allowed");
}

#[test]
fn object_can_be_returned_in_tuple() {
    utils::compile(
        r#"
            mod test;

            object Coin { id: address, balance: u64 }

            fn split(c: Coin) -> (Coin, Coin) {
                let half = c.balance / 2;
                c.balance = half;
                let c2 = Coin { id: meow_vm_fresh_id(), balance: half };
                (c, c2)
            }
        "#,
    )
    .expect("object in tuple return must be allowed");
}
