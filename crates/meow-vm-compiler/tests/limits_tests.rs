use std::str::FromStr;

use meow_vm_compiler::{Compiler, error::CompilerError};
use meow_vm_types::{address::Address, config::CompilerConfig};

//
// ─── Module structure ───
//

#[test]
fn too_many_functions_rejected() {
    let config = CompilerConfig::default();
    let fns = (0..=config.max_functions())
        .map(|i| format!("fn f{i}() {{}}"))
        .collect::<Vec<_>>()
        .join("\n");
    let src = format!(
        r#"
            mod test;
            {fns}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], &[], config).unwrap_err(),
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
            mod test;
            {structs}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many struct definitions")
    ));
}

//
// ─── Imports and dependencies ───
//

#[test]
fn dep_modules_at_limit_succeeds() {
    // Same chain, limit = 2 — [b, c] fits within the limit.
    let cfg = CompilerConfig::default();
    let b_addr = Address::from_str("0xFB").unwrap();
    let c_addr = Address::from_str("0xFC").unwrap();

    let c_module = Compiler::compile(
        r#"
            mod c;

            pub fn get() -> u64 { 1 }
        "#,
        &[],
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = Compiler::compile(
        r#"
            mod b;

            use c@0xFC;

            pub fn get() -> u64 { c::get() }
        "#,
        &[(c_addr, &c_module)],
        &[],
        cfg,
    )
    .expect("b must compile");

    let at_limit = CompilerConfig::default().with_max_dep_modules(2);
    assert!(
        Compiler::compile(
            r#"
                mod main;

                use b@0xFB;

                fn run() -> u64 { b::get() }
            "#,
            &[(b_addr, &b_module), (c_addr, &c_module)],
            &[],
            at_limit
        )
        .is_ok()
    );
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
                mod dep{i};

                fn noop() {{}}
            "#
            ),
            &[],
            &[],
            CompilerConfig::default(),
        )
        .unwrap();
        dep_modules.push((addr, module));
        use_lines.push(format!("use dep{i}@0x{:064x};", i + 1));
    }

    let src = format!(
        r#"
            mod main;
            {}
            fn noop() {{}}
        "#,
        use_lines.join("\n")
    );

    let dep_refs = dep_modules.iter().map(|(a, m)| (*a, m)).collect::<Vec<_>>();

    assert!(matches!(
        Compiler::compile(&src, &dep_refs, &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many use declarations")
    ));
}

#[test]
fn too_many_dep_modules_rejected() {
    // main → b → c: full transitive closure has 2 deps. Limit = 1 → compile must fail.
    let cfg = CompilerConfig::default();
    let b_addr = Address::from_str("0xFB").unwrap();
    let c_addr = Address::from_str("0xFC").unwrap();

    let c_module = Compiler::compile(
        r#"
            mod c;

            pub fn get() -> u64 { 1 }
        "#,
        &[],
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = Compiler::compile(
        r#"
            mod b;

            use c@0xFC;

            pub fn get() -> u64 { c::get() }
        "#,
        &[(c_addr, &c_module)],
        &[],
        cfg,
    )
    .expect("b must compile");

    // Provide the full transitive closure [b, c] (len=2) with limit=1.
    let strict = CompilerConfig::default().with_max_dep_modules(1);
    assert!(matches!(
        Compiler::compile(
            r#"
                mod main;

                use b@0xFB;

                fn run() -> u64 { b::get() }
            "#,
            &[(b_addr, &b_module), (c_addr, &c_module)],
            &[],
            strict
        )
        .unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many dependency modules")
    ));
}

//
// ─── Struct fields ───
//

#[test]
fn too_many_fields_rejected() {
    let config = CompilerConfig::default();
    let fields = (0..=config.max_fields())
        .map(|i| format!("f{i}: u64"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!(
        r#"
            mod test;

            struct Big {{ {fields} }} fn noop() {{}}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many fields")
    ));
}

//
// ─── Function body ───
//

#[test]
fn too_many_params_rejected() {
    let config = CompilerConfig::default();
    let params = (0..=config.max_params())
        .map(|i| format!("p{i}: u64"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!(
        r#"
            mod test;

            fn f({params}) {{}}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many parameters")
    ));
}

#[test]
fn too_many_locals_rejected() {
    // With max_locals = 2, a function can hold at most 2 local slots.
    // Three let bindings need 3 slots — the third allocation must be rejected.
    let config = CompilerConfig::default().with_max_locals(2);
    let src = r#"
        mod test;

        fn f() { let a = 1; let b = 2; let c = 3; }
    "#;
    assert!(matches!(
        Compiler::compile(src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("too many local variables")
    ));
}

#[test]
fn function_too_large_rejected() {
    // With max_fun_code_size = 1, even the minimal fn body (PushU64 + Return = 2
    // instructions) exceeds the limit.
    let config = CompilerConfig::default().with_max_fun_code_size(1);
    let src = r#"
        mod test;

        fn f() -> u64 { 1 }
    "#;
    assert!(matches!(
        Compiler::compile(src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("bytecode too large")
    ));
}

//
// ─── Tuples ───
//

#[test]
fn tuple_literal_too_many_elements_rejected() {
    let config = CompilerConfig::default();
    let items = (0..=config.max_tuple_elements())
        .map(|i| format!("{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!(
        r#"
            mod test;

            fn f() {{ let t = ({items}); }}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("exceeding the limit")
    ));
}

#[test]
fn tuple_return_type_too_many_elements_rejected() {
    let config = CompilerConfig::default();
    let types = (0..=config.max_tuple_elements())
        .map(|_| "u64")
        .collect::<Vec<_>>()
        .join(", ");
    let items = (0..=config.max_tuple_elements())
        .map(|i| format!("{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let src = format!(
        r#"
            mod test;

            fn f() -> ({types}) {{ ({items}) }}
        "#
    );
    assert!(matches!(
        Compiler::compile(&src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("exceeding the limit")
    ));
}

//
// ─── Identifiers ───
//

#[test]
fn module_name_too_long_rejected() {
    let config = CompilerConfig::default();
    let long = "a".repeat(config.max_identifier_len() + 1);

    let src = format!(
        r#"
            mod {long};

            fn f() {{}}
        "#
    );

    assert!(matches!(
        Compiler::compile(&src, &[], &[], config).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("module name")
    ));
}
