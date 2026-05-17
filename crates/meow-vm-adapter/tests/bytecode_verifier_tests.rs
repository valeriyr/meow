//! Tests for the adapter-level bytecode verifier (object layout + ID freshness).
//!
//! These tests publish modules via `executor::execute` and verify that the adapter
//! verifier accepts valid modules and rejects structurally tampered ones.

use std::str::FromStr;

use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::{
        meow_coin::MEOW_COIN_MODULE_ADDRESS,
        meow_object::{
            MEOW_OBJECT_ID_FIELD_NAME, MEOW_OBJECT_MODULE_ADDRESS, MEOW_OBJECT_MODULE_PATH,
        },
    },
    transaction::{
        Transaction, execution_result::ExecutionStatus, transaction_type::TransactionType,
    },
};
use meow_vm_adapter::{Value, builder, executor, external_context::ExternalContext};
use meow_vm_types::types::Type;

//
// ─── Object layout checks ───
//

/// Source with an object struct that has a correct `id: meow_object::Id` first field
/// and a simple creation function. Used across multiple tests.
const OBJECT_SRC: &str = r#"
    mod token_test;

    use meow_object@0x01;

    pub struct Token { id: meow_object::Id, amount: u64 }

    pub fn create(amount: u64) {
        let t = Token { id: meow_vm_fresh_id(), amount: amount };
        meow_vm_transfer(t, meow_vm_sender());
    }
"#;

/// Source with an object struct only (no construction function), used for layout tamper
/// tests where the function bytecode must not diverge from the struct definition.
const LAYOUT_STRUCT_ONLY_SRC: &str = r#"
    mod layout_test;

    use meow_object@0x01;

    pub struct Token { id: meow_object::Id, amount: u64 }
"#;

