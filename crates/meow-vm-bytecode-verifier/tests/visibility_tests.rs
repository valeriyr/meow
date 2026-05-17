mod utils;
use utils::*;

use std::collections::HashMap;
use std::str::FromStr;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{address::Address, bytecode::Instruction, config::CompilerConfig};

//
// ─── Happy paths ───
//

#[test]
fn cross_module_call_chain_passes() {
    let addr = dep_addr();
    let dep = compile(
        r#"
        mod dep;

        pub fn double(x: u64) -> u64 { x + x }
    "#,
    );
    let main = compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            pub fn quadruple(x: u64) -> u64 { dep::double(dep::double(x)) }
        "#,
        &[(addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    verify_ok(&main, &deps);
}

//
// ─── Cross-module call visibility ───
//

#[test]
fn call_public_cross_module_function_passes() {
    let addr = dep_addr();
    let dep = compile(
        r#"
        mod dep;

        pub fn get() -> u64 { 42 }
    "#,
    );
    let main = compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            pub fn f() -> u64 { dep::get() }
        "#,
        &[(addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    verify_ok(&main, &deps);
}

#[test]
fn call_private_cross_module_function_rejected() {
    // The compiler blocks cross-module calls to private functions at source level,
    // so we compile main without the call and inject it via bytecode tamper.
    let addr = dep_addr();
    let dep = compile(
        r#"
        mod dep;

        fn secret() -> u64 { 99 }
    "#,
    );
    let mut main = compile(
        r#"
        mod main;

        pub fn f() -> u64 { 1 }
    "#,
    );
    main.imports.push(addr);
    let addr_str = addr.to_string();
    main.functions[0].code = vec![
        Instruction::Call(format!("@{addr_str}::secret")),
        Instruction::Return,
    ];
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    let errs = verify_errors(&main, &deps);
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
    let dep = compile(
        r#"
        mod dep;

        pub struct Point { x: u64, y: u64 }

        fn noop() -> u64 { 0 }
    "#,
    );
    let mut main = compile_with_deps(
        r#"
            mod main;
            use dep@0x42;
            pub fn f() -> u64 { 1 }
        "#,
        &[(addr, &dep)],
    );
    let addr_str = addr.to_string();
    main.functions[0].code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: format!("@{addr_str}::Point"),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Return,
    ];
    let deps: HashMap<Address, &_> = [(addr, &dep)].into_iter().collect();
    let errs = verify_errors(&main, &deps);
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
    // A cross-module struct can land on the stack via LoadField reading a
    // same-module field whose *type* is cross-module (e.g. `hero.id` yields
    // `dep::Id`). A subsequent GetField("inner") on it must be rejected.
    let addr = dep_addr();
    let dep = compile(
        r#"
            mod dep;

            pub struct Id { inner: u64 }
        "#,
    );
    // Compile a module that has a struct containing a dep::Id field.
    // The compiler emits LoadField(slot, ["id"]) to read the field (same-module
    // read, allowed). We then tamper to append GetField("inner") to read the
    // private field of dep::Id directly from the stack.
    let mut main = compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            struct Wrapper { id: dep::Id }

            pub fn get_id(w: Wrapper) -> dep::Id { w.id }
        "#,
        &[(addr, &dep)],
    );
    tamper(&mut main, "get_id", |code| {
        // Remove the Return, append GetField("inner") then Return so the
        // tampered function tries to read the private `inner` field.
        code.retain(|i| !matches!(i, Instruction::Return));
        code.push(Instruction::GetField("inner".to_string()));
        code.push(Instruction::Return);
    });

    let deps = HashMap::from([(addr, &dep)]);
    let errs = verify_errors(&main, &deps);
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
    // (compiler allows this), then tamper the body to inject StoreField.
    let addr = dep_addr();
    let dep = compile(
        r#"
        mod dep;

        pub struct Pair { a: u64, b: u64 }
    "#,
    );
    let mut main = compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            fn pass(p: dep::Pair) -> dep::Pair { p }
        "#,
        &[(addr, &dep)],
    );
    // Inject PushU64 + StoreField before the Load/Return — StoreField on slot 0
    // (which the abstract interpreter knows holds a dep::Pair) triggers CrossModuleFieldWrite.
    tamper(&mut main, "pass", |code| {
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
    let dep = compile(
        r#"
        mod dep;

        pub fn get() -> u64 { 42 }
    "#,
    );
    let main = compile_with_deps(
        r#"
            mod main;

            use dep@0x42;

            pub fn f() -> u64 { dep::get() }
        "#,
        &[(addr, &dep)],
    );
    let errs = verify_errors(&main, &no_deps());
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
