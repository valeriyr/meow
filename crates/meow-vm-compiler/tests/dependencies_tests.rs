mod utils;

use std::str::FromStr;

use meow_vm_compiler::{Compiler, error::CompilerError};
use meow_vm_types::{address::Address, config::CompilerConfig, module::Module};

//
// ─── extract_deps ───
//

#[test]
fn extract_deps_returns_declared_imports() {
    let src = r#"
        mod main;

        use math@0x01;
        use util@0x02;

        fn run() -> u64 { 0 }
    "#;
    let deps = Compiler::extract_deps(src).unwrap();
    assert_eq!(deps.len(), 2);
    assert_eq!(
        deps[0],
        ("math".to_string(), None, Address::from_str("0x01").unwrap())
    );
    assert_eq!(
        deps[1],
        ("util".to_string(), None, Address::from_str("0x02").unwrap())
    );
}

#[test]
fn extract_deps_no_imports_returns_empty() {
    let src = r#"
        mod main;

        fn run() -> u64 { 0 }
    "#;
    assert!(Compiler::extract_deps(src).unwrap().is_empty());
}

#[test]
fn extract_deps_duplicate_use_is_rejected() {
    let src = r#"
        mod main;

        use helper@0x42;
        use helper@0x42;
    "#;
    assert!(matches!(
        Compiler::extract_deps(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("duplicate use declaration")
    ));
}

#[test]
fn extract_deps_missing_module_decl_is_rejected() {
    assert!(matches!(
        Compiler::extract_deps("use helper@0x01;").unwrap_err(),
        CompilerError::Message(msg) if msg.contains("mod NAME;")
    ));
}

#[test]
fn extract_deps_duplicate_module_decl_is_rejected() {
    let src = r#"
        mod a;
        mod b;
    "#;
    assert!(matches!(
        Compiler::extract_deps(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("duplicate 'mod NAME;'")
    ));
}

//
// ─── use / import rules ───
//

#[test]
fn duplicate_dep_address_is_rejected() {
    let addr = Address::from_str("0x42").unwrap();
    let dep = utils::compile(
        r#"
            mod helper;

            fn get() -> u64 { 1 }
        "#,
    )
    .expect("dep must compile");

    let src = r#"
        mod main;

        use helper@0x42;

        fn run() -> u64 { helper::get() }
    "#;
    assert!(matches!(
        utils::compile_with_deps(src, &[(addr, &dep), (addr, &dep)]).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("duplicate dep address")
    ));
}

#[test]
fn duplicate_use_declaration_is_rejected() {
    let dep = utils::compile(
        r#"
            mod helper;

            fn get() -> u64 { 1 }
        "#,
    )
    .expect("dep must compile");

    let src = r#"
        mod main;

        use helper@0x42;
        use helper@0x42;

        fn run() -> u64 { helper::get() }
    "#;
    assert!(matches!(
        utils::compile_with_deps(src, &[(Address::from_str("0x42").unwrap(), &dep)]).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("duplicate use declaration")
    ));
}

#[test]
fn use_unknown_dep_is_compile_error() {
    let src = r#"
        mod bad;

        use nonexistent@0x99;
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("unknown dependency 'nonexistent@")
    ));
}

#[test]
fn undeclared_module_reference_is_compile_error() {
    let src = r#"
        mod bad;

        fn run(a: u64, b: u64) -> u64 { math::add(a, b) }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("undeclared module 'math'")
    ));
}

//
// ─── Cross-module struct rules ───
//

#[test]
fn cross_module_struct_as_field_type_accepted() {
    // Cross-module struct types are allowed as field types.
    let coin_mod = utils::compile(
        r#"
            mod coin;

            pub struct Coin { id: address, balance: u64 }
        "#,
    )
    .expect("coin must compile");

    let src = r#"
            mod wrapper;

            use coin@0x40;

            struct Wrapper { c: coin::Coin }

            fn noop() {}
        "#;
    utils::compile_with_deps(src, &[(Address::from_str("0x40").unwrap(), &coin_mod)])
        .expect("cross-module struct as field type must be accepted");
}

//
// ─── Transitive closure completeness ───
//

#[test]
fn complete_transitive_closure_is_accepted() {
    // Same chain with both b and c provided — must compile successfully.
    let b_addr = Address::from_str("0x02").unwrap();
    let c_addr = Address::from_str("0x03").unwrap();
    let cfg = CompilerConfig::default();

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

            use c@0x03;

            pub fn get() -> u64 { c::get() }
        "#,
        &[(c_addr, &c_module)],
        &[],
        cfg.clone(),
    )
    .expect("b must compile");

    assert!(
        Compiler::compile(
            r#"
                mod main;

                use b@0x02;

                fn run() -> u64 { b::get() }
            "#,
            &[(b_addr, &b_module), (c_addr, &c_module)],
            &[],
            cfg,
        )
        .is_ok()
    );
}

#[test]
fn diamond_dependency_graph_accepted() {
    // main → b, main → c, b → d, c → d (diamond shape).
    // d is a shared transitive dep — the compiler must accept this without
    // treating the shared dep as a cycle or duplicate error.
    let b_addr = Address::from_str("0x02").unwrap();
    let c_addr = Address::from_str("0x03").unwrap();
    let d_addr = Address::from_str("0x04").unwrap();
    let cfg = CompilerConfig::default();

    let d_module = Compiler::compile(
        r#"
            mod d;
        
            pub fn val() -> u64 { 1 }
        "#,
        &[],
        &[],
        cfg.clone(),
    )
    .expect("d must compile");

    let b_module = Compiler::compile(
        r#"
            mod b;

            use d@0x04;

            pub fn get() -> u64 { d::val() }
        "#,
        &[(d_addr, &d_module)],
        &[],
        cfg.clone(),
    )
    .expect("b must compile");

    let c_module = Compiler::compile(
        r#"
            mod c;

            use d@0x04;

            pub fn get() -> u64 { d::val() }
        "#,
        &[(d_addr, &d_module)],
        &[],
        cfg.clone(),
    )
    .expect("c must compile");

    assert!(
        Compiler::compile(
            r#"
                mod main;

                use b@0x02;
                use c@0x03;

                pub fn run() -> u64 { b::get() + c::get() }
            "#,
            &[
                (b_addr, &b_module),
                (c_addr, &c_module),
                (d_addr, &d_module),
            ],
            &[],
            cfg,
        )
        .is_ok(),
        "diamond dependency graph must be accepted"
    );
}

