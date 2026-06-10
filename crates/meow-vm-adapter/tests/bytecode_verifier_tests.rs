//! Tests for the adapter-level bytecode verifier (object layout, ID freshness,
//! and transfer-argument type).
//!
//! These tests publish modules via `executor::execute` and verify that the adapter
//! verifier accepts valid modules and rejects structurally tampered ones.

mod utils;

use meow_types::{
    address::Address,
    config,
    digest::Digest,
    object::Object,
    system_framework::meow_object::{MEOW_OBJECT_ID_FIELD_NAME, MEOW_OBJECT_MODULE_ADDRESS},
    transaction::{
        Transaction, execution_result::ExecutionStatus, transaction_type::TransactionType,
    },
};
use meow_vm_adapter::{builder, executor, external_context::ExternalContext};
use meow_vm_types::types::Type;
use std::str::FromStr;

//
// ─── Object layout checks ───
//

#[test]
fn object_with_correct_id_field_passes() {
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod token_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            pub fn create(amount: u64) {
                let t = Token { id: meow_vm_fresh_id(), amount: amount };
                meow_vm_transfer(t, meow_vm_sender());
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "object with valid id field must publish successfully, got: {:?}",
        result.status()
    );
}

#[test]
fn struct_with_address_id_field_is_plain_struct() {
    // A struct whose first field is `id: address` (not `id: meow_object::Id`) is treated
    // as a plain struct — it is not an object and must pass adapter verification.
    let meow_object_module = meow_framework::meow_object_module();
    let mut module = builder::build(
        r#"
            mod layout_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    module
        .structs
        .iter_mut()
        .find(|s| s.name == "Token")
        .unwrap()
        .fields[0]
        .ty = Type::Address;

    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "struct with id: address is a plain struct and must pass, got: {:?}",
        result.status()
    );
}

#[test]
fn plain_struct_without_id_field_passes() {
    // Non-object structs have no id constraint — adapter verifier must not flag them.
    let module = builder::build(
        r#"
            mod plain_test;

            pub struct Config { value: u64 }

            pub fn noop(c: Config) -> u64 { let Config { value } = c; value }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "plain struct must pass adapter verification, got: {:?}",
        result.status()
    );
}

#[test]
fn trivial_module_without_objects_passes() {
    // Baseline: a module with no structs at all (hence no object types) verifies
    // cleanly — the layout and freshness checks have nothing to act on.
    let module = builder::build(
        r#"
            mod no_objects;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "module without objects must pass adapter verification, got: {:?}",
        result.status()
    );
}

#[test]
fn non_object_struct_id_field_write_passes() {
    // A plain struct with a field named `id` (but not object-shaped) may write to it freely.
    let module = builder::build(
        r#"
            mod plain_id_test;

            pub struct Receipt { id: u64, amount: u64 }

            pub fn set_id(r: Receipt, new_id: u64) -> Receipt {
                r.id = new_id;
                r
            }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "plain struct id field write must pass, got: {:?}",
        result.status()
    );
}

#[test]
fn id_field_not_first_is_rejected() {
    // The compiler accepts `id: meow_object::Id` in any position, so the adapter is
    // the line of defense: a struct with id in a non-first position is neither a
    // valid object (object-ness requires id first) nor a clean plain struct, and
    // must be rejected at publish rather than silently demoted.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod id_pos_test;

            use meow_object@0x10;

            pub struct Token { amount: u64, id: meow_object::Id }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");

    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not the first field")),
        "id field in non-first position must be rejected, got: {:?}",
        result.status()
    );
}

//
// ─── Object-as-field-type checks ───
//

#[test]
fn object_type_as_local_struct_field_fails() {
    // `Inner` is object-shaped; `Outer` nests it as a field — must be rejected.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod nested_test;

            use meow_object@0x10;

            pub struct Inner { id: meow_object::Id, value: u64 }
            pub struct Outer { inner: Inner, amount: u64 }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("objects cannot be nested")),
        "struct with object-type field must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn cross_module_object_type_as_struct_field_fails() {
    // `Token` (from dep at 0xFD) is object-shaped; `Wrapper` nests it as a field — must be rejected.
    let meow_object_module = meow_framework::meow_object_module();
    let dep_addr = Address::from_str("0xFD").unwrap();

    let dep_module = builder::build(
        r#"
            mod dep_with_object;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, balance: u64 }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("dep must compile");

    let module = builder::build(
        r#"
            mod wrapper_test;

            use dep_with_object@0xFD;

            pub struct Wrapper { token: dep_with_object::Token, extra: u64 }
        "#,
        &[
            (dep_addr, &dep_module),
            (MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module),
        ],
    )
    .expect("must compile");

    let dep_obj = Object::fresh_module(dep_addr, Digest::ZERO, bcs::to_bytes(&dep_module).unwrap());
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object(), dep_obj],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("objects cannot be nested")),
        "struct with cross-module object-type field must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn plain_struct_as_field_type_passes() {
    // Non-object structs are allowed as field types.
    let module = builder::build(
        r#"
            mod plain_nested;

            pub struct Point { x: u64, y: u64 }
            pub struct Line { start: Point, end: Point }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "plain struct as field type must pass, got: {:?}",
        result.status()
    );
}

