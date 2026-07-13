mod utils;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{bytecode::Instruction, types::Type};

//
// ─── Happy paths ───
//

#[test]
fn if_else_both_branches_return_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn max(a: u64, b: u64) -> u64 {
            if a > b { return a; } else { return b; }
        }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn if_without_else_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn clamp(x: u64, hi: u64) -> u64 {
            if x > hi { return hi; }
            x
        }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn both_branches_same_stack_passes() {
    let module = utils::compile(
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
    utils::verify_ok(&module);
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
    let mut module = utils::compile(
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
    let errs = utils::verify_errors(&module);
    // JumpIfNot(3) at pc=1 → target=4; Jump(2) at pc=3 → target=5. Both branches merge at pc=5.
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::StackMergeConflict { function, join_pc }
        if function == "f" && *join_pc == 5
    )));
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
    let mut module = utils::compile(
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
    let errs = utils::verify_errors(&module);
    // JumpIfNot(4) at pc=1 → target=5 (Return). Coin in slot 0 consumed on true branch, not on false.
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::LivenessMergeConflict { function, slot: 0, join_pc }
            if function == "dummy" && *join_pc == 5
        )),
        "expected LivenessMergeConflict(dummy, slot=0, join_pc=5), got: {errs:?}"
    );
}

//
// ─── Slot state divergence at join points ───
//

/// Hand-crafted body where the two branches store DIFFERENT non-linear types into
/// slot 1 (U64 on the then-path, Address on the else-path), followed by `extra`
/// instructions at the join and a final `Return`.
///
/// Layout (slot 0 = cond bool param, slot 1 = scratch):
///   0: Load(0)        — push cond
///   1: JumpIfNot(4)   → 5 (else branch)
///   2: PushU64(7)
///   3: Store(1)       — slot 1 = U64
///   4: Jump(3)        → 7 (join)
///   5: PushAddress    — else branch
///   6: Store(1)       — slot 1 = Address
///   7: <extra...>, Return
fn divergent_primitive_slot_module(extra: Vec<Instruction>) -> meow_vm_types::module::Module {
    let mut module = utils::compile(
        r#"
        mod m;

        fn dummy() { return; }
    "#,
    );
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("cond".to_string(), Type::Bool)];
    func.return_type = None;
    func.local_count = 2;
    func.code = vec![
        Instruction::Load(0),                                            // 0
        Instruction::JumpIfNot(4),                                       // 1 → 5
        Instruction::PushU64(7),                                         // 2
        Instruction::Store(1),                                           // 3  slot1 = U64
        Instruction::Jump(3),                                            // 4 → 7
        Instruction::PushAddress(meow_vm_types::address::Address::ZERO), // 5
        Instruction::Store(1),                                           // 6  slot1 = Address
    ];
    func.code.extend(extra);
    func.code.push(Instruction::Return); // 7 (join)
    module
}

#[test]
fn divergent_primitive_slot_unused_after_join_passes() {
    // Nothing linear can leak here, so the divergent slot is merged to Dead and the
    // join is accepted as long as the slot is never read afterwards.
    let module = divergent_primitive_slot_module(vec![]);
    utils::verify_ok(&module);
}

#[test]
fn divergent_primitive_slot_loaded_after_join_rejected() {
    // Reading the path-dependent value after the join is unsound — the merged slot
    // is Dead, so the Load is rejected as a use-after-move.
    let module = divergent_primitive_slot_module(vec![Instruction::Load(1)]);
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UseAfterMove { function, slot: 1, .. }
            if function == "dummy"
        )),
        "expected UseAfterMove(dummy, slot=1), got: {errs:?}"
    );
}

#[test]
fn let_inside_if_body_verifies() {
    // Regression test: a primitive `let` inside an if body leaves its slot live on
    // the then-path and dead on the fall-through path. The divergence is non-linear,
    // so the join must merge it away rather than reject the function.
    let module = utils::compile(
        r#"
        mod m;

        pub fn f(c: bool) -> u64 {
            if c {
                let y = 5;
                let z = y + 1;
            }
            7
        }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn slot_type_merge_conflict_rejected() {
    // Two branches store live LINEAR values of different struct types into the same
    // slot. The value can no longer be tracked past the join, and merging the slot
    // away would hide the resource leak — rejected at the join point.
    //
    // Layout (slot 0 = cond bool param, slot 1 = scratch):
    //   0: Load(0)              — push cond
    //   1: JumpIfNot(5)         → 6 (else branch)
    //   2: PushU64(1)
    //   3: NewStruct A { v }
    //   4: Store(1)             — slot 1 = A
    //   5: Jump(4)              → 9 (join)
    //   6: PushU64(2)           — else branch
    //   7: NewStruct B { v }
    //   8: Store(1)             — slot 1 = B
    //   9: Return
    let mut module = utils::compile(
        r#"
        mod m;

        struct A { v: u64 }
        struct B { v: u64 }

        fn dummy() { return; }
    "#,
    );
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("cond".to_string(), Type::Bool)];
    func.return_type = None;
    func.local_count = 2;
    func.code = vec![
        Instruction::Load(0),      // 0
        Instruction::JumpIfNot(5), // 1 → 6
        Instruction::PushU64(1),   // 2
        Instruction::NewStruct {
            type_name: "A".to_string(),
            field_names: vec!["v".to_string()],
        }, // 3
        Instruction::Store(1),     // 4  slot1 = A
        Instruction::Jump(4),      // 5 → 9
        Instruction::PushU64(2),   // 6
        Instruction::NewStruct {
            type_name: "B".to_string(),
            field_names: vec!["v".to_string()],
        }, // 7
        Instruction::Store(1),     // 8  slot1 = B
        Instruction::Return,       // 9
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::SlotTypeMergeConflict { function, slot: 1, .. }
            if function == "dummy"
        )),
        "expected SlotTypeMergeConflict(dummy, slot=1), got: {errs:?}"
    );
}

//
// ─── Linear value left on the operand stack at Return ───
//

#[test]
fn linear_value_beneath_return_value_rejected() {
    // Build a struct, push a u64 return value on top, then Return. The struct sits
    // beneath the return value and would be silently dropped at runtime.
    let mut module = utils::compile(
        r#"
        mod m;

        struct W { v: u64 }

        fn dummy() -> u64 { return 0; }
    "#,
    );
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.return_type = Some(Type::U64);
    func.local_count = 0;
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::NewStruct {
            type_name: "W".to_string(),
            field_names: vec!["v".to_string()],
        },
        Instruction::PushU64(9),
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnconsumedStructOnStack { function } if function == "dummy"
        )),
        "expected UnconsumedStructOnStack(dummy), got: {errs:?}"
    );
}
