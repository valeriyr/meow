mod utils;
use utils::*;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::bytecode::Instruction;

//
// ─── Happy paths ───
//

#[test]
fn arithmetic_and_return_passes() {
    let module = compile(
        r#"
        mod m;
        fn compute(a: u64, b: u64) -> u64 {
            let c = a + b;
            return c * 2;
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

#[test]
fn native_sender_call_passes() {
    let module = compile(
        r#"
        mod m;
        fn get_sender() -> address {
            return meow_vm_sender();
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

#[test]
fn native_rand_call_passes() {
    let module = compile(
        r#"
        mod m;
        fn get_rand() -> u64 {
            return meow_vm_rand();
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

//
// ─── Type mismatch ───
//

#[test]
fn add_bool_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushBool(true),
            Instruction::PushBool(true),
            Instruction::Add,
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::TypeMismatch { expected, .. } if expected == "u64"
    )));
}

#[test]
fn compare_mismatched_types_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> bool { return true; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushBool(true),
            Instruction::PushU64(1),
            Instruction::Eq,
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::TypeMismatch { .. }))
    );
}

//
// ─── Stack underflow ───
//

#[test]
fn stack_underflow_on_add() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(1),
            Instruction::Add,
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::StackUnderflow { .. }))
    );
}

#[test]
fn stack_underflow_on_pop() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::Pop,
            Instruction::PushU64(1),
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::StackUnderflow { .. }))
    );
}

//
// ─── Return type mismatch ───
//

#[test]
fn wrong_return_type_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![Instruction::PushBool(true), Instruction::Return];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::ReturnTypeMismatch { .. }))
    );
}

#[test]
fn missing_return_detected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![Instruction::PushU64(1)];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::MissingReturn { .. }))
    );
}

//
// ─── Undefined function ───
//

#[test]
fn call_unknown_function_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::Call("no_such_fn".to_string()),
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::UndefinedFunction { callee, .. } if callee == "no_such_fn"
    )));
}

//
// ─── Native arg errors ───
//

#[test]
fn native_wrong_arg_count_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(1),
            Instruction::Call("meow_vm_abort".to_string()),
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::NativeArgCountMismatch { callee, .. } if callee == "meow_vm_abort"
    )));
}

#[test]
fn native_wrong_arg_type_rejected() {
    let mut module = compile(
        r#"
        mod m;
        fn f() -> u64 { return 1; }
    "#,
    );
    tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(1), // should be bool
            Instruction::PushU64(2),
            Instruction::PushStr("msg".to_string()),
            Instruction::Call("meow_vm_abort".to_string()),
            Instruction::Return,
        ];
    });
    let errs = verify_errors(&module, &no_deps());
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::NativeArgTypeMismatch { callee, .. } if callee == "meow_vm_abort"
    )));
}
