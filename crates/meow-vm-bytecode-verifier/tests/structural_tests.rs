mod utils;

use std::collections::HashMap;
use std::str::FromStr;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{
    address::Address,
    bytecode::Instruction,
    config::CompilerConfig,
    module::Function,
    module_ref,
    types::{FieldDef, StructDef, Type},
};

//
// ─── Happy paths ───
//

#[test]
fn valid_module_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn add(a: u64, b: u64) -> u64 {
            a + b
        }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn if_else_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn pick(cond: bool, a: u64, b: u64) -> u64 {
            if cond { return a; } else { return b; }
        }
    "#,
    );
    utils::verify_ok(&module);
}

//
// ─── Identifier validation ───
//

#[test]
fn invalid_module_name_rejected() {
    let mut module = utils::compile(
        r#"
        mod valid;

        fn f() -> u64 { 1 }
    "#,
    );
    module.name = "1invalid".to_string();
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::InvalidIdentifier { name, .. } if name == "1invalid"
    )));
}

#[test]
fn invalid_struct_name_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].name = "bad-name".to_string();
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::InvalidIdentifier { name, context }
        if name == "bad-name" && context == "struct definition"
    )));
}

#[test]
fn invalid_field_name_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields[0].name = "bad name".to_string();
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::InvalidIdentifier { name, context }
        if name == "bad name" && context.starts_with("field in struct '")
    )));
}

#[test]
fn duplicate_struct_name_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    let dup = module.structs[0].clone();
    module.structs.push(dup);
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::DuplicateStructName { name } if name == "S"
    )));
}

#[test]
fn duplicate_function_name_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    let dup = module.functions[0].clone();
    module.functions.push(dup);
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::DuplicateFunctionName { name } if name == "f"
    )));
}

#[test]
fn config_reserved_function_name_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.functions[0].name = "my_native".to_string();
    let cfg = CompilerConfig::default().with_reserved_function_names(&["my_native"]);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::InvalidIdentifier { name, .. } if name == "my_native"
        )),
        "config-reserved function name must be rejected by verifier, got: {errs:?}"
    );
}

//
// ─── Module limits ───
//

#[test]
fn too_many_structs_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs.push(StructDef {
        name: "T".to_string(),
        fields: vec![],
        is_public: false,
    });
    let cfg = CompilerConfig::default().with_max_structs(1);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::TooManyStructs { count: 2, limit: 1 })),
        "exceeding max_structs must be rejected, got: {errs:?}"
    );
}

#[test]
fn too_many_functions_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    let dup = Function {
        name: "g".to_string(),
        ..module.functions[0].clone()
    };
    module.functions.push(dup);
    let cfg = CompilerConfig::default().with_max_functions(1);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TooManyFunctions { count: 2, limit: 1 }
        )),
        "exceeding max_functions must be rejected, got: {errs:?}"
    );
}

#[test]
fn too_many_imports_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.imports.push(Address::from([1u8; 32]));
    module.imports.push(Address::from([2u8; 32]));
    let cfg = CompilerConfig::default().with_max_imports(1);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::TooManyImports { count: 2, limit: 1 })),
        "exceeding max_imports must be rejected, got: {errs:?}"
    );
}

//
// ─── Struct definitions ───
//

#[test]
fn empty_struct_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 } fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields.clear();
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(
            |e| matches!(e, VerificationError::EmptyStruct { struct_name } if struct_name == "S")
        ),
        "empty struct must be rejected, got: {errs:?}"
    );
}

#[test]
fn too_many_fields_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields.push(FieldDef {
        name: "y".to_string(),
        ty: Type::U64,
    });
    let cfg = CompilerConfig::default().with_max_fields(1);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TooManyFields { struct_name, count: 2, limit: 1 }
            if struct_name == "S"
        )),
        "exceeding max_fields must be rejected, got: {errs:?}"
    );
}

#[test]
fn unresolved_type_in_struct_field_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields[0].ty = Type::Struct("Ghost".to_string());
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnresolvedTypeReference { type_name, context }
            if type_name == "Ghost" && context.contains("field 'x' in struct 'S'")
        )),
        "unresolved struct field type must be rejected, got: {errs:?}"
    );
}

#[test]
fn struct_self_reference_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct A { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields[0].ty = Type::Struct("A".to_string());
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CyclicStructDefinition { struct_name } if struct_name == "A"
        )),
        "self-referential struct must be rejected, got: {errs:?}"
    );
}

#[test]
fn struct_mutual_cycle_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct A { x: u64 }
        struct B { y: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    // A.x → B, B.y → A
    module.structs[0].fields[0].ty = Type::Struct("B".to_string());
    module.structs[1].fields[0].ty = Type::Struct("A".to_string());
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::CyclicStructDefinition { struct_name } if struct_name == "A"
        )),
        "mutually cyclic structs must be rejected, got: {errs:?}"
    );
}