//
// ─── ID freshness checks ───
//

#[test]
fn object_created_with_stored_fresh_id_passes() {
    // Fresh id stored in a local variable first, then used — freshness tag must
    // propagate through Store/Load, so this must still pass.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod stored_id_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            pub fn create(amount: u64) {
                let id = meow_vm_fresh_id();
                let t = Token { id: id, amount: amount };
                meow_vm_transfer(t, meow_vm_sender());
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "object created via stored fresh id must pass, got: {:?}",
        result.status()
    );
}

#[test]
fn object_created_from_parameter_id_fails() {
    // A function whose `id` argument is a caller-supplied `meow_object::Id`.
    // The types are correct (passes language verifier) but the id is not fresh
    // (originated from a parameter, not meow_vm_fresh_id) — adapter verifier
    // must reject it.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod param_id_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            pub fn create_with_id(id: meow_object::Id, amount: u64) {
                let t = Token { id: id, amount: amount };
                meow_vm_transfer(t, meow_vm_sender());
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("non-fresh id")),
        "object constructed from parameter id must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn object_created_from_unpacked_id_fails() {
    // A function that unpacks an object to extract its id, then tries to reuse
    // that id to construct a new object. The id from UnpackStruct is not fresh
    // (all unpacked fields are tagged Fresh::Other) — adapter verifier must reject it.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod reuse_id_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            pub fn reuse_id(t: Token, new_amount: u64) {
                let Token { id, .. } = t;
                let t2 = Token { id: id, amount: new_amount };
                meow_vm_transfer(t2, meow_vm_sender());
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("non-fresh id")),
        "object constructed from unpacked id must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn object_created_from_tuple_unpacked_id_fails() {
    // get_id() wraps meow_vm_fresh_id in a tuple return. UnpackTuple marks every
    // element Fresh::Other — even the Id slot — so the caller cannot use it to
    // construct an object. NewStruct must fire ObjectIdNotFresh.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod tuple_id_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            fn get_id() -> (meow_object::Id, u64) { (meow_vm_fresh_id(), 42) }
            pub fn create_from_tuple() {
                let (id, amount) = get_id();
                let t = Token { id: id, amount: amount };
                meow_vm_transfer(t, meow_vm_sender());
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("non-fresh id")),
        "object constructed from tuple-unpacked id must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn object_created_from_local_call_id_fails() {
    // The id comes from a local helper that wraps meow_vm_fresh_id. Only the
    // native itself yields Fresh::Id; a function-call return is Fresh::Other, so
    // the object construction must be rejected — freshness must be direct.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod local_call_id_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            fn make_id() -> meow_object::Id { meow_vm_fresh_id() }

            pub fn create(amount: u64) -> Token {
                let t = Token { id: make_id(), amount: amount };
                t
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("non-fresh id")),
        "object constructed from a local-call id must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn object_id_freshness_not_guaranteed_on_all_branches_rejected() {
    // One branch calls meow_vm_fresh_id (Fresh::Id), the other calls a local helper
    // function that also produces a meow_object::Id but is Fresh::Other from the
    // caller's perspective. The conservative merge at the join point degrades
    // freshness to Other, so NewStruct must fire ObjectIdNotFresh.
    //
    // Using a local helper avoids struct params consumed asymmetrically (which would
    // trigger LivenessMergeConflict in the language verifier before the adapter runs).
    //
    // Bytecode layout (slots: 0=cond, 1=amount; local_count=3, slot 2=merged id):
    //   pc 0: Load(0)                      push cond
    //   pc 1: JumpIfNot(3)                 false → pc 4; fall-through = pc 2
    //   pc 2: Call("meow_vm_fresh_id")     true branch: Fresh::Id
    //   pc 3: Jump(2)                      → pc 5 (merge point)
    //   pc 4: Call("make_id")              false branch: Fresh::Other (local call)
    //   pc 5: Store(2)                     merged id — Fresh::Other after merge
    //   pc 6: Load(2)                      push id (Other)
    //   pc 7: Load(1)                      push amount
    //   pc 8: NewStruct { Token, [id, amount] }  → ObjectIdNotFresh
    //   pc 9: Return
    use meow_vm_types::bytecode::Instruction;

    let meow_object_module = meow_framework::meow_object_module();
    let mut module = builder::build(
        r#"
            mod branch_merge_test;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            fn make_id() -> meow_object::Id { meow_vm_fresh_id() }
            pub fn create(cond: bool, amount: u64) -> Token {
                let t = Token { id: meow_vm_fresh_id(), amount: amount };
                t
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");

    let create = module
        .functions
        .iter_mut()
        .find(|f| f.name == "create")
        .unwrap();
    create.local_count = 3;
    create.code = vec![
        Instruction::Load(0),
        Instruction::JumpIfNot(3),
        Instruction::Call(config::NATIVE_FN_FRESH_ID.to_string()),
        Instruction::Jump(2),
        Instruction::Call("make_id".to_string()),
        Instruction::Store(2),
        Instruction::Load(2),
        Instruction::Load(1),
        Instruction::NewStruct {
            type_name: "Token".to_string(),
            field_names: vec![MEOW_OBJECT_ID_FIELD_NAME.to_string(), "amount".to_string()],
        },
        Instruction::Return,
    ];

    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("non-fresh id")),
        "non-fresh id on one branch must be rejected after conservative merge, got: {:?}",
        result.status()
    );
}

