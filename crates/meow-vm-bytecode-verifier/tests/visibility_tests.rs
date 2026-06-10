mod utils;

use std::collections::HashMap;
use std::str::FromStr;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{
    address::Address, bytecode::Instruction, config::CompilerConfig, module_ref, types::Type,
};

//
// ─── Happy paths ───
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

#[test]
fn cross_module_struct_parameter_passed_through_passes() {
    // A function that accepts a cross-module struct and returns it unchanged
    // (no field access, no construction) must pass verification.
    let d_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Pair { a: u64, b: u64 }
        "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn pass(p: dep::Pair) -> dep::Pair { p }
        "#,
        &[(d_addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    utils::verify_ok_with_deps(&main, &deps);
}

#[test]
fn cross_module_struct_from_dep_call_passed_through_passes() {
    // A function that calls a dep function returning a cross-module struct and
    // returns it unchanged (no field access) must pass verification.
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Point { x: u64 }

            pub fn make_point(x: u64) -> Point { Point { x: x } }
        "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f(x: u64) -> dep::Point { dep::make_point(x) }
        "#,
        &[(dep_addr, &dep)],
    );
    let deps: HashMap<Address, &_> = [(dep_addr, &dep)].into_iter().collect();
    utils::verify_ok_with_deps(&main, &deps);
}

//
// ─── Cross-module function call visibility ───
//

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
            VerificationError::CrossModuleCallToPrivateFunction { function, callee, .. }
            if function == "f" && *callee == module_ref::qualify(&d_addr, "secret")
        )),
        "expected CrossModuleCallToPrivateFunction(f → @0xFD::secret), got: {errs:?}"
    );
}

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
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TypeMismatch { expected, found, .. }
            if expected == "address" && found == "u64"
        )),
        "expected TypeMismatch(expected=address, found=u64), got: {errs:?}"
    );
}

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
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UndefinedFunction { callee, .. }
            if *callee == module_ref::qualify(&d_addr, "get")
        )),
        "expected UndefinedFunction(@0xFD::get), got: {errs:?}"
    );
}

//
// ─── Cross-module struct access ───
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
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModuleStructAccess { function, type_name, .. }
            if function == "f" && *type_name == module_ref::qualify(&d_addr, "Point")
        )),
        "expected CrossModuleStructAccess(f, @0xFD::Point), got: {errs:?}"
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
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModuleStructAccess { function, type_name, .. }
            if function == "f" && *type_name == module_ref::qualify(&d_addr, "Point")
        )),
        "expected CrossModuleStructAccess(f, @0xFD::Point), got: {errs:?}"
    );
}

#[test]
fn cross_module_private_struct_as_type_rejected() {
    // Referencing a *private* dep struct as a param/return type is rejected — only
    // `pub` structs are visible across modules. The compiler forbids this too, so to
    // exercise the verifier directly we compile with a public struct, then flip it to
    // private (the compiler would otherwise refuse to emit such bytecode).
    let d_addr = Address::from_str("0xFD").unwrap();
    let mut dep = utils::compile(
        r#"
            mod dep;

            pub struct Secret { a: u64 }
        "#,
    );
    let main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn pass(s: dep::Secret) -> dep::Secret { s }
        "#,
        &[(d_addr, &dep)],
    );
    // Make the dep struct private after the fact.
    dep.structs
        .iter_mut()
        .find(|s| s.name == "Secret")
        .unwrap()
        .is_public = false;

    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    let errors = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errors.iter().any(|e| matches!(
            e,
            VerificationError::CrossModulePrivateStructReference { type_name, .. }
            if type_name.contains("Secret")
        )),
        "referencing a private dep struct as a type must be rejected, got: {errors:?}"
    );
}

