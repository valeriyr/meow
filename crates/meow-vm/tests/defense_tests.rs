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
    types::{FieldDef, StructDef, Type, Value},
};

//
// ─── EqOnLinearType ───
//

#[test]
fn eq_on_struct_returns_eq_on_linear_type() {
    // Two struct values on the stack; Eq instruction fires VmError::EqOnLinearType.
    // The bytecode verifier would normally catch this statically.
    let m = point_module_with_function(make_function(
        "run",
        vec![],
        0,
        vec![
            Instruction::PushU64(1),
            Instruction::PushU64(2),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::PushU64(1),
            Instruction::PushU64(2),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::Eq,
        ],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::EqOnLinearType(ref name) if name.contains("Point")),
        "expected EqOnLinearType(Point), got: {err:?}"
    );
}

#[test]
fn ne_on_struct_returns_eq_on_linear_type() {
    // Same as above but with the Ne instruction.
    let m = point_module_with_function(make_function(
        "run",
        vec![],
        0,
        vec![
            Instruction::PushU64(3),
            Instruction::PushU64(4),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::PushU64(3),
            Instruction::PushU64(4),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::Ne,
        ],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::EqOnLinearType(ref name) if name.contains("Point")),
        "expected EqOnLinearType(Point), got: {err:?}"
    );
}

#[test]
fn eq_on_struct_on_right_returns_eq_on_linear_type() {
    // Left operand is a primitive, right operand is a struct; the VM must still
    // fire EqOnLinearType and report the right operand's type name.
    let m = point_module_with_function(make_function(
        "run",
        vec![],
        0,
        vec![
            Instruction::PushU64(99),
            Instruction::PushU64(1),
            Instruction::PushU64(2),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::Eq,
        ],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::EqOnLinearType(ref name) if name.contains("Point")),
        "expected EqOnLinearType(Point), got: {err:?}"
    );
}

#[test]
fn ne_on_struct_on_right_returns_eq_on_linear_type() {
    // Left operand is a primitive, right operand is a struct; Ne must also fire.
    let m = point_module_with_function(make_function(
        "run",
        vec![],
        0,
        vec![
            Instruction::PushU64(99),
            Instruction::PushU64(1),
            Instruction::PushU64(2),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::Ne,
        ],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::EqOnLinearType(ref name) if name.contains("Point")),
        "expected EqOnLinearType(Point), got: {err:?}"
    );
}

#[test]
fn eq_on_tuple_containing_struct_returns_eq_on_linear_type() {
    // A tuple that wraps a struct is itself linear; Eq on it fires VmError::EqOnLinearType.
    let m = point_module_with_function(make_function(
        "run",
        vec![],
        0,
        vec![
            // (Point { x:1, y:2 }, 42)
            Instruction::PushU64(1),
            Instruction::PushU64(2),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::PushU64(42),
            Instruction::MakeTuple(2),
            // identical second tuple
            Instruction::PushU64(1),
            Instruction::PushU64(2),
            Instruction::NewStruct {
                type_name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
            },
            Instruction::PushU64(42),
            Instruction::MakeTuple(2),
            Instruction::Eq,
        ],
    ));
    let err = utils::try_run(m, "run", vec![]).unwrap_err();
    assert!(
        matches!(err, VmError::EqOnLinearType(ref name) if name == "tuple"),
        "expected EqOnLinearType(tuple), got: {err:?}"
    );
}

//
// ─── SlotOverwrite ───
//

#[test]
fn store_overwrites_live_struct_returns_slot_overwrite() {
    // Slot 0 holds a live struct param; storing any value there without
    // consuming it first is a resource leak. The verifier catches this statically.
    let m = module_with_function(make_function(
        "run",
        vec![("s".to_string(), Type::Struct("S".to_string()))],
        1,
        vec![
            Instruction::PushU64(42),
            Instruction::Store(0), // slot 0 still holds the live struct
        ],
    ));
    let s = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "S"),
        fields: vec![("x".to_string(), Value::U64(1))],
    };
    let err = utils::try_run(m, "run", vec![s]).unwrap_err();
    assert!(
        matches!(err, VmError::SlotOverwrite(0)),
        "expected SlotOverwrite(0), got: {err:?}"
    );
}

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

