use meow_types::system_framework::{
    MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_MODULE_NAME, MEOW_COIN_OBJECT_NAME,
    MEOW_SYSTEM_ADDRESS_ADDRESS, is_gas_coin,
};

#[test]
fn system_addresses_to_string() {
    assert_eq!(
        MEOW_SYSTEM_ADDRESS_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(
        MEOW_COIN_MODULE_ADDRESS.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );
}

#[test]
fn gas_coin_positive() {
    assert!(is_gas_coin(
        &MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_OBJECT_NAME
    ));
}

#[test]
fn gas_coin_negative() {
    assert!(!is_gas_coin(
        &MEOW_SYSTEM_ADDRESS_ADDRESS,
        MEOW_COIN_OBJECT_NAME
    ));
    assert!(!is_gas_coin(
        &MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MODULE_NAME
    ));
}
