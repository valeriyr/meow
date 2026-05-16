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
            MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_MODULE_NAME, MEOW_COIN_OBJECT_NAME, MeowCoin,
            deduct_gas_coin_balance, gas_meow_coin_balance, is_meow_coin, is_meow_coin_object,
            is_meow_coin_object_decl_ref,
        },
        meow_object::{
            MEOW_OBJECT_ID_BYTECODE_TYPE_NAME, MEOW_OBJECT_MODULE_ADDRESS, MeowObjectId,
        },
    },
};
use meow_vm_types::{
    convert::{self, struct_from_rust},
    types::Value,
};

//
// ─── System address tests ───
//

#[test]
fn system_address_to_string() {
    assert_eq!(
        MEOW_SYSTEM_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}

//
// ─── MeowObjectId tests ───
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
    let val = MeowObjectId::new(addr).to_qualified_vm_value();
    assert_eq!(
        val,
        Value::Struct {
            type_name: MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string(),
            fields: vec![("inner".to_string(), Value::Address(addr.into()))],
        }
    );
}

//
// ─── MeowCoin tests ───
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
fn round_trip_meow_coin() {
    let id = Address::fill(0xFFu8);
    let balance = 50;

    let rust_coin = MeowCoin::new(id, balance);
    let coin = struct_from_rust(&rust_coin).expect("must convert to Value");

    assert_eq!(rust_coin.id().inner(), &id);
    assert_eq!(rust_coin.balance(), balance);

    let restored = convert::value_to_rust::<MeowCoin>(&coin).expect("must round-trip back");
    assert_eq!(restored.id().inner(), &id);
    assert_eq!(restored.balance(), balance);
}

#[test]
fn object_is_meow_coin() {
    let object = test_meow_coin_object();

    assert!(is_meow_coin_object(&object));
    assert_eq!(gas_meow_coin_balance(&object), Some(100));
}

#[test]
fn object_is_not_meow_coin() {
    let object = test_other_object();

    assert!(!is_meow_coin_object(&object));
}

#[test]
fn object_decl_ref_is_meow_coin() {
    let decl_ref = test_meow_coin_object_decl_ref();

    assert!(is_meow_coin_object_decl_ref(&decl_ref));
}

#[test]
fn object_decl_ref_is_not_meow_coin() {
    let decl_ref = test_other_object_decl_ref();

    assert!(!is_meow_coin_object_decl_ref(&decl_ref));
}

#[test]
fn meow_coin_object_deduct_balance() {
    let object = test_meow_coin_object();

    let updated_content = deduct_gas_coin_balance(&object, 30).expect("must get updated content");

    let fields: Vec<(String, Value)> = bcs::from_bytes(&updated_content).unwrap();

    assert_eq!(fields.len(), 1);

    assert_eq!(fields[0], ("balance".to_owned(), Value::U64(70)));
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
    let coin_value =
        struct_from_rust(&MeowCoin::new(test_address(), 100)).expect("must convert to Value");

    object_conversion::vm_object_value_to_object(
        &coin_value,
        ObjectOwner::Address(test_owner()),
        test_transaction_digest(),
        ObjectVersion::ONE,
        &MEOW_COIN_MODULE_ADDRESS,
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
