use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object,
        object_conversion::{object_to_vm_object_value, vm_object_value_to_object},
        object_decl_ref::ObjectDeclRef,
        object_owner::ObjectOwner,
        object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::meow_object::{MEOW_OBJECT_ID_FIELD_NAME, MeowObjectId},
};
use meow_vm_types::types::Value;

/// Fixed module address used in all tests.
const MODULE_ADDR: Address = Address::fill(0x01);
/// Fixed object owner address used in all tests.
const OWNER: Address = Address::fill(0xAA);
/// Fixed object id used in all tests.
const OBJECT_ID: Address = Address::fill(0xBB);

//
// ─── object_to_vm_object_value ───
//

#[test]
fn object_to_vm_injects_id_as_first_field() {
    let obj = make_object(OBJECT_ID, vec![("balance".to_string(), Value::U64(100))]);

    let val = object_to_vm_object_value(&obj).unwrap();

    let fields = vm_object_fields(&val);
    assert_eq!(fields[0].0, MEOW_OBJECT_ID_FIELD_NAME);
    // id is injected as meow_object::Id { inner: address }
    let expected = MeowObjectId::new(OBJECT_ID).to_qualified_vm_value();
    assert_eq!(fields[0].1, expected);
}

#[test]
fn object_to_vm_preserves_other_fields() {
    let obj = make_object(
        OBJECT_ID,
        vec![
            ("balance".to_string(), Value::U64(42)),
            ("flag".to_string(), Value::Bool(true)),
        ],
    );

    let val = object_to_vm_object_value(&obj).unwrap();

    let fields = vm_object_fields(&val);
    assert_eq!(fields.len(), 3); // id + balance + flag
    assert_eq!(fields[1], ("balance".to_string(), Value::U64(42)));
    assert_eq!(fields[2], ("flag".to_string(), Value::Bool(true)));
}

#[test]
fn object_to_vm_carries_type_name() {
    let obj = make_object(OBJECT_ID, vec![]);

    let val = object_to_vm_object_value(&obj).unwrap();

    assert_eq!(val.type_name(), "Foo");
}

#[test]
fn object_to_vm_fails_for_module_type() {
    let obj = Object::new(
        OBJECT_ID,
        ObjectOwner::Immutable,
        Digest::ZERO,
        ObjectVersion::ZERO,
        ObjectType::Module,
        vec![],
    );

    assert!(object_to_vm_object_value(&obj).is_err());
}

//
// ─── vm_object_value_to_object ───
//

#[test]
fn vm_to_object_strips_id_from_content() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![
            (
                MEOW_OBJECT_ID_FIELD_NAME.to_string(),
                Value::Address(OBJECT_ID.into()),
            ),
            ("balance".to_string(), Value::U64(99)),
        ],
    };

    let obj = make_vm_to_object(&val);

    let content_fields: Vec<(String, Value)> = bcs::from_bytes(obj.content()).unwrap();
    assert!(
        content_fields
            .iter()
            .all(|(n, _)| n != MEOW_OBJECT_ID_FIELD_NAME),
        "id must not appear in content"
    );
    assert_eq!(content_fields.len(), 1);
    assert_eq!(content_fields[0], ("balance".to_string(), Value::U64(99)));
}

#[test]
fn vm_to_object_sets_address_from_id_field() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![
            (
                MEOW_OBJECT_ID_FIELD_NAME.to_string(),
                Value::Address(OBJECT_ID.into()),
            ),
            ("balance".to_string(), Value::U64(1)),
        ],
    };

    let obj = make_vm_to_object(&val);

    assert_eq!(obj.address(), &Address::new(OBJECT_ID.into()));
}

#[test]
fn vm_to_object_fails_for_non_object_value() {
    assert!(
        vm_object_value_to_object(
            &Value::U64(42),
            ObjectOwner::Address(OWNER),
            Digest::ZERO,
            ObjectVersion::ZERO,
            &MODULE_ADDR,
        )
        .is_err()
    );
}

//
// ─── Round-trip ───
//

#[test]
fn round_trip_object_to_vm_and_back() {
    let original = make_object(
        OBJECT_ID,
        vec![
            ("balance".to_string(), Value::U64(500)),
            ("flag".to_string(), Value::Bool(false)),
        ],
    );

    let vm_val = object_to_vm_object_value(&original).unwrap();
    let restored = vm_object_value_to_object(
        &vm_val,
        *original.owner(),
        original.digest(),
        *original.version(),
        &MODULE_ADDR,
    )
    .unwrap();

    assert_eq!(restored.address(), original.address());
    assert_eq!(restored.content(), original.content());
    assert_eq!(restored.owner(), original.owner());
}

//
// ─── Utility functions ───
//

fn make_object(id: Address, fields: Vec<(String, Value)>) -> Object {
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let ident = Identifier::new("Foo").unwrap();
    let decl_ref = ObjectDeclRef::new(MODULE_ADDR, ident);
    Object::new(
        id,
        ObjectOwner::Address(OWNER),
        Digest::ZERO,
        ObjectVersion::ZERO,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn make_vm_to_object(val: &Value) -> Object {
    vm_object_value_to_object(
        val,
        ObjectOwner::Address(OWNER),
        Digest::ZERO,
        ObjectVersion::ZERO,
        &MODULE_ADDR,
    )
    .expect("conversion must succeed")
}

fn vm_object_fields(val: &Value) -> &Vec<(String, Value)> {
    match val {
        Value::Struct { fields, .. } => fields,
        _ => panic!("expected Value::Struct"),
    }
}
