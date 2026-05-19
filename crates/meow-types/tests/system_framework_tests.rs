use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_conversion, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner,
        object_version::ObjectVersion,
    },
    system_framework::{
        MEOW_SYSTEM_ADDRESS,
        meow_coin::{
            MEOW_COIN_BALANCE_BYTECODE_TYPE_NAME, MEOW_COIN_BALANCE_STRUCT_NAME,
            MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_MODULE_NAME, MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME,
            MEOW_COIN_OBJECT_NAME, MeowCoin, MeowCoinBalance, is_meow_coin, is_meow_coin_balance,
            is_meow_coin_object, is_meow_coin_object_decl_ref, meow_coin_balance_struct,
            meow_coin_object,
        },
        meow_object::{
            MEOW_OBJECT_ID_BYTECODE_TYPE_NAME, MEOW_OBJECT_ID_FIELD_NAME,
            MEOW_OBJECT_MODULE_ADDRESS, MeowObjectId, is_object_struct, object_address,
        },
    },
};
use meow_vm_types::{
    convert,
    types::{FieldDef, StructDef, Type, Value},
};

//
// ─── System address ───
//

#[test]
fn system_address_to_string() {
    assert_eq!(
        MEOW_SYSTEM_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}

//
// ─── MeowObjectId ───
//

#[test]
fn meow_object_module_address_to_string() {
    assert_eq!(
        MEOW_OBJECT_MODULE_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );
}

#[test]
fn meow_object_id_from_address_roundtrip() {
    let addr = Address::fill(0xCD);
    let id = MeowObjectId::from(addr);
    assert_eq!(Address::from(id), addr);
}

#[test]
fn meow_object_id_to_qualified_vm_value() {
    let addr = Address::fill(0x01);
    let val: Value = MeowObjectId::new(addr).into();
    assert_eq!(
        val,
        Value::Struct {
            type_name: MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string(),
            fields: vec![("inner".to_string(), Value::Address(addr.into()))],
        }
    );
}

#[test]
fn is_object_struct_positive() {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());
    let s = StructDef {
        name: "Foo".to_string(),
        is_public: true,
        fields: vec![
            FieldDef {
                name: MEOW_OBJECT_ID_FIELD_NAME.to_string(),
                ty: id_type,
            },
            FieldDef {
                name: "balance".to_string(),
                ty: Type::U64,
            },
        ],
    };
    assert!(is_object_struct(&s));
}

#[test]
fn is_object_struct_negative_wrong_first_field_name() {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());
    let s = StructDef {
        name: "Foo".to_string(),
        is_public: true,
        fields: vec![FieldDef {
            name: "not_id".to_string(),
            ty: id_type,
        }],
    };
    assert!(!is_object_struct(&s));
}

#[test]
fn is_object_struct_negative_wrong_first_field_type() {
    let s = StructDef {
        name: "Foo".to_string(),
        is_public: true,
        fields: vec![FieldDef {
            name: MEOW_OBJECT_ID_FIELD_NAME.to_string(),
            ty: Type::U64,
        }],
    };
    assert!(!is_object_struct(&s));
}

#[test]
fn is_object_struct_negative_empty() {
    assert!(!is_object_struct(&StructDef {
        name: "Foo".to_string(),
        is_public: true,
        fields: vec![],
    }));
}

#[test]
fn is_object_struct_negative_id_not_first_field() {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());
    let s = StructDef {
        name: "Foo".to_string(),
        is_public: true,
        fields: vec![
            FieldDef {
                name: "balance".to_string(),
                ty: Type::U64,
            },
            FieldDef {
                name: MEOW_OBJECT_ID_FIELD_NAME.to_string(),
                ty: id_type,
            },
        ],
    };
    assert!(!is_object_struct(&s));
}

#[test]
fn object_address_extracts_address_from_struct() {
    let addr = Address::fill(0x42);
    let val: Value = MeowCoin::new(addr, 0).into();
    assert_eq!(object_address(&val), Some(addr));
}

#[test]
fn object_address_returns_none_for_non_struct() {
    assert_eq!(object_address(&Value::U64(42)), None);
}

#[test]
fn object_address_returns_none_when_id_field_missing() {
    let val = Value::Struct {
        type_name: "Foo".to_string(),
        fields: vec![("balance".to_string(), Value::U64(42))],
    };
    assert_eq!(object_address(&val), None);
}

//
// ─── MeowCoin — identity ───
//

#[test]
fn meow_coin_module_address_to_string() {
    assert_eq!(
        MEOW_COIN_MODULE_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000010"
    );
}

#[test]
fn is_meow_coin_positive() {
    assert!(is_meow_coin(
        &MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_OBJECT_NAME
    ));
}

#[test]
fn is_meow_coin_negative() {
    assert!(!is_meow_coin(&MEOW_SYSTEM_ADDRESS, MEOW_COIN_OBJECT_NAME));
    assert!(!is_meow_coin(
        &MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MODULE_NAME
    ));
}

#[test]
fn object_decl_ref_is_meow_coin() {
    assert!(is_meow_coin_object_decl_ref(
        &test_meow_coin_object_decl_ref()
    ));
}

#[test]
fn object_decl_ref_is_not_meow_coin() {
    assert!(!is_meow_coin_object_decl_ref(&test_other_object_decl_ref()));
}

#[test]
fn object_is_meow_coin() {
    assert!(is_meow_coin_object(&test_meow_coin_object()));
}

