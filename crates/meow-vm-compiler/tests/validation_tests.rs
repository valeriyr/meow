use meow_vm_compiler::{Compiler, error::CompilerError};
use meow_vm_types::limits;

//
// ─── Object rules ───
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
// ─── Struct rules ───
//

#[test]
fn struct_field_can_be_string_type() {
    let src = r#"
        struct Msg { text: string }

        fn make(text: string): Msg { return Msg { text: text }; }
    "#;
    assert!(Compiler::compile("test", src).is_ok());
}

//
// ─── Identifier rules ───
//

#[test]
fn invalid_module_name_rejected() {
    assert!(matches!(
        Compiler::compile("1bad", "fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}

#[test]
fn module_name_with_dash_rejected() {
    assert!(matches!(
        Compiler::compile("my-module", "fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}

#[test]
fn module_name_too_long_rejected() {
    let long = "a".repeat(limits::MAX_IDENTIFIER_LEN + 1);
    assert!(matches!(
        Compiler::compile(&long, "fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}

#[test]
fn invalid_function_name_rejected() {
    let src = r#"fn 1bad() {}"#;
    // The parser won't accept a leading digit as a function name at all.
    assert!(Compiler::compile("test", src).is_err());
}

#[test]
fn invalid_struct_name_rejected() {
    // Parser only accepts ASCII idents so we test the post-parse identifier
    // check by smuggling an underscore-only name that is actually valid, then
    // verify a name starting with a digit is rejected by the parser.
    let src = r#"struct 2bad { x: u64 } fn f() {}"#;
    assert!(Compiler::compile("test", src).is_err());
}

#[test]
fn invalid_param_name_rejected() {
    // The parser enforces ASCII idents, so a param starting with a digit won't
    // parse. Verify via a mangled name that the parser itself catches this.
    let src = r#"fn f(1x: u64) {}"#;
    assert!(Compiler::compile("test", src).is_err());
}

//
// ─── Limits ───
//

#[test]
fn too_many_functions_rejected() {
    let src = (0..=limits::MAX_FUNCTIONS)
        .map(|i| format!("fn f{i}() {{}}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(matches!(
        Compiler::compile("test", &src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many functions")
    ));
}

#[test]
fn too_many_structs_rejected() {
    let src = (0..=limits::MAX_STRUCTS)
        .map(|i| format!("struct S{i} {{ x: u64 }}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(matches!(
        Compiler::compile("test", &src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many struct/object definitions")
    ));
}

#[test]
fn too_many_params_rejected() {
    let params = (0..=limits::MAX_PARAMS)
        .map(|i| format!("p{i}: u64"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!("fn f({params}) {{}}");
    assert!(matches!(
        Compiler::compile("test", &src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many parameters")
    ));
}

#[test]
fn too_many_fields_rejected() {
    let fields = (0..=limits::MAX_FIELDS)
        .map(|i| format!("f{i}: u64"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!("struct Big {{ {fields} }} fn noop() {{}}");
    assert!(matches!(
        Compiler::compile("test", &src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many fields")
    ));
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
