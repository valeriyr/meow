use std::str::FromStr;

use meow_vm_types::address::{Address, error::AddressError};

//
// ─── Address creation tests ───
//

#[test]
fn zero_address() {
    let address = Address::ZERO;
    assert_eq!(
        address.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}

#[test]
fn custom_address() {
    let address = Address::new([1; 32]);
    assert_eq!(
        address.to_string(),
        "0x0101010101010101010101010101010101010101010101010101010101010101"
    );
}

#[test]
fn fill_address() {
    assert_eq!(
        Address::fill(0).to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(
        Address::fill(1).to_string(),
        "0x0101010101010101010101010101010101010101010101010101010101010101"
    );
    assert_eq!(
        Address::fill(0xAA).to_string(),
        "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn suffixed_address() {
    assert_eq!(
        Address::suffixed(0x0000).to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
    assert_eq!(
        Address::suffixed(0x0001).to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );
    assert_eq!(
        Address::suffixed(0x1234).to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000001234"
    );
    assert_eq!(
        Address::suffixed(0xFFFF).to_string(),
        "0x000000000000000000000000000000000000000000000000000000000000ffff"
    );
}

#[test]
fn suffixed_distinct_suffixes_produce_distinct_addresses() {
    assert_ne!(Address::suffixed(0x01), Address::suffixed(0x10));
    assert_ne!(Address::suffixed(0x00), Address::suffixed(0x01));
}

//
// ─── Address conversion tests ───
//

#[test]
fn address_from_string() {
    let parsed =
        Address::from_str("0x0000000000000000000000000000000000000000000000000000000000000042")
            .unwrap();

    assert_eq!(
        parsed.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000042"
    );
}

#[test]
fn address_from_short_string() {
    let parsed = Address::from_str("0x42").unwrap();

    assert_eq!(
        parsed.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000042"
    );
}

#[test]
fn address_from_bytes() {
    let bytes = [1u8; 32];

    let parsed = Address::try_from(bytes.as_slice()).unwrap();

    assert_eq!(
        parsed.to_string(),
        "0x0101010101010101010101010101010101010101010101010101010101010101"
    );
}

//
// ─── Address invalid conversion tests ───
//

#[test]
fn address_from_string_missing_prefix_returns_error() {
    let err = Address::from_str("42").expect_err("missing 0x prefix must fail");
    assert!(matches!(err, AddressError::PrefixHexError(_)));
}

#[test]
fn address_from_string_invalid_hex_returns_error() {
    let err = Address::from_str("0xzz").expect_err("invalid hex must fail");
    assert!(matches!(err, AddressError::PrefixHexError(_)));
}

#[test]
fn address_from_string_odd_length_hex_returns_error() {
    let err = Address::from_str("0x1").expect_err("odd-length hex must fail");
    assert!(matches!(err, AddressError::PrefixHexError(_)));
}

#[test]
fn address_from_string_too_long_returns_error() {
    let too_long = format!("0x{}", "00".repeat(33));
    let err = Address::from_str(&too_long).expect_err("more than 32 bytes must fail");
    assert!(matches!(
        err,
        AddressError::InvalidLength {
            actual: 33,
            expected: 32
        }
    ));
}

#[test]
fn address_from_bytes_invalid_length_returns_error() {
    let bytes = [0u8; 31];
    let err = Address::try_from(bytes.as_slice()).expect_err("31 bytes must fail");
    assert!(matches!(
        err,
        AddressError::InvalidLength {
            actual: 31,
            expected: 32
        }
    ));
}

//
// ─── Address debug tests ───
//

#[test]
fn debug_print_address() {
    let parsed = Address::from_str("0x42").unwrap();

    assert_eq!(
        format!("{parsed:?}"),
        "0x0000000000000000000000000000000000000000000000000000000000000042"
    );
}