#[test]
fn missing_transitive_dep_is_rejected() {
    // main → b → c. Providing only b (not c) must fail because b's import of c is unresolved.
    let b_addr = Address::from_str("0x02").unwrap();
    let c_addr = Address::from_str("0x03").unwrap();
    let cfg = CompilerConfig::default();

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

            use c@0x03;

            pub fn get() -> u64 { c::get() }
        "#,
        &[(c_addr, &c_module)],
        &[],
        cfg.clone(),
    )
    .expect("b must compile");

    // Provide only b — c is missing from the transitive closure.
    assert!(matches!(
        Compiler::compile(
            r#"
                mod main;

                use b@0x02;

                fn run() -> u64 { b::get() }
            "#,
            &[(b_addr, &b_module)],
            &[],
            cfg,
        )
        .unwrap_err(),
        CompilerError::Message(msg) if msg.contains("transitive dependency")
    ));
}

//
// ─── Alias (`use mod@addr as alias`) ───
//

#[test]
fn two_modules_same_name_different_addresses_with_aliases_accepted() {
    let dep1 = utils::compile(
        r#"
            mod math;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
    )
    .expect("dep1 must compile");
    let dep2 = utils::compile(
        r#"
            mod math;

            pub fn mul(a: u64, b: u64) -> u64 { a * b }
        "#,
    )
    .expect("dep2 must compile");

    let src = r#"
        mod main;

        use math@0x01 as math1;
        use math@0x02 as math2;

        pub fn run(a: u64, b: u64) -> u64 { math1::add(a, b) }
    "#;
    utils::compile_with_deps(
        src,
        &[
            (Address::from_str("0x01").unwrap(), &dep1),
            (Address::from_str("0x02").unwrap(), &dep2),
        ],
    )
    .expect("two modules with same name but different aliases must compile");
}

#[test]
fn extract_deps_alias_returns_alias_as_key() {
    let src = r#"
        mod main;

        use helper@0x42 as h;

        fn run() -> u64 { 0 }
    "#;
    let deps = Compiler::extract_deps(src).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(
        deps[0],
        (
            "helper".to_string(),
            Some("h".to_string()),
            Address::from_str("0x42").unwrap()
        )
    );
}

#[test]
fn alias_used_for_cross_module_call() {
    let dep = utils::compile(
        r#"
            mod helper;

            pub fn get() -> u64 { 7 }
        "#,
    )
    .expect("dep must compile");

    let src = r#"
        mod main;

        use helper@0x42 as h;

        pub fn run() -> u64 { h::get() }
    "#;
    utils::compile_with_deps(src, &[(Address::from_str("0x42").unwrap(), &dep)])
        .expect("alias must work for cross-module call");
}

#[test]
fn original_module_name_rejected_when_alias_set() {
    let dep = utils::compile(
        r#"
            mod helper;

            pub fn get() -> u64 { 1 }
        "#,
    )
    .expect("dep must compile");

    // Declares alias `h` — using `helper::get()` must fail.
    let src = r#"
        mod main;

        use helper@0x42 as h;

        pub fn run() -> u64 { helper::get() }
    "#;
    assert!(matches!(
        utils::compile_with_deps(src, &[(Address::from_str("0x42").unwrap(), &dep)]).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("undeclared module 'helper'")
    ));
}

#[test]
fn alias_too_long_is_rejected() {
    let dep = utils::compile(
        r#"
            mod helper;

            pub fn get() -> u64 { 1 }
        "#,
    )
    .expect("dep must compile");

    let long_alias = "a".repeat(256);
    let src = format!(
        r#"
            mod main;

            use helper@0x42 as {long_alias};
        "#
    );
    assert!(matches!(
        utils::compile_with_deps(&src, &[(Address::from_str("0x42").unwrap(), &dep)]).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("not a valid identifier")
    ));
}

#[test]
fn duplicate_alias_is_rejected() {
    let src = r#"
        mod main;

        use foo@0x01 as shared;
        use bar@0x02 as shared;
    "#;
    assert!(matches!(
        Compiler::extract_deps(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("duplicate use declaration")
    ));
}

//
// ─── Module dependency cycle ───
//

#[test]
fn circular_module_dep_is_compile_error() {
    let addr1 = Address::from_str("0x01").unwrap();
    let addr2 = Address::from_str("0x02").unwrap();
    // Hand-craft two modules that import each other — the compiler refuses to produce
    // cycles itself, so we construct them directly to test the cycle detector.
    let mut mod_a = Module::new("mod_a");
    mod_a.imports = vec![addr2]; // A imports B

    let mut mod_b = Module::new("mod_b");
    mod_b.imports = vec![addr1]; // B imports A

    assert!(matches!(
        Compiler::compile(
            "mod top;",
            &[(addr1, &mod_a), (addr2, &mod_b)],
            &[],
            CompilerConfig::default(),
        )
        .unwrap_err(),
        CompilerError::Message(msg) if msg.contains("circular module dependency")
    ));
}
