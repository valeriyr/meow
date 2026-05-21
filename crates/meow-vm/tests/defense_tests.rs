//! VM runtime defense checks — guards against tampered bytecode that bypasses the verifier.
//!
//! Each test constructs a [`Module`] directly (no compiler, no bytecode verifier) and
//! injects an instruction sequence that the verifier would normally reject. The goal is
//! to confirm that the VM's own defensive checks fire correctly if those guarantees are
//! ever violated at runtime.

mod utils;

use meow_vm::error::VmError;
use meow_vm_types::{
    address::Address,
    bytecode::Instruction,
    module::{Function, Module},
    module_ref,
    types::{Type, Value},
};

//
// ─── StackUnderflow ───
//

#[test]
fn pop_on_empty_stack_returns_stack_underflow() {
    // Pop with nothing on the stack — the verifier's abstract interpretation
    // would normally catch this, but the VM guard fires if bytecode is tampered.
    let m = module_with_function(make_function("run", vec![], 0, vec![Instruction::Pop]));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::StackUnderflow),
        "expected StackUnderflow, got: {err:?}"
    );
}

//
// ─── TypeError ───
//

#[test]
fn not_on_integer_returns_type_error() {
    // `Not` requires a bool; PushU64 leaves an integer on the stack.
    // The verifier's type checker would catch the type mismatch statically.
    let m = module_with_function(make_function(
        "run",
        vec![],
        0,
        vec![Instruction::PushU64(0), Instruction::Not],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::TypeError(ref msg) if msg.contains("expected bool")),
        "expected TypeError(expected bool ...), got: {err:?}"
    );
}

#[test]
fn store_field_empty_path_returns_type_error() {
    // StoreField with an empty path is malformed bytecode — the verifier's
    // instruction-shape check would normally reject it.
    let m = module_with_function(make_function(
        "run",
        vec![],
        1,
        vec![
            Instruction::PushU64(0),
            Instruction::Store(0),
            Instruction::PushU64(1),
            Instruction::StoreField(0, vec![]),
        ],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::TypeError(ref msg) if msg.contains("path must not be empty")),
        "expected TypeError(path must not be empty), got: {err:?}"
    );
}

//
// ─── UndefinedVariable ───
//

#[test]
fn load_out_of_range_slot_returns_undefined_variable() {
    // local_count=0 allocates no slots; Load(5) is outside that range.
    // The verifier's slot-bounds check would normally catch this.
    let m = module_with_function(make_function("run", vec![], 0, vec![Instruction::Load(5)]));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::UndefinedVariable(5)),
        "expected UndefinedVariable(5), got: {err:?}"
    );
}

//
// ─── UndefinedStruct ───
//

#[test]
fn new_struct_unknown_type_returns_undefined_struct() {
    // The module declares no structs; NewStruct("Ghost") has no definition to look up.
    // The verifier's type-resolution pass would catch this before execution.
    let m = module_with_function(make_function(
        "run",
        vec![],
        0,
        vec![Instruction::NewStruct {
            type_name: "Ghost".to_string(),
            field_names: vec![],
        }],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::UndefinedStruct(ref name) if name == "Ghost"),
        "expected UndefinedStruct(Ghost), got: {err:?}"
    );
}

//
// ─── UndefinedField ───
//

#[test]
fn get_field_missing_field_returns_undefined_field() {
    // GetField("missing") on a struct that only has field "x".
    // The verifier's field-access check would catch this statically.
    let m = module_with_function(make_function(
        "run",
        vec![("s".to_string(), Type::Struct("S".to_string()))],
        1,
        vec![
            Instruction::Load(0),
            Instruction::GetField("missing".to_string()),
        ],
    ));
    let struct_val = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "S"),
        fields: vec![("x".to_string(), Value::U64(1))],
    };
    let err = utils::try_run(m, "run", vec![struct_val]).unwrap_err();
    assert!(
        matches!(err, VmError::UndefinedField { ref field, .. } if field == "missing"),
        "expected UndefinedField {{ field: \"missing\", .. }}, got: {err:?}"
    );
}

//
// ─── Helpers ───
//

fn make_function(
    name: &str,
    params: Vec<(String, Type)>,
    local_count: u8,
    code: Vec<Instruction>,
) -> Function {
    Function {
        name: name.to_string(),
        is_public: true,
        params,
        return_type: None,
        local_count,
        code,
    }
}

fn module_with_function(function: Function) -> Module {
    let mut module = Module::new("defense_test");
    module.functions.push(function);
    module
}