#[test]
fn object_is_not_meow_coin() {
    assert!(!is_meow_coin_object(&test_other_object()));
}

//
// ─── MeowCoin — value conversion ───
//

#[test]
fn meow_coin_into_value_has_qualified_type_name() {
    let val: Value = MeowCoin::new(Address::fill(0x01), 100).into();
    assert_eq!(val.type_name(), MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME);

    let Value::Struct { fields, .. } = &val else {
        panic!("expected Struct")
    };
    let id_field = fields
        .iter()
        .find(|(n, _)| n == MEOW_OBJECT_ID_FIELD_NAME)
        .expect("id field must exist");
    assert_eq!(id_field.1.type_name(), MEOW_OBJECT_ID_BYTECODE_TYPE_NAME);
}

#[test]
fn round_trip_meow_coin() {
    let id = Address::fill(0xFFu8);
    let balance = 50;

    let coin: Value = MeowCoin::new(id, balance).into();
    let restored = convert::value_to_rust::<MeowCoin>(&coin).expect("must round-trip back");

    assert_eq!(restored.id().inner(), &id);
    assert_eq!(restored.balance(), balance);
}

//
// ─── meow_coin_object ───
//

#[test]
fn balance_from_object_reads_coin_balance() {
    assert_eq!(
        meow_coin_object::balance_from_object(&test_meow_coin_object()),
        Some(100)
    );
}

#[test]
fn balance_from_object_returns_none_for_non_coin() {
    assert_eq!(
        meow_coin_object::balance_from_object(&test_other_object()),
        None
    );
}

#[test]
fn balance_from_value_reads_coin_balance() {
    let coin: Value = MeowCoin::new(Address::fill(0x01), 77).into();
    assert_eq!(meow_coin_object::balance_from_value(&coin), Some(77));
}

#[test]
fn balance_from_value_returns_none_for_wrong_type() {
    let balance: Value = MeowCoinBalance::new(50).into();
    assert_eq!(meow_coin_object::balance_from_value(&balance), None);
}

#[test]
fn deduct_gas_reduces_balance() {
    let object = test_meow_coin_object();

    let updated_content =
        meow_coin_object::deduct_gas(&object, 30).expect("must get updated content");

    let fields: Vec<(String, Value)> = bcs::from_bytes(&updated_content).unwrap();

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0], ("balance".to_owned(), Value::U64(70)));
}

#[test]
fn deduct_gas_on_non_coin_object_returns_none() {
    assert!(meow_coin_object::deduct_gas(&test_other_object(), 10).is_none());
}

#[test]
fn deduct_gas_underflow_saturates_to_zero() {
    let updated_content = meow_coin_object::deduct_gas(&test_meow_coin_object(), 200)
        .expect("must get updated content");

    let fields: Vec<(String, Value)> = bcs::from_bytes(&updated_content).unwrap();
    assert_eq!(fields[0], ("balance".to_owned(), Value::U64(0)));
}

//
// ─── MeowCoinBalance ───
//

#[test]
fn meow_coin_balance_into_value_has_qualified_type_name() {
    let val: Value = MeowCoinBalance::new(50).into();
    assert_eq!(val.type_name(), MEOW_COIN_BALANCE_BYTECODE_TYPE_NAME);
}

#[test]
fn meow_coin_balance_round_trip() {
    let balance = MeowCoinBalance::new(250);
    assert_eq!(balance.amount(), 250);

    let value: Value = balance.into();
    assert_eq!(meow_coin_balance_struct::amount(&value), Some(250));
}

#[test]
fn is_meow_coin_balance_positive() {
    assert!(is_meow_coin_balance(
        &MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_BALANCE_STRUCT_NAME
    ));
}

#[test]
fn is_meow_coin_balance_negative() {
    assert!(!is_meow_coin_balance(
        &MEOW_SYSTEM_ADDRESS,
        MEOW_COIN_BALANCE_STRUCT_NAME
    ));
    assert!(!is_meow_coin_balance(
        &MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_OBJECT_NAME
    ));
}

#[test]
fn meow_coin_balance_struct_amount_returns_none_for_wrong_type() {
    let coin: Value = MeowCoin::new(Address::fill(0x01), 100).into();
    assert_eq!(meow_coin_balance_struct::amount(&coin), None);
}

//
// ─── Utility functions ───
//

fn test_address() -> Address {
    Address::new([1; 32])
}

fn test_owner() -> Address {
    Address::new([2; 32])
}

fn test_content() -> Vec<u8> {
    vec![1, 2, 3]
}

fn test_transaction_digest() -> Digest {
    Digest::compute(b"test transaction").unwrap()
}

fn test_other_object() -> Object {
    Object::fresh_object(
        test_address(),
        test_owner(),
        test_transaction_digest(),
        test_other_object_decl_ref(),
        test_content(),
    )
}

fn test_meow_coin_object() -> Object {
    let coin_value: Value = MeowCoin::new(test_address(), 100).into();

    object_conversion::vm_object_value_to_object(
        &coin_value,
        ObjectOwner::Address(test_owner()),
        test_transaction_digest(),
        ObjectVersion::ONE,
    )
    .unwrap()
}

fn test_meow_coin_object_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::try_from(MEOW_COIN_OBJECT_NAME).unwrap(),
    )
}

fn test_other_object_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::try_from("Coin").unwrap(),
    )
}
