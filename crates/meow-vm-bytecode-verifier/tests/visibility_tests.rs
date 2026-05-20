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
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
        mod dep;

        pub fn double(x: u64) -> u64 { x + x }
    "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn quadruple(x: u64) -> u64 { dep::double(dep::double(x)) }
        "#,
        &[(d_addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    utils::verify_ok_with_deps(&main, &deps);
}

//
// ─── Cross-module call visibility ───
//

#[test]
fn call_public_cross_module_function_passes() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
        mod dep;

        pub fn get() -> u64 { 42 }
    "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f() -> u64 { dep::get() }
        "#,
        &[(d_addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    utils::verify_ok_with_deps(&main, &deps);
}

#[test]
fn call_private_cross_module_function_rejected() {
    // The compiler blocks cross-module calls to private functions at source level,
    // so we utils::compile main without the call and inject it via bytecode utils::tamper.
    let d_addr = Address::from_str("0xFD").unwrap();
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
    main.imports.push(d_addr);
    main.functions[0].code = vec![
        Instruction::Call(module_ref::qualify(&d_addr, "secret")),
        Instruction::Return,
    ];
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors_with_deps(&main, &deps);
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
    let d_addr = Address::from_str("0xFD").unwrap();
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
            use dep@0xFD;
            pub fn f() -> u64 { 1 }
        "#,
        &[(d_addr, &dep)],
    );
    main.functions[0].code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: module_ref::qualify(&d_addr, "Point"),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Return,
    ];
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::CrossModuleStructConstruction { .. })),
        "expected CrossModuleStructConstruction, got: {errs:?}"
    );
}

#[test]
fn cross_module_unpack_struct_rejected() {
    // UnpackStruct on a cross-module type is forbidden by the same structural rule as NewStruct.
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
        mod dep;

        pub struct Point { x: u64, y: u64 }
    "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f() -> u64 { 1 }
        "#,
        &[(d_addr, &dep)],
    );
    utils::tamper(&mut main, "f", |code| {
        *code = vec![
            Instruction::UnpackStruct {
                type_name: module_ref::qualify(&d_addr, "Point"),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::Return,
        ];
    });
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors_with_deps(&main, &deps);
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
    let d_addr = Address::from_str("0xFD").unwrap();
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

            use dep@0xFD;

            fn pass(p: dep::Pair) -> dep::Pair { p }
        "#,
        &[(d_addr, &dep)],
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

    let deps = HashMap::from([(d_addr, &dep)]);
    let errs = meow_vm_bytecode_verifier::verify(&main, &deps, &[], &CompilerConfig::default())
        .expect_err("cross-module LoadField must be rejected");
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::CrossModulePrivateFieldRead { .. })),
        "expected CrossModulePrivateFieldRead, got: {errs:?}"
    );
}

#[test]
fn cross_module_get_field_rejected() {
    // GetField on a cross-module struct also triggers CrossModulePrivateFieldRead —
    // the abstract interpreter checks visibility on the same path as LoadField.
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Pair { a: u64, b: u64 }
        "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            fn get_a(p: dep::Pair) -> dep::Pair { p }
        "#,
        &[(d_addr, &dep)],
    );
    utils::tamper(&mut main, "get_a", |code| {
        *code = vec![
            Instruction::Load(0),
            Instruction::GetField("a".to_string()),
            Instruction::Return,
        ];
    });
    main.functions
        .iter_mut()
        .find(|f| f.name == "get_a")
        .unwrap()
        .return_type = Some(meow_vm_types::types::Type::U64);
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors_with_deps(&main, &deps);
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
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
        mod dep;

        pub struct Pair { a: u64, b: u64 }
    "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            fn pass(p: dep::Pair) -> dep::Pair { p }
        "#,
        &[(d_addr, &dep)],
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
    let deps = HashMap::from([(d_addr, &dep)]);
    let errs = meow_vm_bytecode_verifier::verify(&main, &deps, &[], &CompilerConfig::default())
        .expect_err("cross-module StoreField must be rejected");
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::CrossModuleFieldWrite { .. })),
        "expected CrossModuleFieldWrite, got: {errs:?}"
    );
}

//
// ─── Cross-module call type checking ───
//

#[test]
fn cross_module_call_with_wrong_arg_type_rejected() {
    // The compiler guarantees correct argument types at the call site, but a hand-crafted
    // module could push a u64 where an address is expected. The verifier must catch it.
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
        mod dep;

        pub fn takes_address(a: address) -> u64 { 1 }
    "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f(a: address) -> u64 { dep::takes_address(a) }
        "#,
        &[(d_addr, &dep)],
    );
    // Replace Load(0) (pushes the address param) with PushU64 to pass the wrong type.
    utils::tamper(&mut main, "f", |code| {
        if let Some(instr) = code.first_mut() {
            *instr = Instruction::PushU64(0);
        }
    });
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::TypeMismatch { .. })),
        "expected TypeMismatch, got: {errs:?}"
    );
}

//
// ─── Missing dep ───
//

#[test]
fn missing_dep_causes_undefined_function_error() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
        mod dep;

        pub fn get() -> u64 { 42 }
    "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f() -> u64 { dep::get() }
        "#,
        &[(d_addr, &dep)],
    );
    let errs = utils::verify_errors(&main);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UndefinedFunction { .. })),
        "expected UndefinedFunction for missing dep, got: {errs:?}"
    );
}
