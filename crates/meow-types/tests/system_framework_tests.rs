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
    },
};
use meow_vm_types::{convert, types::Value};

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
// ─── MeowCoin tests ───
//

#[test]
fn meow_coin_module_address_to_string() {
    assert_eq!(
        MEOW_COIN_MODULE_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000001"
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
    let id: [u8; 32] = [0xFFu8; 32];
    let balance = 50;

    let coin = Value::Object {
        type_name: "MeowCoin".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("balance".to_string(), Value::U64(balance)),
        ],
    };

    let rust_coin = convert::value_to_rust::<MeowCoin>(&coin).expect("must convert to Rust");

    assert_eq!(rust_coin.id(), &Address::from(id));
    assert_eq!(rust_coin.balance(), balance);

    assert_eq!(
        coin,
        convert::object_from_rust(&rust_coin).expect("must convert back to Value")
    );
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
    let coin = MeowCoin::new(test_address(), 100);
    let coin_value = convert::object_from_rust(&coin).expect("must convert to Value");

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
