use meow_vm_compiler::{Compiler, Result, error::CompilerError};
use meow_vm_types::{config::CompilerConfig, module::Module};

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
        compile("test", src).unwrap_err(),
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
        compile("test", src).unwrap_err(),
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
        compile("test", src).unwrap_err(),
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
    assert!(compile("test", src).is_ok());
}

//
// ─── Identifier rules ───
//

#[test]
fn invalid_module_name_rejected() {
    assert!(matches!(
        compile("1bad", "fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}

#[test]
fn module_name_with_dash_rejected() {
    assert!(matches!(
        compile("my-module", "fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}

#[test]
fn module_name_too_long_rejected() {
    let config = CompilerConfig::default();
    let long = "a".repeat(config.max_identifier_len() + 1);
    assert!(matches!(
        compile(&long, "fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}

#[test]
fn invalid_function_name_rejected() {
    // The parser won't accept a leading digit as a function name at all.
    assert!(matches!(
        compile("test", "fn 1bad() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("expected identifier")
    ));
}

#[test]
fn invalid_struct_name_rejected() {
    // The parser won't accept a leading digit as a struct name at all.
    assert!(matches!(
        compile("test", "struct 2bad { x: u64 } fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("expected identifier")
    ));
}

#[test]
fn invalid_param_name_rejected() {
    // The parser won't accept a leading digit as a parameter name at all.
    assert!(matches!(
        compile("test", "fn f(1x: u64) {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("expected identifier")
    ));
}

//
// ─── Limits ───
//

#[test]
fn too_many_functions_rejected() {
    let config = CompilerConfig::default();
    let src = (0..=config.max_functions())
        .map(|i| format!("fn f{i}() {{}}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(matches!(
        Compiler::compile("test", &src, config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many functions")
    ));
}

#[test]
fn too_many_structs_rejected() {
    let config = CompilerConfig::default();
    let src = (0..=config.max_structs())
        .map(|i| format!("struct S{i} {{ x: u64 }}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(matches!(
        Compiler::compile("test", &src, config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many struct/object definitions")
    ));
}

#[test]
fn too_many_params_rejected() {
    let config = CompilerConfig::default();
    let params = (0..=config.max_params())
        .map(|i| format!("p{i}: u64"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!("fn f({params}) {{}}");
    assert!(matches!(
        Compiler::compile("test", &src, config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many parameters")
    ));
}

#[test]
fn too_many_fields_rejected() {
    let config = CompilerConfig::default();
    let fields = (0..=config.max_fields())
        .map(|i| format!("f{i}: u64"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!("struct Big {{ {fields} }} fn noop() {{}}");
    assert!(matches!(
        Compiler::compile("test", &src, config).unwrap_err(),
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
        compile("test", src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("non-primitive type")
    ));
}

//
// ─── Utility functions ───
//

fn compile(module_name: &str, source: &str) -> Result<Module> {
    Compiler::compile(module_name, source, CompilerConfig::default())
}