#[test]
fn acyclic_struct_field_type_passes() {
    // B has a field of type A; A has only primitive fields — no cycle.
    let mut module = utils::compile(
        r#"
        mod m;

        struct A { x: u64 }
        struct B { y: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[1].fields[0].ty = Type::Struct("A".to_string());
    utils::verify_ok(&module);
}

#[test]
fn tuple_field_type_rejected() {
    // Tuples are not valid field types — only primitives and structs are.
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields[0].ty = Type::Tuple(vec![Type::U64, Type::U64]);
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TupleFieldType { struct_name, field_name }
            if struct_name == "S" && field_name == "x"
        )),
        "expected TupleFieldType for struct S field x, got: {errs:?}"
    );
}

//
// ─── Function definitions ───
//

#[test]
fn local_count_too_small_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f(x: u64) -> u64 { x }
    "#,
    );
    module.functions[0].local_count = 0;
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::LocalCountTooSmall { function, local_count: 0, param_count: 1 }
        if function == "f"
    )));
}

#[test]
fn too_many_params_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f(a: u64) -> u64 { a }
    "#,
    );
    module.functions[0]
        .params
        .push(("b".to_string(), Type::U64));
    module.functions[0].local_count = 2;
    let cfg = CompilerConfig::default().with_max_params(1);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TooManyParams { function, count: 2, limit: 1 }
            if function == "f"
        )),
        "exceeding max_params must be rejected, got: {errs:?}"
    );
}

#[test]
fn function_too_large_rejected() {
    let module = utils::compile(r#"mod m; fn f() -> u64 { 1 }"#);
    let cfg = CompilerConfig::default().with_max_fun_code_size(1);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::FunctionTooLarge { function, .. }
            if function == "f"
        )),
        "exceeding max_fun_code_size must be rejected, got: {errs:?}"
    );
}

#[test]
fn too_many_locals_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.functions[0].local_count = 5;
    let cfg = CompilerConfig::default().with_max_locals(4);
    let errs = verify_errors_cfg(&module, cfg);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TooManyLocals { function, count: 5, limit: 4 }
            if function == "f"
        )),
        "exceeding max_locals must be rejected, got: {errs:?}"
    );
}

#[test]
fn tuple_too_large_in_return_type_rejected() {
    let limit = CompilerConfig::default().max_tuple_elements();
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    // Inject an oversized MakeTuple instruction directly.
    utils::tamper(&mut module, "f", |code| {
        *code = vec![Instruction::MakeTuple(limit + 1), Instruction::Return];
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TupleTooLarge { function, size, .. }
            if function == "f" && *size == (limit + 1) as usize
        )),
        "MakeTuple exceeding limit must be rejected, got: {errs:?}"
    );
}

#[test]
fn unresolved_type_in_param_rejected() {
    // A function param whose struct type is not defined in the module must be rejected.
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.functions[0].params = vec![("x".to_string(), Type::Struct("Ghost".to_string()))];
    module.functions[0].local_count = 1;
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnresolvedTypeReference { type_name, context }
            if type_name == "Ghost" && context.contains("param 'x'")
        )),
        "unresolved param type must be rejected, got: {errs:?}"
    );
}

#[test]
fn unresolved_type_in_return_type_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.functions[0].return_type = Some(Type::Struct("Ghost".to_string()));
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnresolvedTypeReference { type_name, context }
            if type_name == "Ghost" && context.contains("return type")
        )),
        "unresolved return type must be rejected, got: {errs:?}"
    );
}

#[test]
fn unresolved_type_in_tuple_rejected() {
    // validate_type_ref recurses into Tuple elements; a ghost struct inside a
    // tuple return type must be caught just like a bare struct.
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.functions[0].return_type = Some(Type::Tuple(vec![
        Type::U64,
        Type::Struct("Ghost".to_string()),
    ]));
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnresolvedTypeReference { type_name, context }
            if type_name == "Ghost" && context.contains("return type")
        )),
        "unresolved struct inside tuple return type must be rejected, got: {errs:?}"
    );
}

#[test]
fn unregistered_dep_address_in_type_rejected() {
    // A qualified type @0xFF...::Token where that address is not in module.imports
    // must be rejected even though the name looks like a valid cross-module ref.
    let dep_addr = Address::from([0xFFu8; 32]);
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.functions[0].params = vec![(
        "p".to_string(),
        Type::Struct(module_ref::qualify(&dep_addr, "Token")),
    )];
    module.functions[0].local_count = 1;
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnresolvedTypeReference { type_name, .. }
            if type_name.ends_with("::Token")
        )),
        "qualified type with unregistered dep address must be rejected, got: {errs:?}"
    );
}

