mod utils;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{bytecode::Instruction, types::Type};

//
// ─── Happy path ───
//

#[test]
fn struct_consumed_by_call_passes() {
    // A struct loaded from its slot and passed as an argument to a function is
    // consumed — no UnconsumedStruct should fire at Return.
    let module = utils::compile(
        r#"
        mod m;

        struct Point { x: u64, y: u64 }

        fn sink(p: Point) { let Point { .. } = p; }
        pub fn give(p: Point) { sink(p); }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn param_struct_consumed_before_return_passes() {
    // A param struct that is consumed by destructuring before Return — verifier must pass.
    let module = utils::compile(
        r#"
        mod m;

        struct Point { x: u64, y: u64 }

        pub fn consume(p: Point) { let Point { .. } = p; }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn struct_load_consumes_slot() {
    // Structs use move semantics — loading a slot consumes it.
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.return_type = Some(Type::Struct("Point".to_string()));
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Store(0),
        Instruction::Load(0), // moves Point — slot 0 dead
        Instruction::Return,  // returns the Point on the stack
    ];
    utils::verify_ok(&module);
}

//
// ─── UnconsumedStruct ───
//

#[test]
fn struct_unconsumed_in_local_slot_rejected() {
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Store(0),
        Instruction::Return, // slot 0 still holds a live Point — resource leak
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UnconsumedStruct { slot: 0, .. }))
    );
}

#[test]
fn param_struct_moved_to_local_slot_rejected() {
    // The compiler catches this pattern (fn lose(p: Point) { let q = p; }), but the
    // verifier must also reject it as a defence-in-depth check.
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("p".to_string(), Type::Struct("Point".to_string()))];
    func.local_count = 2;
    func.code = vec![
        Instruction::Load(0),  // move Point out of param slot 0
        Instruction::Store(1), // into non-param local slot 1
        Instruction::Return,   // slot 1 still live — unconsumed struct
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnconsumedStruct { function, slot }
            if function == "dummy" && *slot == 1
        )),
        "expected UnconsumedStruct(dummy, slot=1), got: {errs:?}"
    );
}

#[test]
fn struct_from_unpacked_tuple_unconsumed_at_return_rejected() {
    // `make_pair` returns (Point, u64). `lose_from_tuple` unpacks the tuple but
    // never returns/consumes the Point — must trigger UnconsumedStruct.
    let module = utils::compile(
        r#"
        mod m;

        struct Point { x: u64, y: u64 }

        fn make_pair(p: Point) -> (Point, u64) {
            let v = p.x;
            (p, v)
        }
        pub fn lose_from_tuple(p: Point) {
            let (q, _v) = make_pair(p);
        }
    "#,
    );
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::UnconsumedStruct { function, .. }
            if function == "lose_from_tuple"
        )),
        "expected UnconsumedStruct(lose_from_tuple), got: {errs:?}"
    );
}

//
// ─── UseAfterMove ───
//

#[test]
fn struct_use_after_move_rejected() {
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.return_type = Some(Type::Struct("Point".to_string()));
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Store(0),
        Instruction::Load(0), // first load — consumes slot 0
        Instruction::Load(0), // second load — UseAfterMove
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UseAfterMove { slot: 0, .. }))
    );
}

//
// ─── Pop / Dup on struct ───
//

#[test]
fn pop_on_struct_rejected() {
    let mut module = struct_module();
    module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap()
        .code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Pop, // rejected — structs have move semantics
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::PopOnStruct { function, .. } if function == "dummy"
    )));
}

#[test]
fn dup_on_struct_rejected() {
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.return_type = Some(Type::Struct("Point".to_string()));
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Dup, // rejected — structs have move semantics
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(errs.iter().any(|e| matches!(
        e,
        VerificationError::DupOnStruct { function, .. } if function == "dummy"
    )));
}

//
// ─── Eq / Ne on linear type ───
//

