mod utils;
use utils::*;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{bytecode::Instruction, config::CompilerConfig};

//
// ─── Happy paths ───
//

#[test]
fn valid_module_passes() {
    let module = compile(
        r#"
        mod m;
        fn add(a: u64, b: u64) -> u64 {
            a + b
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

#[test]
fn if_else_passes() {
    let module = compile(
        r#"
        mod m;
        fn pick(cond: bool, a: u64, b: u64) -> u64 {
            if cond { return a; } else { return b; }
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

//
// ─── Identifier validation ───
//

#[test]
fn invalid_module_name_rejected() {
    let mut module = compile(
        r#"
        mod valid;
        fn f() -> u64 { 1 }
    "#,
    );
    module.name = "1invalid".to_string();
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::InvalidIdentifier { name, .. } if name == "1invalid"
    )));
}

#[test]
fn invalid_struct_name_rejected() {
    let mut module = compile(
        r#"
        mod m;
        struct S { x: u64 }
        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].name = "bad-name".to_string();
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::InvalidIdentifier { .. }))
    );
}

#[test]
fn invalid_field_name_rejected() {
    let mut module = compile(
        r#"
        mod m;
        struct S { x: u64 }
        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields[0].name = "bad name".to_string();
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::InvalidIdentifier { .. }))
    );
}

//
// ─── Duplicate names ───
//

#[test]
fn duplicate_struct_name_rejected() {
    let mut module = compile(
        r#"
        mod m;
        struct S { x: u64 }
        fn f() -> u64 { 1 }
    "#,
    );
    let dup = module.structs[0].clone();
    module.structs.push(dup);
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::DuplicateStructName { name } if name == "S"
    )));
}

#[test]
fn duplicate_function_name_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { 1 }
    "#,
    );
    let dup = module.functions[0].clone();
    module.functions.push(dup);
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::DuplicateFunctionName { name } if name == "f"
    )));
}

//
// ─── Object first-field constraint ───
//

#[test]
fn object_missing_id_field_rejected() {
    let mut module = compile(
        r#"
        mod m;
        object Coin { id: address, value: u64 }
        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields[0].name = "ident".to_string();
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::ObjectMissingIdField { struct_name } if struct_name == "Coin"
    )));
}

#[test]
fn invalid_object_name_rejected() {
    let mut module = compile(
        r#"
        mod m;
        object Coin { id: address, value: u64 }
        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].name = "bad-name".to_string();
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::InvalidIdentifier { .. }))
    );
}

#[test]
fn invalid_object_field_name_rejected() {
    let mut module = compile(
        r#"
        mod m;
        object Coin { id: address, value: u64 }
        fn f() -> u64 { 1 }
    "#,
    );
    module.structs[0].fields[1].name = "bad name".to_string();
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::InvalidIdentifier { .. }))
    );
}

#[test]
fn duplicate_object_name_rejected() {
    let mut module = compile(
        r#"
        mod m;
        object Coin { id: address, value: u64 }
        fn f() -> u64 { 1 }
    "#,
    );
    let dup = module.structs[0].clone();
    module.structs.push(dup);
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::DuplicateStructName { name } if name == "Coin"
    )));
}

//
// ─── local_count / slot bounds ───
//

#[test]
fn local_count_too_small_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f(x: u64) -> u64 { x }
    "#,
    );
    module.functions[0].local_count = 0;
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::LocalCountTooSmall { .. }))
    );
}

#[test]
fn slot_out_of_range_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { 1 }
    "#,
    );
    tamper(&mut module, "f", |code| {
        code.insert(0, Instruction::Load(5));
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::SlotOutOfRange { slot: 5, .. }))
    );
}

//
// ─── Jump checks ───
//

#[test]
fn backward_jump_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { 1 }
    "#,
    );
    tamper(&mut module, "f", |code| {
        code.insert(0, Instruction::Jump(-1));
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::BackwardJump { .. }))
    );
}

#[test]
fn jump_out_of_bounds_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { 1 }
    "#,
    );
    tamper(&mut module, "f", |code| {
        code.insert(0, Instruction::Jump(10000));
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::JumpOutOfBounds { .. }))
    );
}

//
// ─── Tuple element count ───
//

#[test]
fn tuple_too_large_in_return_type_rejected() {
    let limit = CompilerConfig::default().max_tuple_elements();
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { 1 }
    "#,
    );
    // Inject an oversized MakeTuple instruction directly.
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::MakeTuple((limit + 1) as u8),
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::TupleTooLarge { .. })),
        "MakeTuple exceeding limit must be rejected, got: {errs:?}"
    );
}

#[test]
fn jump_to_past_end_rejected() {
    // A reachable Jump(offset) with target == code_len escapes the function
    // without a Return, bypassing MissingReturn and UnconsumedObject checks.
    // The abstract interpreter must catch it via the pending[code_len] path.
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { 1 }
    "#,
    );
    tamper(&mut module, "f", |code| {
        // Replace code with just Jump(1), which lands at code_len = 1.
        // No Return follows — the MissingReturn must be detected.
        *code = vec![Instruction::Jump(1)];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::MissingReturn { .. })),
        "reachable jump to code_len must produce MissingReturn, got: {errs:?}"
    );
}
