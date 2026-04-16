use std::str::FromStr;

use meow_vm_compiler::{Compiler, error::CompilerError};
use meow_vm_types::{address::Address, config::CompilerConfig};

#[test]
fn too_many_functions_rejected() {
    let config = CompilerConfig::default();
    let fns = (0..=config.max_functions())
        .map(|i| format!("fn f{i}() {{}}"))
        .collect::<Vec<_>>()
        .join("\n");
    let src = format!(
        r#"
            module test;
            {fns}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many functions")
    ));
}

#[test]
fn too_many_structs_rejected() {
    let config = CompilerConfig::default();
    let structs = (0..=config.max_structs())
        .map(|i| format!("struct S{i} {{ x: u64 }}"))
        .collect::<Vec<_>>()
        .join("\n");
    let src = format!(
        r#"
            module test;
            {structs}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], config).unwrap_err(),
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
    let src = format!(
        r#"
            module test;
            fn f({params}) {{}}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], config).unwrap_err(),
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
    let src = format!(
        r#"
            module test;
            struct Big {{ {fields} }} fn noop() {{}}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many fields")
    ));
}

#[test]
fn too_many_imports_rejected() {
    let config = CompilerConfig::default();
    // Build (max_imports + 1) unique dep modules and use declarations.
    let count = config.max_imports() + 1;

    let mut dep_modules = Vec::new();
    let mut use_lines = Vec::new();
    for i in 0..count {
        let addr = Address::from_str(&format!("0x{:064x}", i + 1)).unwrap();
        let module = Compiler::compile(
            &format!(
                r#"
                module dep{i};
                fn noop() {{}}
            "#
            ),
            &[],
            CompilerConfig::default(),
        )
        .unwrap();
        dep_modules.push((addr, module));
        use_lines.push(format!("use dep{i}@0x{:064x};", i + 1));
    }

    let src = format!(
        r#"
            module main;
            {}
            fn noop() {{}}
        "#,
        use_lines.join("\n")
    );

    let dep_refs = dep_modules.iter().map(|(a, m)| (*a, m)).collect::<Vec<_>>();

    assert!(matches!(
        Compiler::compile(&src, &dep_refs, config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many use declarations")
    ));
}

#[test]
fn too_many_dep_modules_rejected() {
    // main → b → c: full transitive closure has 2 deps. Limit = 1 → compile must fail.
    let cfg = CompilerConfig::default();
    let b_addr = Address::from_str("0x02").unwrap();
    let c_addr = Address::from_str("0x03").unwrap();

    let c_module = Compiler::compile(
        r#"
            module c;
            fn get(): u64 { return 1; }
        "#,
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = Compiler::compile(
        r#"
            module b;
            use c@0x03;
            fn get(): u64 { return c::get(); }
        "#,
        &[(c_addr, &c_module)],
        cfg,
    )
    .expect("b must compile");

    // Provide the full transitive closure [b, c] (len=2) with limit=1.
    let strict = CompilerConfig::default().with_max_dep_modules(1);
    assert!(matches!(
        Compiler::compile(
            r#"
                module main;
                use b@0x02;
                fn run(): u64 { return b::get(); }
            "#,
            &[(b_addr, &b_module), (c_addr, &c_module)],
            strict
        )
        .unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many dependency modules")
    ));
}

#[test]
fn dep_modules_at_limit_succeeds() {
    // Same chain, limit = 2 — [b, c] fits within the limit.
    let cfg = CompilerConfig::default();
    let b_addr = Address::from_str("0x02").unwrap();
    let c_addr = Address::from_str("0x03").unwrap();

    let c_module = Compiler::compile(
        r#"
            module c;
            fn get(): u64 { return 1; }
        "#,
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = Compiler::compile(
        r#"
            module b;
            use c@0x03;
            fn get(): u64 { return c::get(); }
        "#,
        &[(c_addr, &c_module)],
        cfg,
    )
    .expect("b must compile");

    let at_limit = CompilerConfig::default().with_max_dep_modules(2);
    assert!(
        Compiler::compile(
            r#"
                module main;
                use b@0x02;
                fn run(): u64 { return b::get(); }
            "#,
            &[(b_addr, &b_module), (c_addr, &c_module)],
            at_limit
        )
        .is_ok()
    );
}

#[test]
fn module_name_too_long_rejected() {
    let config = CompilerConfig::default();
    let long = "a".repeat(config.max_identifier_len() + 1);

    let src = format!(
        r#"
            module {long};
            fn f() {{}}
        "#
    );

    assert!(matches!(
        Compiler::compile(&src, &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}
