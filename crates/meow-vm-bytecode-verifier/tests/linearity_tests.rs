mod utils;
use utils::*;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::{bytecode::Instruction, types::Type};

//
// ─── Happy path: struct consumed by a function call ───
//

#[test]
fn struct_consumed_by_call_passes() {
    // A struct loaded from its slot and passed as an argument to a function is
    // consumed — no UnconsumedStruct should fire at Return.
    let module = compile(
        r#"
        mod m;
        struct Point { x: u64, y: u64 }
        fn sink(p: Point) {}
        pub fn give(p: Point) { sink(p); }
    "#,
    );
    verify_ok(&module, &no_deps());
}

//
// ─── Param struct alive at Return (mutated in place) ───
//

#[test]
fn param_struct_alive_at_return_passes() {
    // A param struct remaining in its original slot at Return is the
    // "mutated in place" path — the effects system writes it back.
    let module = compile(
        r#"
        mod m;
        struct Point { x: u64, y: u64 }
        fn mutate(p: Point) { p.x = 42; return; }
    "#,
    );
    verify_ok(&module, &no_deps());
}

#[test]
fn param_struct_moved_to_local_slot_at_return_rejected() {
    // A param struct consumed from its slot (Load) and stored in a non-param
    // local slot without being returned is an unconsumed struct.
    let mut module = compile(
        r#"
        mod m;
        struct Point { x: u64, y: u64 }
        fn dummy(p: Point) { return; }
    "#,
    );
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 2;
    func.code = vec![
        Instruction::Load(0),  // move Point out of param slot 0
        Instruction::Store(1), // into non-param local slot 1
        Instruction::Return,   // slot 1 still live — unconsumed struct
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UnconsumedStruct { slot: 1, .. })),
        "expected UnconsumedStruct for slot 1, got: {errs:?}"
    );
}

#[test]
fn compiled_lose_function_rejected() {
    // Compile exactly: fn lose(p: Point) { let q = p; }
    // The compiler emits Load(0)/Store(1) for `let q = p`, leaving
    // the Point alive in slot 1 at Return — the verifier must catch it.
    let module = compile(
        r#"
        mod m;
        struct Point { x: u64, y: u64 }
        pub fn lose(p: Point) { let q = p; }
    "#,
    );
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UnconsumedStruct { .. })),
        "expected UnconsumedStruct, got: {errs:?}"
    );
}

#[test]
fn struct_from_unpacked_tuple_unconsumed_at_return_rejected() {
    // `make_pair` returns (Point, u64). `lose_from_tuple` unpacks the tuple but
    // never returns/consumes the Point — must trigger UnconsumedStruct.
    let module = compile(
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
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UnconsumedStruct { .. })),
        "expected UnconsumedStruct when struct from unpacked tuple is not consumed, got: {errs:?}"
    );
}

//
// ─── Struct: move semantics ───
//

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
    verify_ok(&module, &no_deps());
}

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
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UseAfterMove { slot: 0, .. }))
    );
}

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
        Instruction::Pop, // rejected — structs are linear
        Instruction::Return,
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::PopOnStruct { .. }))
    );
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
        Instruction::Dup, // rejected — structs are linear
        Instruction::Return,
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::DupOnStruct { .. }))
    );
}

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
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::SlotOverwrite { slot: 0, .. }))
    );
}

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
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UnconsumedStruct { slot: 0, .. }))
    );
}

//
// ─── Struct destructuring (UnpackStruct) ───
//

#[test]
fn struct_unpack_passes() {
    // let Point { x, y } = p; binds both fields and consumes the struct
    let module = compile(
        r#"
        mod m;
        struct Point { x: u64, y: u64 }
        pub fn sum(p: Point) -> u64 {
            let Point { x, y } = p;
            x + y
        }
    "#,
    );
    verify_ok(&module, &no_deps());
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
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UseAfterMove { slot: 0, .. })),
        "expected UseAfterMove for slot 0 after UnpackStruct, got: {errs:?}"
    );
}

//
// ─── Utility functions ───
//

fn struct_module() -> meow_vm_types::module::Module {
    compile(
        r#"
        mod m;
        pub struct Point { x: u64, y: u64 }
        fn dummy() { return; }
    "#,
    )
}