//
// ─── Cross-module field access on parameters ───
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
        .return_type = Some(Type::U64);

    let deps = HashMap::from([(d_addr, &dep)]);
    let errs = meow_vm_bytecode_verifier::verify(&main, &deps, &[], &CompilerConfig::default())
        .expect_err("cross-module LoadField must be rejected");
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModulePrivateFieldRead { function, type_name, field, .. }
            if function == "pass" && field == "a" && *type_name == module_ref::qualify(&d_addr, "Pair")
        )),
        "expected CrossModulePrivateFieldRead(pass, @0xFD::Pair, field=a), got: {errs:?}"
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
        .return_type = Some(Type::U64);
    let deps: HashMap<Address, &_> = [(d_addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModulePrivateFieldRead { function, type_name, field, .. }
            if function == "get_a" && field == "a" && *type_name == module_ref::qualify(&d_addr, "Pair")
        )),
        "expected CrossModulePrivateFieldRead(get_a, @0xFD::Pair, field=a), got: {errs:?}"
    );
}

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
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModuleFieldWrite { function, type_name, field, .. }
            if function == "pass" && field == "a" && *type_name == module_ref::qualify(&d_addr, "Pair")
        )),
        "expected CrossModuleFieldWrite(pass, @0xFD::Pair, field=a), got: {errs:?}"
    );
}

//
// ─── Cross-module field access on structs returned from dep calls ───
//
// qualify_type marks return-value structs as cross-module so that GetField,
// LoadField, and StoreField checks fire even when the struct was not a parameter
// but arrived via a dep function call.
//

#[test]
fn cross_module_get_field_on_returned_struct_rejected() {
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Point { x: u64 }

            pub fn make_point(x: u64) -> Point { Point { x: x } }
        "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f(x: u64) -> u64 { 1 }
        "#,
        &[(dep_addr, &dep)],
    );
    utils::tamper(&mut main, "f", |code| {
        *code = vec![
            Instruction::Load(0),
            Instruction::Call(module_ref::qualify(&dep_addr, "make_point")),
            Instruction::GetField("x".to_string()),
            Instruction::Return,
        ];
    });
    let deps = HashMap::from([(dep_addr, &dep)]);
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModulePrivateFieldRead { function, type_name, field, .. }
            if function == "f" && field == "x" && *type_name == module_ref::qualify(&dep_addr, "Point")
        )),
        "expected CrossModulePrivateFieldRead(f, @0xFD::Point, field=x), got: {errs:?}"
    );
}

#[test]
fn cross_module_load_field_on_returned_struct_rejected() {
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Point { x: u64 }

            pub fn make_point(x: u64) -> Point { Point { x: x } }
        "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f(x: u64) -> u64 { 1 }
        "#,
        &[(dep_addr, &dep)],
    );
    let func = main.functions.iter_mut().find(|f| f.name == "f").unwrap();
    func.local_count = 2; // slot 0: x param, slot 1: returned struct
    func.code = vec![
        Instruction::Load(0),
        Instruction::Call(module_ref::qualify(&dep_addr, "make_point")),
        Instruction::Store(1),
        Instruction::LoadField(1, vec!["x".to_string()]),
        Instruction::Return,
    ];
    let deps = HashMap::from([(dep_addr, &dep)]);
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModulePrivateFieldRead { function, type_name, field, .. }
            if function == "f" && field == "x" && *type_name == module_ref::qualify(&dep_addr, "Point")
        )),
        "expected CrossModulePrivateFieldRead(f, @0xFD::Point, field=x), got: {errs:?}"
    );
}

#[test]
fn cross_module_store_field_on_returned_struct_rejected() {
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Point { x: u64 }

            pub fn make_point(x: u64) -> Point { Point { x: x } }
        "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f(x: u64) -> u64 { 1 }
        "#,
        &[(dep_addr, &dep)],
    );
    let func = main.functions.iter_mut().find(|f| f.name == "f").unwrap();
    func.local_count = 2; // slot 0: x param, slot 1: returned struct
    func.code = vec![
        Instruction::Load(0),
        Instruction::Call(module_ref::qualify(&dep_addr, "make_point")),
        Instruction::Store(1),
        Instruction::PushU64(99),
        Instruction::StoreField(1, vec!["x".to_string()]),
        Instruction::PushU64(0),
        Instruction::Return,
    ];
    let deps = HashMap::from([(dep_addr, &dep)]);
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModuleFieldWrite { function, type_name, field, .. }
            if function == "f" && field == "x" && *type_name == module_ref::qualify(&dep_addr, "Point")
        )),
        "expected CrossModuleFieldWrite(f, @0xFD::Point, field=x), got: {errs:?}"
    );
}