#[test]
fn object_with_correct_id_field_passes() {
    let meow_object_module = build_meow_object();
    let module = builder::build(
        OBJECT_SRC,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![make_meow_object_dep(&meow_object_module)],
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
    let meow_object_module = build_meow_object();
    let mut module = builder::build(
        LAYOUT_STRUCT_ONLY_SRC,
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
        vec![make_meow_object_dep(&meow_object_module)],
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

            pub fn noop(c: Config) -> u64 { c.value }
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

//
// ─── ID freshness checks ───
//

#[test]
fn object_created_with_stored_fresh_id_passes() {
    // Fresh id stored in a local variable first, then used — freshness tag must
    // propagate through Store/Load, so this must still pass.
    let meow_object_module = build_meow_object();
    let module = builder::build(
        r#"
            mod stored_id_test;

            use meow_object@0x01;

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
        vec![make_meow_object_dep(&meow_object_module)],
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
    let meow_object_module = build_meow_object();
    let module = builder::build(
        r#"
            mod param_id_test;

            use meow_object@0x01;

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
        vec![make_meow_object_dep(&meow_object_module)],
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
    let meow_object_module = build_meow_object();
    let module = builder::build(
        r#"
            mod reuse_id_test;

            use meow_object@0x01;

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
        vec![make_meow_object_dep(&meow_object_module)],
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
    let meow_object_module = build_meow_object();
    let module = builder::build(
        r#"
            mod tuple_id_test;

            use meow_object@0x01;

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
        vec![make_meow_object_dep(&meow_object_module)],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("non-fresh id")),
        "object constructed from tuple-unpacked id must be rejected, got: {:?}",
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

    let meow_object_module = build_meow_object();
    let mut module = builder::build(
        r#"
            mod branch_merge_test;

            use meow_object@0x01;

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
        Instruction::Call("meow_vm_fresh_id".to_string()),
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
        vec![make_meow_object_dep(&meow_object_module)],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("non-fresh id")),
        "non-fresh id on one branch must be rejected after conservative merge, got: {:?}",
        result.status()
    );
}

#[test]
fn module_without_object_types_skips_freshness_check() {
    // No object types → freshness check is skipped entirely → must pass.
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

//
// ─── Object-as-field-type checks ───
//

#[test]
fn object_type_as_local_struct_field_fails() {
    // `Inner` is object-shaped; `Outer` nests it as a field — must be rejected.
    let meow_object_module = build_meow_object();
    let module = builder::build(
        r#"
            mod nested_test;

            use meow_object@0x01;

            pub struct Inner { id: meow_object::Id, value: u64 }
            pub struct Outer { inner: Inner, amount: u64 }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![make_meow_object_dep(&meow_object_module)],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("objects cannot be nested")),
        "struct with object-type field must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn cross_module_object_type_as_struct_field_fails() {
    // `Token` (from dep at 0x02) is object-shaped; `Wrapper` nests it as a field — must be rejected.
    let meow_object_module = build_meow_object();
    let dep_addr = Address::from_str("0x02").unwrap();

    let dep_module = builder::build(
        r#"
            mod dep_with_object;

            use meow_object@0x01;

            pub struct Token { id: meow_object::Id, balance: u64 }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("dep must compile");

    let module = builder::build(
        r#"
            mod wrapper_test;

            use dep_with_object@0x02;

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
        vec![make_meow_object_dep(&meow_object_module), dep_obj],
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
// ─── Object id field immutability checks ───
//

#[test]
fn object_id_field_mutation_rejected() {
    // Tamper: inject StoreField(0, "id") into a function that receives an object param.
    // This simulates bytecode that attempts to overwrite an object's identity.
    use meow_vm_types::bytecode::Instruction;

    let meow_object_module = build_meow_object();
    let mut module = builder::build(
        r#"
            mod mutation_test;

            use meow_object@0x01;

            pub struct Token { id: meow_object::Id, value: u64 }

            pub fn noop(t: Token) -> Token { t }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");

    // Tamper: read the id field then write it back.
    // LoadField keeps the slot Live; StoreField type-checks correctly (Id → Id);
    // this passes the language verifier but must be caught by the adapter verifier.
    let noop = module
        .functions
        .iter_mut()
        .find(|f| f.name == "noop")
        .unwrap();
    noop.code = vec![
        Instruction::LoadField(0, vec![MEOW_OBJECT_ID_FIELD_NAME.to_string()]), // borrows id: push @0x1::Id
        Instruction::StoreField(0, vec![MEOW_OBJECT_ID_FIELD_NAME.to_string()]), // writes it back — adapter must reject
        Instruction::Load(0),
        Instruction::Return,
    ];

    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![make_meow_object_dep(&meow_object_module)],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("id field is immutable")),
        "mutation of object id field must be rejected, got: {:?}",
        result.status()
    );
}

#[test]
fn object_id_mutation_of_locally_created_struct_rejected() {
    // Object created inside the function body (not a param) and stored in a local slot.
    // StoreField(1, ["id"]) on that slot must be caught even though slot 1 is not a param.
    use meow_vm_types::bytecode::Instruction;

    let meow_object_module = build_meow_object();
    let mut module = builder::build(
        r#"
            mod local_create_mutate_test;

            use meow_object@0x01;

            pub struct Token { id: meow_object::Id, amount: u64 }

            pub fn create(amount: u64) -> Token {
                let t = Token { id: meow_vm_fresh_id(), amount: amount };
                t
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");

    // Tamper: after creating Token and storing in slot 1, read and write back the id
    // field via LoadField+StoreField. LoadField keeps the slot live so the language
    // verifier passes (types match). The adapter catches StoreField(1, ["id"]) because
    // slot 1 holds an object-shaped struct.
    let create = module
        .functions
        .iter_mut()
        .find(|f| f.name == "create")
        .unwrap();
    create.local_count = 2;
    create.code = vec![
        Instruction::Call("meow_vm_fresh_id".to_string()),
        Instruction::Load(0),
        Instruction::NewStruct {
            type_name: "Token".to_string(),
            field_names: vec![MEOW_OBJECT_ID_FIELD_NAME.to_string(), "amount".to_string()],
        },
        Instruction::Store(1),
        Instruction::LoadField(1, vec![MEOW_OBJECT_ID_FIELD_NAME.to_string()]),
        Instruction::StoreField(1, vec![MEOW_OBJECT_ID_FIELD_NAME.to_string()]),
        Instruction::Load(1),
        Instruction::Return,
    ];

    let result = publish(
        bcs::to_bytes(&module).unwrap(),
        vec![make_meow_object_dep(&meow_object_module)],
    );
    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("id field is immutable")),
        "mutation of locally-created object's id field must be rejected, got: {:?}",
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

//
// ─── Helpers ───
//

const SENDER: Address = Address::fill(0xAA);
const GAS_ADDR: Address = Address::fill(0xBB);
const GAS_BALANCE: u64 = 1_000_000;

fn build_meow_object() -> meow_vm_types::module::Module {
    builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[]).expect("meow_object must compile")
}

fn make_meow_object_dep(module: &meow_vm_types::module::Module) -> Object {
    Object::fresh_module(
        MEOW_OBJECT_MODULE_ADDRESS,
        Digest::ZERO,
        bcs::to_bytes(module).expect("meow_object must serialize"),
    )
}

fn make_gas_coin_object() -> Object {
    let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(GAS_BALANCE))];
    let content = bcs::to_bytes(&fields).unwrap();
    let decl_ref = ObjectDeclRef::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("MeowCoin").unwrap(),
    );
    Object::new(
        GAS_ADDR,
        ObjectOwner::Address(SENDER),
        Digest::ZERO,
        ObjectVersion::ONE,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn publish(
    module_bytes: Vec<u8>,
    mut dep_objects: Vec<Object>,
) -> meow_types::transaction::execution_result::ExecutionResult {
    let gas_obj = make_gas_coin_object();
    let tx = Transaction::new(
        SENDER,
        gas_obj.object_ref(),
        TransactionType::MeowModulePublish(module_bytes),
    );
    dep_objects.push(gas_obj);
    executor::execute(&tx, dep_objects, &ExternalContext::default()).unwrap()
}