//
// ─── Transfer object-type checks ───
//

#[test]
fn plain_struct_passed_to_transfer_fails() {
    // Config is not an object type — meow_vm_transfer must be caught at publish
    // time rather than aborting at runtime.
    let module = builder::build(
        r#"
            mod transfer_plain;

            pub struct Config { value: u64 }

            pub fn bad(c: Config, owner: address) {
                meow_vm_transfer(c, owner);
            }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not an on-chain object")),
        "transferring a non-object struct must be rejected at publish time, got: {:?}",
        result.status()
    );
}

#[test]
fn plain_struct_from_factory_passed_to_transfer_fails() {
    // Return-type tracking: make_config() returns Config; the verifier knows the
    // stack top is Config (not an object type) and must reject the transfer.
    let module = builder::build(
        r#"
            mod transfer_factory;

            pub struct Config { value: u64 }

            fn make_config(v: u64) -> Config { Config { value: v } }

            pub fn bad(owner: address) {
                let c = make_config(42);
                meow_vm_transfer(c, owner);
            }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not an on-chain object")),
        "transferring a plain struct from a factory must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn object_struct_via_param_passes_transfer_check() {
    // A function that receives an object-type struct and forwards it to
    // meow_vm_transfer — param-type tracking must recognise Token as an object.
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod transfer_param;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }

            pub fn forward(t: Token, owner: address) {
                meow_vm_transfer(t, owner);
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "transferring an object-type struct via param must pass, got: {:?}",
        result.status()
    );
}

#[test]
fn unpacked_plain_struct_passed_to_transfer_fails() {
    // A plain struct extracted via destructuring must still be caught at the
    // transfer site: UnpackStruct propagates each field's type tag, so `start`
    // (a non-object Point) is rejected rather than slipping through as untracked.
    let module = builder::build(
        r#"
            mod unpack_transfer;

            pub struct Point { x: u64, y: u64 }
            pub struct Line { start: Point, end: Point }

            pub fn bad(l: Line, owner: address) {
                let Line { start, end } = l;
                let Point { x, y } = end;
                meow_vm_transfer(start, owner);
            }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not an on-chain object")),
        "transferring an unpacked plain struct must be rejected at publish, got: {:?}",
        result.status()
    );
}