#[test]
fn eq_on_primitives_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn f(a: u64, b: u64) -> bool { a == b }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn eq_on_primitive_tuple_passes() {
    let module = utils::compile(
        r#"
        mod m;

        fn pair() -> (u64, bool) { (1, true) }

        fn f() -> bool { pair() == pair() }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn eq_on_struct_rejected() {
    let mut module = struct_module();
    module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap()
        .code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
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
        Instruction::Eq, // rejected — structs have move semantics
        Instruction::Pop,
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::EqOnLinearType { function, .. } if function == "dummy"
        )),
        "expected EqOnLinearType, got: {errs:?}"
    );
}

#[test]
fn ne_on_struct_rejected() {
    let mut module = struct_module();
    module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap()
        .code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
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
        Instruction::Ne, // rejected — structs have move semantics
        Instruction::Pop,
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::EqOnLinearType { function, .. } if function == "dummy"
        )),
        "expected EqOnLinearType, got: {errs:?}"
    );
}

#[test]
fn eq_struct_on_right_rejected() {
    // struct is the right operand (pushed second); primitive is on the left
    let mut module = struct_module();
    module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap()
        .code = vec![
        Instruction::PushU64(99), // left operand — primitive
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        }, // right operand — struct
        Instruction::Eq, // rejected — right operand has move semantics
        Instruction::Pop,
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::EqOnLinearType { function, .. } if function == "dummy"
        )),
        "expected EqOnLinearType, got: {errs:?}"
    );
}

#[test]
fn eq_on_tuple_with_struct_rejected() {
    // Craft bytecode directly — compiler rejects this before it reaches the verifier,
    // so we bypass it to test the verifier layer independently.
    let mut module = struct_module();
    module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap()
        .code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::PushU64(0),
        Instruction::MakeTuple(2), // (Point, u64) — contains a linear type
        Instruction::PushU64(3),
        Instruction::PushU64(4),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::PushU64(0),
        Instruction::MakeTuple(2),
        Instruction::Eq, // rejected — tuple contains a struct
        Instruction::Pop,
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(e, VerificationError::EqOnLinearType { function, .. } if function == "dummy")),
        "expected EqOnLinearType, got: {errs:?}"
    );
}

//
// ─── SlotOverwrite ───
//

#[test]
fn struct_slot_overwrite_rejected() {
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Store(0),
        Instruction::PushU64(3),
        Instruction::PushU64(4),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Store(0), // rejected — slot 0 still holds a live struct
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::SlotOverwrite { slot: 0, .. }))
    );
}

//
// ─── UnpackStruct ───
//

#[test]
fn struct_unpack_passes() {
    // let Point { x, y } = p; binds both fields and consumes the struct
    let module = utils::compile(
        r#"
        mod m;

        struct Point { x: u64, y: u64 }

        pub fn sum(p: Point) -> u64 {
            let Point { x, y } = p;
            x + y
        }
    "#,
    );
    utils::verify_ok(&module);
}

#[test]
fn struct_unpack_consumes_slot() {
    // After UnpackStruct the source struct slot is consumed.
    // Emitting a second Load of that slot should produce UseAfterMove.
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 3;
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Store(0),
        Instruction::Load(0), // moves Point out of slot 0 onto stack
        Instruction::UnpackStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Store(1), // store x
        Instruction::Store(2), // store y
        Instruction::Load(0),  // use-after-move: slot 0 already consumed
        Instruction::Pop,
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UseAfterMove { slot: 0, .. })),
        "expected UseAfterMove for slot 0 after UnpackStruct, got: {errs:?}"
    );
}

//
// ─── Struct-typed field access ───
//
// LoadField / GetField on a struct-typed field creates an alias of the field
// value while the parent struct stays in its slot — a linearity violation.
// StoreField into a struct-typed field implicitly drops the old value.
// Both are forbidden and must be caught here as a defence-in-depth check
// (the compiler already rejects them, but hand-crafted bytecode could bypass that).
//

#[test]
fn load_field_struct_typed_rejected() {
    // Tamper: fn dummy(o: Outer) { LoadField(0, ["inner"]); Pop; Return; }
    // LoadField extracts the `inner: Inner` field without consuming `o`.
    // Since Inner is a struct type, this creates an alias — forbidden.
    let mut module = nested_struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("o".to_string(), Type::Struct("Outer".to_string()))];
    func.local_count = 1;
    func.code = vec![
        Instruction::LoadField(0, vec!["inner".to_string()]), // loads Inner — forbidden
        Instruction::Pop,
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::StructTypedFieldLoaded { field, .. } if field == "inner")),
        "expected StructTypedFieldLoaded for 'inner', got: {errs:?}"
    );
}

