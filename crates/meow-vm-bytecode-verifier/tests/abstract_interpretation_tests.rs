mod utils;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::bytecode::Instruction;

//
// ─── Happy paths ───
//

#[test]
fn arithmetic_and_return_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn compute(a: u64, b: u64) -> u64 {
            let c = a + b;
            c * 2
        }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn native_returning_address_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn get_addr() -> address {
            addr_native()
        }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn native_returning_u64_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn get_num() -> u64 {
            u64_native()
        }
    "#,
    );
    utils::verify_ok(&module);
}

//
// ─── Boolean not ───
//

#[test]
fn not_on_bool_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn f(x: bool) -> bool { !x }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn not_on_non_bool_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> bool { true }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(1),
            Instruction::Not,
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::TypeMismatch { expected, .. } if expected == "bool"
    )));
}

#[test]
fn not_on_empty_stack_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> bool { true }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![Instruction::Not, Instruction::Return];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::StackUnderflow { function, expected, .. }
        if function == "f" && expected == "bool"
    )));
}

//
// ─── Type mismatch ───
//

#[test]
fn add_bool_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushBool(true),
            Instruction::PushBool(true),
            Instruction::Add,
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::TypeMismatch { expected, .. } if expected == "u64"
    )));
}

#[test]
fn compare_mismatched_types_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> bool { true }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushBool(true),
            Instruction::PushU64(1),
            Instruction::Eq,
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        // Eq pops RHS then LHS; stack had [Bool(bottom), U64(top)] → expected=LHS=bool, found=RHS=u64
        VerificationError::TypeMismatch { expected, found, .. }
        if expected == "bool" && found == "u64"
    )));
}

//
// ─── Stack underflow ───
//

#[test]
fn stack_underflow_on_add() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(1),
            Instruction::Add,
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        // Add iterates ["right operand", "left operand"]; right is on top and pops OK,
        // then left has nothing → StackUnderflow with expected = "left operand".
        VerificationError::StackUnderflow { function, expected, .. }
        if function == "f" && expected == "left operand"
    )));
}

#[test]
fn stack_underflow_on_pop() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::Pop,
            Instruction::PushU64(1),
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::StackUnderflow { function, expected, .. }
        if function == "f" && expected == "any"
    )));
}

//
// ─── Return type mismatch ───
//

#[test]
fn wrong_return_type_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![Instruction::PushBool(true), Instruction::Return];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::ReturnTypeMismatch { function, declared, found }
        if function == "f" && declared == "u64" && found == "bool"
    )));
}

#[test]
fn missing_return_detected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![Instruction::PushU64(1)];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::MissingReturn { function }
        if function == "f"
    )));
}

//
// ─── Undefined function ───
//

#[test]
fn call_unknown_function_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::Call("no_such_fn".to_string()),
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::UndefinedFunction { callee, .. } if callee == "no_such_fn"
    )));
}

//
// ─── Struct type mismatch ───
//

#[test]
fn unpack_wrong_struct_type_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        struct Point { x: u64, y: u64 }
        struct Coin { balance: u64 }

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(50),
            Instruction::NewStruct {
                type_name: "Coin".to_string(),
                field_names: vec!["balance".to_string()],
            },
            Instruction::UnpackStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::Pop,
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::TypeMismatch { expected, found, .. }
                if expected.contains("Point") && found.contains("Coin")
        )),
        "unpacking Coin as Point must be a type error, got: {errs:?}"
    );
}

//
// ─── Native arg errors ───
//

#[test]
fn native_wrong_arg_count_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(1),
            Instruction::Call("meow_vm_abort".to_string()),
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::NativeArgCountMismatch { callee, .. } if callee == "meow_vm_abort"
    )));
}

#[test]
fn native_wrong_arg_type_rejected() {
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(1), // should be bool
            Instruction::PushU64(2),
            Instruction::PushStr("msg".to_string()),
            Instruction::Call("meow_vm_abort".to_string()),
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::NativeArgTypeMismatch { callee, .. } if callee == "meow_vm_abort"
    )));
}

#[test]
fn native_any_struct_non_struct_arg_rejected() {
    // consume_native (declared in utils::test_natives with NativeParam::AnyStruct)
    // takes any struct; passing a u64 must be rejected.
    let mut module = utils::compile(
        r#"
        mod m;

        fn f() {}
    "#,
    );
    utils::tamper(&mut module, "f", |code| {
        *code = vec![
            Instruction::PushU64(42), // not a struct
            Instruction::Call("consume_native".to_string()),
            Instruction::Return,
        ];
    });
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::NativeArgTypeMismatch { callee, arg_index: 0, .. }
            if callee == "consume_native"
        )),
        "expected NativeArgTypeMismatch for consume_native arg 0, got: {errs:?}"
    );
}