#[test]
fn unpack_struct_with_mismatched_fields_returns_error() {
    // Module defines Token { value: u64, extra: u64 }.
    // The runtime value carries { amount: u64 } — different field name, fewer fields.
    // The VM must return UndefinedField rather than panic.
    let mut m = Module::new("defense_test");
    m.structs.push(StructDef {
        name: "Token".to_string(),
        is_public: false,
        fields: vec![
            FieldDef {
                name: "value".to_string(),
                ty: Type::U64,
            },
            FieldDef {
                name: "extra".to_string(),
                ty: Type::U64,
            },
        ],
    });
    m.functions.push(make_function(
        "run",
        vec![("t".to_string(), Type::Struct("Token".to_string()))],
        1,
        vec![
            Instruction::Load(0),
            Instruction::UnpackStruct {
                type_name: "Token".to_string(),
                field_names: vec!["value".to_string(), "extra".to_string()],
            },
        ],
    ));
    // Values passed to the VM must carry the qualified type name; the module is at Address::ZERO.
    let wrong_token = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "Token"),
        fields: vec![("amount".to_string(), Value::U64(100))],
    };
    let err = utils::try_run(m, "run", vec![wrong_token]).unwrap_err();
    assert!(
        matches!(err, VmError::UndefinedField { ref field, .. } if field == "extra"),
        "expected UndefinedField {{ field: \"extra\", .. }}, got: {err:?}"
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
// ─── UseAfterMove ───
//

#[test]
fn load_after_move_returns_use_after_move() {
    // Load a struct param (consuming it), then Load the same slot again.
    // The verifier's UseAfterMove check would catch this statically.
    let m = module_with_function(make_function(
        "run",
        vec![("s".to_string(), Type::Struct("S".to_string()))],
        1,
        vec![
            Instruction::Load(0), // moves s out of slot 0
            Instruction::Load(0), // slot 0 is now None
        ],
    ));
    let s = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "S"),
        fields: vec![("x".to_string(), Value::U64(1))],
    };
    let err = utils::try_run(m, "run", vec![s]).unwrap_err();
    assert!(
        matches!(err, VmError::UseAfterMove(ref msg) if msg.contains("slot 0")),
        "expected UseAfterMove for slot 0, got: {err:?}"
    );
}

#[test]
fn load_field_after_move_returns_use_after_move() {
    // Load a struct param (consuming it), then try LoadField on the now-dead slot.
    let m = module_with_function(make_function(
        "run",
        vec![("s".to_string(), Type::Struct("S".to_string()))],
        1,
        vec![
            Instruction::Load(0),
            Instruction::LoadField(0, vec!["x".to_string()]),
        ],
    ));
    let s = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "S"),
        fields: vec![("x".to_string(), Value::U64(1))],
    };
    let err = utils::try_run(m, "run", vec![s]).unwrap_err();
    assert!(
        matches!(err, VmError::UseAfterMove(ref msg) if msg.contains("slot 0")),
        "expected UseAfterMove for slot 0, got: {err:?}"
    );
}

#[test]
fn store_field_after_move_returns_use_after_move() {
    // Load a struct param (consuming it), then try StoreField on the now-dead slot.
    let m = module_with_function(make_function(
        "run",
        vec![("s".to_string(), Type::Struct("S".to_string()))],
        1,
        vec![
            Instruction::Load(0),
            Instruction::PushU64(42),
            Instruction::StoreField(0, vec!["x".to_string()]),
        ],
    ));
    let s = Value::Struct {
        type_name: module_ref::qualify(&Address::ZERO, "S"),
        fields: vec![("x".to_string(), Value::U64(1))],
    };
    let err = utils::try_run(m, "run", vec![s]).unwrap_err();
    assert!(
        matches!(err, VmError::UseAfterMove(ref msg) if msg.contains("slot 0")),
        "expected UseAfterMove for slot 0, got: {err:?}"
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

fn point_module_with_function(function: Function) -> Module {
    let mut module = Module::new("defense_test");
    module.structs.push(StructDef {
        name: "Point".to_string(),
        is_public: true,
        fields: vec![
            FieldDef {
                name: "x".to_string(),
                ty: Type::U64,
            },
            FieldDef {
                name: "y".to_string(),
                ty: Type::U64,
            },
        ],
    });
    module.functions.push(function);
    module
}
