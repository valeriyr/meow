use std::str::FromStr;

use meow_types::{address::Address, object::object_id::ObjectId};

//
// Object ID creation tests.
//

#[test]
fn zero_object_id() {
    let object_id = ObjectId::ZERO;
    assert_eq!(
        object_id.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}

#[test]
fn custom_object_id() {
    let object_id = ObjectId::new(Address::new([1; 32]));
    assert_eq!(
        object_id.to_string(),
        "0x0101010101010101010101010101010101010101010101010101010101010101"
    );
}

//
// Object ID conversion tests.
//

#[test]
fn object_id_from_string() {
    let parsed =
        ObjectId::from_str("0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f")
            .unwrap();

    assert_eq!(
        parsed.to_string(),
        "0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f"
    );
}

#[test]
fn object_id_from_bytes() {
    let bytes: Vec<u8> =
        prefix_hex::decode("0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f")
            .unwrap();

    let parsed = ObjectId::try_from(bytes.as_slice()).unwrap();

    assert_eq!(
        parsed.to_string(),
        "0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f"
    );
}