#[test]
fn undefined_struct_in_registered_dep_type_rejected() {
    // @0xFD is a registered import but the dep module does not define 'Ghost'.
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = utils::compile(
        r#"
        mod dep;

        pub struct Real { x: u64 }
    "#,
    );
    let mut main = utils::compile_with_deps(
        r#"
                mod m;

                use dep@0xFD;

                fn f() -> u64 { 1 }
            "#,
        &[(dep_addr, &dep)],
    );
    main.functions[0].params = vec![(
        "p".to_string(),
        Type::Struct(module_ref::qualify(&dep_addr, "Ghost")),
    )];
    main.functions[0].local_count = 1;
    let deps = std::collections::HashMap::from([(dep_addr, &dep)]);
    let errs = meow_vm_bytecode_verifier::verify(&main, &deps, &[], &CompilerConfig::default())
        .expect_err("non-existent struct in dep must be rejected");
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnresolvedTypeReference { type_name, .. }
            if type_name.ends_with("::Ghost")
        )),
        "param type referencing non-existent dep struct must be rejected, got: {errs:?}"
    );
}

//
// ─── Per-instruction checks ───
//

#[test]
fn slot_out_of_range_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        code.insert(0, Instruction::Load(5));
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::SlotOutOfRange { slot: 5, .. }))
    );
}

#[test]
fn load_field_empty_path_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f(s: S) { let S { x } = s; }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![Instruction::LoadField(0, vec![]), Instruction::Return];
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::EmptyFieldPath { function, .. } if function == "f"
        )),
        "expected EmptyFieldPath for LoadField with empty path, got: {errs:?}"
    );
}

#[test]
fn store_field_empty_path_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn f(s: S) { let S { x } = s; }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(0),
            Instruction::StoreField(0, vec![]),
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::EmptyFieldPath { function, .. } if function == "f"
        )),
        "expected EmptyFieldPath for StoreField with empty path, got: {errs:?}"
    );
}

#[test]
fn backward_jump_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        code.insert(0, Instruction::Jump(-1));
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::BackwardJump { function, offset: -1, .. } if function == "f"
    )));
}

#[test]
fn jump_out_of_bounds_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        code.insert(0, Instruction::Jump(10000));
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::JumpOutOfBounds { function, target: 10000, .. } if function == "f"
    )));
}

#[test]
fn jump_to_past_end_rejected() {
    // A reachable Jump(offset) with target == code_len escapes the function
    // without a Return, bypassing MissingReturn and UnconsumedObject checks.
    // The abstract interpreter must catch it via the pending[code_len] path.
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        // Replace code with just Jump(1), which lands at code_len = 1.
        // No Return follows — the MissingReturn must be detected.
        *code = vec![Instruction::Jump(1)];
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::MissingReturn { function } if function == "f"
        )),
        "reachable jump to code_len must produce MissingReturn, got: {errs:?}"
    );
}

#[test]
fn unknown_struct_type_in_new_struct_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn make(v: u64) -> S { S { x: v } }
    "#,
    );
    utils::tamper(&mut module, "make", |code| {
        for instr in code.iter_mut() {
            if let Instruction::NewStruct { type_name, .. } = instr {
                *type_name = "Ghost".to_string();
            }
        }
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UndefinedStructType { type_name, .. } if type_name == "Ghost"
        )),
        "NewStruct with unknown type must be rejected, got: {errs:?}"
    );
}

#[test]
fn new_struct_field_mismatch_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn make(v: u64) -> S { S { x: v } }
    "#,
    );
    utils::tamper(&mut module, "make", |code| {
        for instr in code.iter_mut() {
            if let Instruction::NewStruct { field_names, .. } = instr {
                *field_names = vec!["wrong_field".to_string()];
            }
        }
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::StructFieldMismatch { type_name, .. } if type_name == "S"
        )),
        "NewStruct with wrong field list must be rejected, got: {errs:?}"
    );
}

#[test]
fn unpack_struct_field_mismatch_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct S { x: u64 }

        fn consume(s: S) -> u64 {
            let S { x } = s;
            x
        }
    "#,
    );
    utils::tamper(&mut module, "consume", |code| {
        for instr in code.iter_mut() {
            if let Instruction::UnpackStruct { field_names, .. } = instr {
                *field_names = vec!["wrong_field".to_string()];
            }
        }
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::StructFieldMismatch { type_name, .. } if type_name == "S"
        )),
        "UnpackStruct with wrong field list must be rejected, got: {errs:?}"
    );
}

//
// ─── Utility functions ───
//

fn verify_errors_cfg(
    module: &meow_vm_types::module::Module,
    cfg: CompilerConfig,
) -> Vec<VerificationError> {
    meow_vm_bytecode_verifier::verify(module, &HashMap::new(), &[], &cfg)
        .expect_err("expected verification errors but verification passed")
}
