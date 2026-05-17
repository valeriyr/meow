mod utils;
use utils::*;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{bytecode::Instruction, types::Type};

//
// ─── Happy paths ───
//

#[test]
fn if_else_both_branches_return_passes() {
    let module = compile(
        r#"
        mod m;

        fn max(a: u64, b: u64) -> u64 {
            if a > b { return a; } else { return b; }
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

#[test]
fn if_without_else_passes() {
    let module = compile(
        r#"
        mod m;

        fn clamp(x: u64, hi: u64) -> u64 {
            if x > hi { return hi; }
            x
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

#[test]
fn both_branches_same_stack_passes() {
    let module = compile(
        r#"
        mod m;

        fn abs_diff(a: u64, b: u64) -> u64 {
            if a > b {
                return a - b;
            } else {
                return b - a;
            }
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

//
// ─── Stack merge conflict ───
//

#[test]
fn stack_merge_conflict_rejected() {
    // One branch leaves u64 on stack, the other leaves bool.
    // The join point will have conflicting stack types.
    //
    // Layout:
    //   0: PushBool(true)
    //   1: JumpIfNot(3)      → target = 1+3 = 4
    //   2: PushU64(1)
    //   3: Jump(2)           → target = 3+2 = 5
    //   4: PushBool(false)   ← join from JumpIfNot
    //   5: Return            ← join from Jump — u64 vs bool conflict
    let mut module = compile(
        r#"
        mod m;

        fn f() -> u64 { 1 }
    "#,
    );
    module.functions[0].return_type = None;
    module.functions[0].code = vec![
        Instruction::PushBool(true),  // 0
        Instruction::JumpIfNot(3),    // 1 → target 4
        Instruction::PushU64(1),      // 2
        Instruction::Jump(2),         // 3 → target 5
        Instruction::PushBool(false), // 4
        Instruction::Return,          // 5
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::StackMergeConflict { .. }))
    );
}

//
// ─── Liveness merge conflict ───
//

#[test]
fn liveness_merge_conflict_rejected() {
    // One branch consumes a Coin (Load(0) → consume_native), the other skips it.
    // At the join point slot 0 liveness differs → LivenessMergeConflict.
    //
    // Layout (slot 0 = Coin, slot 1 = bool):
    //   0: Load(1)                — push bool
    //   1: JumpIfNot(4)           → target = 1+4 = 5
    //   2: Load(0)                — move Coin out of slot 0
    //   3: Call(consume_native)   — pop Coin, push Void
    //   4: Pop                    — pop Void; stack []
    //   5: Return                 ← join: slot 0 Dead vs Live(Coin)
    let mut module = compile(
        r#"
        mod m;

        struct Coin { id: address, value: u64 }

        fn dummy() { return; }
    "#,
    );
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![
        ("c".to_string(), Type::Struct("Coin".to_string())),
        ("cond".to_string(), Type::Bool),
    ];
    func.return_type = None;
    func.local_count = 2;
    func.code = vec![
        Instruction::Load(1),
        Instruction::JumpIfNot(4), // target = 1+4 = 5
        Instruction::Load(0),
        Instruction::Call("consume_native".to_string()),
        Instruction::Pop,
        Instruction::Return, // ← join point
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::LivenessMergeConflict { slot: 0, .. })),
        "expected LivenessMergeConflict, got: {errs:?}"
    );
}
