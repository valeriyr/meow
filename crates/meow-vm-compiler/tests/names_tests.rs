mod utils;

use meow_vm_compiler::{Compiler, error::CompilerError};
use meow_vm_types::{config::CompilerConfig, identifier::RESERVED_FUNCTION_NAMES};

//
// ─── Identifier rules ───
//

#[test]
fn invalid_module_name_rejected() {
    // `1bad` starts with a digit — rejected at parse time (not a valid ident).
    let src = r#"
        mod 1bad;
        fn f() {}
    "#;
    let err = utils::compile(src).unwrap_err();
    assert!(
        format!("{err}").contains("module name") || format!("{err}").contains("found '1'"),
        "unexpected error: {err}"
    );
}

#[test]
fn module_name_with_dash_rejected() {
    let src = r#"
        mod my-module;
        fn f() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name") | msg.contains("expected")
    ));
}

#[test]
fn invalid_function_name_rejected() {
    // The parser won't accept a leading digit as a function name at all.
    let src = r#"
            mod test;
            fn 1bad() {}
        "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("expected identifier")
    ));
}

#[test]
fn invalid_struct_name_rejected() {
    // The parser won't accept a leading digit as a struct name at all.
    let src = r#"
            mod test;
            struct 2bad { x: u64 } fn f() {}
        "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("expected identifier")
    ));
}

#[test]
fn invalid_param_name_rejected() {
    // The parser won't accept a leading digit as a parameter name at all.
    let src = r#"
        mod test;
        fn f(1x: u64) {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("expected identifier")
    ));
}

//
// ─── Reserved native names ───
//

#[test]
fn vm_level_reserved_names_are_rejected_with_default_config() {
    for name in RESERVED_FUNCTION_NAMES {
        let src = format!(
            r#"
                mod test;
                fn {name}() {{}}
            "#
        );
        assert!(
            matches!(
                utils::compile(&src).unwrap_err(),
                CompilerError::Message(msg) if msg.contains("reserved for a built-in native function")
            ),
            "defining a function named '{name}' must be rejected"
        );
    }
}

#[test]
fn config_supplied_reserved_names_are_rejected() {
    let extra = ["my_native_a", "my_native_b"];
    let config = CompilerConfig::default().with_reserved_function_names(&extra);

    for name in extra {
        let src = format!(
            r#"
                mod test;
                fn {name}() {{}}
            "#
        );
        assert!(
            matches!(
                Compiler::compile(&src, &[], &[], config.clone()).unwrap_err(),
                CompilerError::Message(msg) if msg.contains("reserved for a built-in native function")
            ),
            "config-reserved name '{name}' must be rejected"
        );
    }
}

#[test]
fn non_reserved_function_with_name_starting_meow_is_accepted() {
    let src = r#"
        mod test;
        fn meow_my_function() {}
    "#;
    assert!(utils::compile(src).is_ok());
}