#[test]
fn load_field_primitive_field_passes() {
    // LoadField on a primitive (u64) field is always valid.
    let mut module = nested_struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("o".to_string(), Type::Struct("Outer".to_string()))];
    func.local_count = 1;
    func.return_type = Some(Type::U64);
    func.code = vec![
        Instruction::LoadField(0, vec!["amount".to_string()]), // loads u64 — OK
        Instruction::Return,
    ];
    // But `o` is still live at Return — add UnpackStruct to consume it first.
    // This test only verifies that LoadField itself doesn't fire StructTypedFieldLoaded.
    let errs = utils::verify_errors(&module);
    assert!(
        !errs
            .iter()
            .any(|e| matches!(e, VerificationError::StructTypedFieldLoaded { .. })),
        "LoadField on primitive field must not fire StructTypedFieldLoaded, got: {errs:?}"
    );
}

#[test]
fn store_field_struct_typed_rejected() {
    // Tamper: fn dummy(o: Outer) { PushU64(0); StoreField(0, ["inner"]); Return; }
    // StoreField into a struct-typed field would implicitly drop the old Inner value.
    let mut module = nested_struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("o".to_string(), Type::Struct("Outer".to_string()))];
    func.local_count = 1;
    func.code = vec![
        Instruction::PushU64(0),
        Instruction::StoreField(0, vec!["inner".to_string()]), // writes to struct-typed field — forbidden
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::StructTypedFieldWritten { field, .. } if field == "inner")),
        "expected StructTypedFieldWritten for 'inner', got: {errs:?}"
    );
}

#[test]
fn get_field_drops_linear_field_rejected() {
    // GetField("amount") on Outer { inner: Inner, amount: u64 } — the u64 result is
    // fine, but inner: Inner would be silently dropped — a linearity violation.
    let mut module = nested_struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("o".to_string(), Type::Struct("Outer".to_string()))];
    func.local_count = 1;
    func.return_type = Some(Type::U64);
    func.code = vec![
        Instruction::Load(0),                        // move Outer onto stack
        Instruction::GetField("amount".to_string()), // u64 result, but inner: Inner dropped
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter().any(|e| matches!(
            e,
            VerificationError::GetFieldDropsLinearField { type_name, linear_field, .. }
                if type_name == "Outer" && linear_field == "inner"
        )),
        "expected GetFieldDropsLinearField for 'inner' on Outer, got: {errs:?}"
    );
}

#[test]
fn get_field_struct_typed_rejected() {
    // Tamper: load Outer onto the stack, then GetField("inner").
    // GetField consumes the struct from the stack and extracts the field — but
    // for a struct-typed field this violates move semantics (other fields are dropped).
    let mut module = nested_struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.params = vec![("o".to_string(), Type::Struct("Outer".to_string()))];
    func.local_count = 1;
    func.code = vec![
        Instruction::Load(0), // move Outer onto stack (slot 0 → dead)
        Instruction::GetField("inner".to_string()), // extracts Inner — forbidden
        Instruction::Pop,
        Instruction::Return,
    ];
    let errs = utils::verify_errors(&module);
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::StructTypedFieldLoaded { field, .. } if field == "inner")),
        "expected StructTypedFieldLoaded for GetField on 'inner', got: {errs:?}"
    );
}

//
// ─── Utility functions ───
//

fn struct_module() -> meow_vm_types::module::Module {
    utils::compile(
        r#"
        mod m;

        pub struct Point { x: u64, y: u64 }

        fn dummy() { return; }
    "#,
    )
}

fn nested_struct_module() -> meow_vm_types::module::Module {
    utils::compile(
        r#"
        mod m;

        pub struct Inner { value: u64 }
        pub struct Outer { inner: Inner, amount: u64 }

        fn dummy() { return; }
    "#,
    )
}
