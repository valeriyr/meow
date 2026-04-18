mod utils;
use utils::*;

use meow_vm_bytecode_verifier::VerificationError;
use meow_vm_types::bytecode::Instruction;

//
// ─── Happy paths: transfer and destroy ───
//

#[test]
fn transfer_object_passes() {
    let module = compile(
        r#"
        mod m;
        object Coin { id: address, value: u64 }
        fn give(v: u64, owner: address) {
            let c = Coin { id: meow_vm_fresh_id(), value: v };
            meow_vm_transfer(c, owner);
            return;
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

#[test]
fn destroy_object_passes() {
    let module = compile(
        r#"
        mod m;
        object Coin { id: address, value: u64 }
        fn burn(v: u64) {
            let c = Coin { id: meow_vm_fresh_id(), value: v };
            meow_vm_destroy(c);
            return;
        }
    "#,
    );
    verify_ok(&module, &no_deps());
}

//
// ─── Use-after-move ───
//

#[test]
fn use_after_move_rejected() {
    let mut module = coin_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.code = vec![
        Instruction::PushAddress([0u8; 32].into()),
        Instruction::PushU64(0),
        Instruction::NewStruct {
            type_name: "Coin".to_string(),
            field_names: vec!["id".to_string(), "value".to_string()],
        },
        Instruction::Store(0),
        Instruction::Load(0), // first load — consumes the slot
        Instruction::Load(0), // use-after-move
        Instruction::Return,
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UseAfterMove { slot: 0, .. }))
    );
}

//
// ─── Pop on object ───
//

#[test]
fn pop_on_object_rejected() {
    let mut module = coin_module();
    module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap()
        .code = vec![
        Instruction::PushAddress([0u8; 32].into()),
        Instruction::PushU64(0),
        Instruction::NewStruct {
            type_name: "Coin".to_string(),
            field_names: vec!["id".to_string(), "value".to_string()],
        },
        Instruction::Pop, // dropping an object — forbidden
        Instruction::PushU64(0),
        Instruction::Return,
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::PopOnObject { .. }))
    );
}

//
// ─── Dup on object ───
//

#[test]
fn dup_on_object_rejected() {
    let mut module = coin_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 2;
    func.code = vec![
        Instruction::PushAddress([0u8; 32].into()),
        Instruction::PushU64(0),
        Instruction::NewStruct {
            type_name: "Coin".to_string(),
            field_names: vec!["id".to_string(), "value".to_string()],
        },
        Instruction::Dup, // duplicating an object — forbidden
        Instruction::Return,
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::DupOnObject { .. }))
    );
}

//
// ─── Object slot overwrite ───
//

#[test]
fn object_slot_overwrite_rejected() {
    let mut module = coin_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.code = vec![
        Instruction::PushAddress([0u8; 32].into()),
        Instruction::PushU64(0),
        Instruction::NewStruct {
            type_name: "Coin".to_string(),
            field_names: vec!["id".to_string(), "value".to_string()],
        },
        Instruction::Store(0),
        Instruction::PushAddress([0u8; 32].into()),
        Instruction::PushU64(0),
        Instruction::NewStruct {
            type_name: "Coin".to_string(),
            field_names: vec!["id".to_string(), "value".to_string()],
        },
        Instruction::Store(0), // overwrite live object slot — forbidden
        Instruction::PushU64(0),
        Instruction::Return,
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::ObjectSlotOverwrite { slot: 0, .. }))
    );
}

//
// ─── Unconsumed object ───
//

#[test]
fn unconsumed_object_at_return_rejected() {
    let mut module = coin_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 1;
    func.code = vec![
        Instruction::PushAddress([0u8; 32].into()),
        Instruction::PushU64(0),
        Instruction::NewStruct {
            type_name: "Coin".to_string(),
            field_names: vec!["id".to_string(), "value".to_string()],
        },
        Instruction::Store(0),
        Instruction::PushU64(0),
        Instruction::Return, // slot 0 still holds a live Coin — forbidden
    ];
    let errs = verify_errors(&module, &no_deps());
    assert!(
        errs.iter()
            .any(|e| matches!(e, VerificationError::UnconsumedObject { slot: 0, .. }))
    );
}

//
// ─── Struct: value semantics (no linearity constraints) ───
//

#[test]
fn struct_load_does_not_consume_slot() {
    // Structs use copy semantics — loading the same slot twice must pass.
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
        Instruction::Load(0), // first load — slot stays live for structs
        Instruction::Pop,
        Instruction::Load(0), // second load — must not be UseAfterMove
        Instruction::Pop,
        Instruction::Return,
    ];
    verify_ok(&module, &no_deps());
}

#[test]
fn pop_on_struct_passes() {
    // Dropping a struct value is allowed — no linearity constraint.
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
        Instruction::Pop, // allowed for structs
        Instruction::Return,
    ];
    verify_ok(&module, &no_deps());
}

#[test]
fn dup_on_struct_passes() {
    // Duplicating a struct value is allowed — copy semantics.
    let mut module = struct_module();
    let func = module
        .functions
        .iter_mut()
        .find(|f| f.name == "dummy")
        .unwrap();
    func.local_count = 2;
    func.code = vec![
        Instruction::PushU64(1),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Point".to_string(),
            field_names: vec!["x".to_string(), "y".to_string()],
        },
        Instruction::Dup, // allowed for structs
        Instruction::Pop,
        Instruction::Pop,
        Instruction::Return,
    ];
    verify_ok(&module, &no_deps());
}

#[test]
fn struct_slot_overwrite_passes() {
    // Overwriting a local slot that holds a struct is allowed.
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
        Instruction::Store(0), // overwrite — allowed for structs
        Instruction::Return,
    ];
    verify_ok(&module, &no_deps());
}

#[test]
fn struct_in_local_slot_at_return_passes() {
    // A struct remaining in a local slot at Return is not a resource leak.
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
        Instruction::Return, // slot 0 still holds a Point — allowed for structs
    ];
    verify_ok(&module, &no_deps());
}

//
// ─── Utility functions ───
//

fn coin_module() -> meow_vm_types::module::Module {
    compile(
        r#"
        mod m;
        object Coin { id: address, value: u64 }
        fn dummy() -> u64 { return 0; }
    "#,
    )
}

fn struct_module() -> meow_vm_types::module::Module {
    compile(
        r#"
        mod m;
        pub struct Point { pub x: u64, pub y: u64 }
        fn dummy() { return; }
    "#,
    )
}
