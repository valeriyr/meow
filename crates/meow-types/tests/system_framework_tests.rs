use meow_types::{
    address::Address,
    system_framework::{
        MEOW_SYSTEM_ADDRESS,
        meow_coin::{
            MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_MODULE_NAME, MEOW_COIN_OBJECT_NAME, MeowCoin,
            is_meow_coin,
        },
    },
};
use meow_vm_types::{convert, types::Value};

//
// System address tests.
//

#[test]
fn system_address_to_string() {
    assert_eq!(
        MEOW_SYSTEM_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}

//
// MeowCoin tests.
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
