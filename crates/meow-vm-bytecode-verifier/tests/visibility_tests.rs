mod utils;

use std::collections::HashMap;
use std::str::FromStr;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{address::Address, bytecode::Instruction, config::CompilerConfig, module_ref};

//
// ─── Happy paths ───
//

#[test]
fn cross_module_call_chain_passes() {
    let addr = dep_addr();
    let dep = utils::compile(
        r#"
        mod dep;

        pub fn double(x: u64) -> u64 { x + x }
    "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            pub fn quadruple(x: u64) -> u64 { dep::double(dep::double(x)) }
        "#,
        &[(addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    utils::verify_ok(&main, &deps);
}

//
// ─── Cross-module call visibility ───
//

#[test]
fn call_public_cross_module_function_passes() {
    let addr = dep_addr();
    let dep = utils::compile(
        r#"
        mod dep;

        pub fn get() -> u64 { 42 }
    "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            pub fn f() -> u64 { dep::get() }
        "#,
        &[(addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    utils::verify_ok(&main, &deps);
}

#[test]
fn call_private_cross_module_function_rejected() {
    // The compiler blocks cross-module calls to private functions at source level,
    // so we utils::compile main without the call and inject it via bytecode utils::tamper.
    let addr = dep_addr();
    let dep = utils::compile(
        r#"
        mod dep;

        fn secret() -> u64 { 99 }
    "#,
    );
    let mut main = utils::compile(
        r#"
        mod main;

        pub fn f() -> u64 { 1 }
    "#,
    );
    main.imports.push(addr);
    main.functions[0].code = vec![
        Instruction::Call(module_ref::qualify(&addr, "secret")),
        Instruction::Return,
    ];
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors(&main, &deps);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModuleCallToPrivateFunction { .. }
        )),
        "expected CrossModuleCallToPrivateFunction, got: {errs:?}"
    );
}

//
// ─── Cross-module struct construction ───
//

#[test]
fn cross_module_struct_construction_rejected() {
    // The compiler never emits NewStruct for cross-module types, so inject it directly.
    let addr = dep_addr();
    let dep = utils::compile(
        r#"
        mod dep;

        pub struct Point { x: u64, y: u64 }

        fn noop() -> u64 { 0 }
    "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;
            use dep@0x42;
            pub fn f() -> u64 { 1 }
        "#,
        &[(addr, &dep)],
    );
    main.functions[0].code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: module_ref::qualify(&addr, "Point"),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Return,
    ];
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors(&main, &deps);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::CrossModuleStructConstruction { .. })),
        "expected CrossModuleStructConstruction, got: {errs:?}"
    );
}

//
// ─── Cross-module private field read ───
//

#[test]
fn cross_module_private_field_read_via_load_field_rejected() {
    // A function that accepts a cross-module struct and tries to read a private
    // field via LoadField must be rejected by the bytecode verifier.
    // We craft the bytecode directly since the compiler rejects struct-typed field access.
    let addr = dep_addr();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Pair { a: u64, b: u64 }
        "#,
    );
    // Compile a pass-through function accepting dep::Pair, then tamper.
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            fn pass(p: dep::Pair) -> dep::Pair { p }
        "#,
        &[(addr, &dep)],
    );
    // Replace body with LoadField(0, ["a"])/Return — private field read on cross-module struct.
    utils::tamper(&mut main, "pass", |code| {
        *code = vec![
            Instruction::LoadField(0, vec!["a".to_string()]),
            Instruction::Return,
        ];
    });
    main.functions
        .iter_mut()
        .find(|f| f.name == "pass")
        .unwrap()
        .return_type = Some(meow_vm_types::types::Type::U64);

    let deps = HashMap::from([(addr, &dep)]);
    let errs = meow_vm_bytecode_verifier::verify(&main, &deps, &[], &CompilerConfig::default())
        .expect_err("cross-module LoadField must be rejected");
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::CrossModulePrivateFieldRead { .. })),
        "expected CrossModulePrivateFieldRead, got: {errs:?}"
    );
}

//
// ─── Cross-module field write ───
//

#[test]
fn cross_module_field_write_rejected() {
    // CrossModuleFieldWrite: StoreField on a slot holding a cross-module struct.
    // Compile a function that passes through a dep::Pair without field access
    // (compiler allows this), then utils::tamper the body to inject StoreField.
    let addr = dep_addr();
    let dep = utils::compile(
        r#"
        mod dep;

        pub struct Pair { a: u64, b: u64 }
    "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            fn pass(p: dep::Pair) -> dep::Pair { p }
        "#,
        &[(addr, &dep)],
    );
    // Inject PushU64 + StoreField before the Load/Return — StoreField on slot 0
    // (which the abstract interpreter knows holds a dep::Pair) triggers CrossModuleFieldWrite.
    utils::tamper(&mut main, "pass", |code| {
        code.splice(
            0..0,
            [
                Instruction::PushU64(42),
                Instruction::StoreField(0, vec!["a".to_string()]),
            ],
        );
    });
    let deps = HashMap::from([(addr, &dep)]);
    let errs = meow_vm_bytecode_verifier::verify(&main, &deps, &[], &CompilerConfig::default())
        .expect_err("cross-module StoreField must be rejected");
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::CrossModuleFieldWrite { .. })),
        "expected CrossModuleFieldWrite, got: {errs:?}"
    );
}

//
// ─── Missing dep ───
//

#[test]
fn missing_dep_causes_undefined_function_error() {
    let addr = dep_addr();
    let dep = utils::compile(
        r#"
        mod dep;

        pub fn get() -> u64 { 42 }
    "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            pub fn f() -> u64 { dep::get() }
        "#,
        &[(addr, &dep)],
    );
    let errs = utils::verify_errors(&main, &utils::no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UndefinedFunction { .. })),
        "expected UndefinedFunction for missing dep, got: {errs:?}"
    );
}

//
// ─── Utility functions ───
//

fn dep_addr() -> Address {
    Address::from_str("0x42").unwrap()
}