#[test]
fn cross_module_get_field_on_struct_from_dep_tuple_rejected() {
    // qualify_type recurses into Tuple types, so a dep function returning (Token, u64)
    // must have the struct element qualified as @addr::Token. Field access on the
    // extracted struct must still be rejected as CrossModulePrivateFieldRead.
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            pub struct Token { amount: u64 }

            pub fn make_pair(amount: u64) -> (Token, u64) {
                let t = Token { amount: amount };
                (t, amount)
            }
        "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            pub fn f(x: u64) -> u64 { 1 }
        "#,
        &[(dep_addr, &dep)],
    );
    // Tamper: call dep::make_pair, unpack the (Token, u64) tuple, then GetField on Token.
    // Stack after UnpackTuple(2): Token on top, u64 below.
    let func = main.functions.iter_mut().find(|f| f.name == "f").unwrap();
    func.code = vec![
        Instruction::Load(0),
        Instruction::Call(module_ref::qualify(&dep_addr, "make_pair")),
        Instruction::UnpackTuple(2), // Token on top, u64 below
        Instruction::GetField("amount".to_string()), // ← CrossModulePrivateFieldRead
        Instruction::Pop,
        Instruction::Return,
    ];
    let deps = HashMap::from([(dep_addr, &dep)]);
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CrossModulePrivateFieldRead { function, type_name, field, .. }
            if function == "f" && field == "amount" && *type_name == module_ref::qualify(&dep_addr, "Token")
        )),
        "expected CrossModulePrivateFieldRead(f, @0xFD::Token, field=amount), got: {errs:?}"
    );
}

//
// ─── NativeParam::LocalStruct enforcement ───
//
// Some native functions declare NativeParam::LocalStruct, meaning they only accept
// structs defined in the calling module. Passing a cross-module struct must be rejected.
//

#[test]
fn local_struct_native_param_passes() {
    let mut module = utils::compile(
        r#"
            mod m;

            struct Token { amount: u64 }

            fn dummy() { return; }
        "#,
    );
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("obj".to_string(), Type::Struct("Token".to_string()))];
    func.local_count = 1;
    func.code = vec![
        Instruction::Load(0),
        Instruction::Call("consume_local_native".to_string()),
        Instruction::Return,
    ];
    utils::verify_ok(&module);
}

#[test]
fn cross_module_struct_rejected_by_local_only_native() {
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
            mod dep;

            struct Token { amount: u64 }

            pub fn make_token(amount: u64) -> Token { Token { amount: amount } }
        "#,
    );

    // Build a main module with one param; tamper the body to call dep::make_token
    // and then pass the returned cross-module struct to consume_local_native.
    let mut main = utils::compile_with_deps(
        r#"
            mod main;

            use dep@0xFD;

            fn dummy(amount: u64) { return; }
        "#,
        &[(dep_addr, &dep)],
    );
    let func = main
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.code = vec![
        Instruction::Load(0),
        Instruction::Call(module_ref::qualify(&dep_addr, "make_token")),
        Instruction::Call("consume_local_native".to_string()),
        Instruction::Return,
    ];

    let deps: HashMap<Address, &_> = [(dep_addr, &dep)].into_iter().collect();
    let errs = utils::verify_errors_with_deps(&main, &deps);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::NativeArgTypeMismatch { callee, arg_index: 0, expected, .. }
            if callee == "consume_local_native" && expected == "local struct"
        )),
        "expected NativeArgTypeMismatch(consume_local_native, arg=0, expected=local struct), got: {errs:?}"
    );
}
