//! Execution coverage for ISA instructions the compiler never emits.
//!
//! `JumpIf` (jump-if-true) and `Dup` are valid instructions handled by the VM,
//! both verifiers, and the gas schedule, but the current codegen only emits
//! `JumpIfNot`/`Jump` and never `Dup`. They are therefore reachable only via
//! hand-built bytecode — these tests exercise the VM's handling of them directly.

mod utils;

use meow_vm_types::{
    bytecode::Instruction,
    module::{Function, Module},
    types::{Type, Value},
};

//
// ─── JumpIf (jump-if-true) ───
//

#[test]
fn jump_if_takes_branch_when_true() {
    // pick(cond) -> u64 : returns 1 when cond is true, 2 otherwise, using JumpIf.
    //   pc0 Load(0)      push cond
    //   pc1 JumpIf(3)    true → pc4
    //   pc2 PushU64(2)   false path
    //   pc3 Jump(2)      → pc5
    //   pc4 PushU64(1)   true path
    //   pc5 Return
    let code = vec![
        Instruction::Load(0),
        Instruction::JumpIf(3),
        Instruction::PushU64(2),
        Instruction::Jump(2),
        Instruction::PushU64(1),
        Instruction::Return,
    ];
    let fun = func("pick", vec![("cond".to_string(), Type::Bool)], 1, code);

    let when_true =
        utils::try_run(module_with(fun.clone()), "pick", vec![Value::Bool(true)]).expect("run");
    assert_eq!(
        when_true,
        Some(Value::U64(1)),
        "JumpIf should take the branch when condition is true"
    );

    let when_false =
        utils::try_run(module_with(fun), "pick", vec![Value::Bool(false)]).expect("run");
    assert_eq!(
        when_false,
        Some(Value::U64(2)),
        "JumpIf should fall through when condition is false"
    );
}

//
// ─── Dup ───
//

#[test]
fn dup_duplicates_stack_top() {
    // dup_add() -> u64 : push 21, Dup, Add → 42.
    let code = vec![
        Instruction::PushU64(21),
        Instruction::Dup,
        Instruction::Add,
        Instruction::Return,
    ];
    let fun = func("dup_add", vec![], 0, code);
    let result = utils::try_run(module_with(fun), "dup_add", vec![]).expect("run");
    assert_eq!(
        result,
        Some(Value::U64(42)),
        "Dup should duplicate the stack top"
    );
}

//
// ─── Utilities ───
//

/// A public function returning `u64` with the given params and body.
fn func(
    name: &str,
    params: Vec<(String, Type)>,
    local_count: u8,
    code: Vec<Instruction>,
) -> Function {
    Function {
        name: name.to_string(),
        is_public: true,
        params,
        return_type: Some(Type::U64),
        local_count,
        code,
    }
}

fn module_with(function: Function) -> Module {
    let mut module = Module::new("isa_test");
    module.functions.push(function);
    module
}
