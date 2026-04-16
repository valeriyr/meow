mod utils;

use meow_vm_compiler::error::CompilerError;

#[test]
fn empty_source_rejected() {
    assert!(matches!(
        utils::compile("").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module NAME;")
    ));
}

#[test]
fn missing_module_decl_rejected() {
    assert!(matches!(
        utils::compile("fn f() {}").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module NAME;")
    ));
}

#[test]
fn duplicate_module_decl_rejected() {
    let src = r#"
        module foo;
        module bar;

        fn f() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("duplicate 'module NAME;'")
    ));
}

#[test]
fn module_decl_must_be_first() {
    // A `use` before `module` is a parse error (module decl not first parseable item).
    let src = r#"
        fn f() {}
        module late;
    "#;
    let err = utils::compile(src).unwrap_err();
    assert!(
        format!("{err}").contains("module NAME;"),
        "unexpected error: {err}"
    );
}
