use meow_types::{
    address::Address,
    keypair::{signature_scheme::SignatureScheme, KeyPair},
};
use rand::{rngs::StdRng, SeedableRng};

//
// Address creation tests.
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

//
// Address conversion tests.
//

#[test]
fn address_from_keypair() {
    let address = Address::from(&test_keypair());

    assert_eq!(
        address.to_string(),
        "0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f"
    );
}

#[test]
fn address_from_public_key() {
    let address = Address::from(test_keypair().public());

    assert_eq!(
        address.to_string(),
        "0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f"
    );
}

//
// Utility functions.
//

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]))
}
