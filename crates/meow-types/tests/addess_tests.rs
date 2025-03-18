use meow_types::address::Address;

#[test]
fn zero_address() {
    let address = Address::ZERO;
    assert_eq!(
        address.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}