#[test]
fn object_from_mixed_tuple_passes_transfer_check() {
    // A factory returns (object, non-object). After unpacking, transferring the
    // OBJECT element must be accepted — the object's tag must survive the tuple
    // round-trip and land in the right slot (guards UnpackTuple element order: a
    // mis-ordered restore would tag the transferred value NonObject and reject).
    let meow_object_module = meow_framework::meow_object_module();
    let module = builder::build(
        r#"
            mod tuple_mixed;

            use meow_object@0x10;

            pub struct Token { id: meow_object::Id, amount: u64 }
            pub struct Config { value: u64 }

            fn make() -> (Token, Config) {
                let t = Token { id: meow_vm_fresh_id(), amount: 5 };
                let c = Config { value: 1 };
                (t, c)
            }

            pub fn run(owner: address) {
                let (a, b) = make();
                meow_vm_transfer(a, owner);
                let Config { value } = b;
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![meow_framework::meow_object_module_object()],
    );
    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "transferring the object element of a mixed tuple must pass, got: {:?}",
        result.status()
    );
}

#[test]
fn tuple_unpacked_plain_struct_passed_to_transfer_fails() {
    // A plain struct returned inside a tuple, then unpacked and transferred, must
    // still be caught: MakeTuple records each element's tag and UnpackTuple restores
    // it, so `p` (a non-object Point) is rejected at the transfer site.
    let module = builder::build(
        r#"
            mod tuple_transfer;

            pub struct Point { x: u64, y: u64 }

            fn make() -> (Point, u64) { (Point { x: 1, y: 2 }, 3) }

            pub fn bad(owner: address) {
                let (p, n) = make();
                meow_vm_transfer(p, owner);
            }
        "#,
        &[],
    )
    .expect("must compile");
    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not an on-chain object")),
        "transferring a plain struct unpacked from a tuple must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn non_object_struct_tag_survives_branch_merge_rejected() {
    // Both branches build the SAME non-object struct (Config) into the merge slot.
    // The conservative merge keeps StructTag::NonObject when both paths agree, so
    // transferring the merged value must still be rejected — a non-object cannot
    // slip past the transfer check by being produced on multiple branches.
    //
    // Hand-built because a struct held live across a control-flow join can't be
    // expressed in source (reassigning a live struct trips the language verifier's
    // SlotOverwrite / liveness-merge rules before the adapter runs).
    //
    // Bytecode (slots: 0=cond, 1=owner; local_count=3, slot 2=merged Config):
    //   pc 0: Load(0)                       push cond
    //   pc 1: JumpIfNot(4)                  false → pc 5
    //   pc 2: PushU64(1)                    true branch
    //   pc 3: NewStruct { Config, [value] }
    //   pc 4: Jump(3)                       → pc 7 (merge point)
    //   pc 5: PushU64(2)                    false branch
    //   pc 6: NewStruct { Config, [value] }
    //   pc 7: Store(2)                      merged Config — NonObject on both paths
    //   pc 8: Load(2)
    //   pc 9: Load(1)                       owner
    //   pc 10: Call("meow_vm_transfer")     → TransferNonObjectStruct
    //   pc 11: Return
    use meow_vm_types::bytecode::Instruction;

    let mut module = builder::build(
        r#"
            mod merge_tag_test;

            pub struct Config { value: u64 }

            pub fn bad(cond: bool, owner: address) {}
        "#,
        &[],
    )
    .expect("must compile");

    let bad = module
        .functions
        .iter_mut()
        .find(|f| f.name == "bad")
        .unwrap();
    bad.local_count = 3;
    bad.code = vec![
        Instruction::Load(0),
        Instruction::JumpIfNot(4),
        Instruction::PushU64(1),
        Instruction::NewStruct {
            type_name: "Config".to_string(),
            field_names: vec!["value".to_string()],
        },
        Instruction::Jump(3),
        Instruction::PushU64(2),
        Instruction::NewStruct {
            type_name: "Config".to_string(),
            field_names: vec!["value".to_string()],
        },
        Instruction::Store(2),
        Instruction::Load(2),
        Instruction::Load(1),
        Instruction::Call(config::NATIVE_FN_TRANSFER.to_string()),
        Instruction::Return,
    ];

    let result = publish(bcs::to_bytes(&module).unwrap(), vec![]);
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not an on-chain object")),
        "non-object struct merged from both branches must be rejected at transfer, got: {:?}",
        result.status()
    );
}

//
// ─── Helpers ───
//

fn publish(
    module_bytes: Vec<u8>,
    mut dep_objects: Vec<Object>,
) -> meow_types::transaction::execution_result::ExecutionResult {
    let gas_obj = utils::make_gas_coin_object();
    let transaction = Transaction::new(
        utils::SENDER,
        gas_obj.object_ref(),
        TransactionType::MeowModulePublish(module_bytes),
    );
    dep_objects.push(gas_obj);
    executor::execute(&transaction, dep_objects, &ExternalContext::default()).unwrap()
}
