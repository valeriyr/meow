use std::str::FromStr;

use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
};
use rand::{SeedableRng, rngs::StdRng};

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
// Address derive tests.
//

#[test]
fn derive_is_deterministic() {
    let digest = test_digest();
    let a1 = Address::derive(digest.clone(), 0, 0);
    let a2 = Address::derive(digest, 0, 0);
    assert_eq!(a1, a2);
}

#[test]
fn derive_not_zero() {
    let address = Address::derive(test_digest(), 0, 0);
    assert_ne!(address, Address::ZERO);
}

#[test]
fn derive_differs_by_tag() {
    let digest = test_digest();
    let a0 = Address::derive(digest.clone(), 0, 0);
    let a1 = Address::derive(digest, 1, 0);
    assert_ne!(a0, a1);
}

#[test]
fn derive_differs_by_number() {
    let digest = test_digest();
    let a0 = Address::derive(digest.clone(), 0, 0);
    let a1 = Address::derive(digest, 0, 1);
    assert_ne!(a0, a1);
}

#[test]
fn derive_differs_by_digest() {
    let a0 = Address::derive(test_digest(), 0, 0);
    let a1 = Address::derive(other_digest(), 0, 0);
    assert_ne!(a0, a1);
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

#[test]
fn address_from_string() {
    let parsed =
        Address::from_str("0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f")
            .unwrap();
    let expected = Address::from(&test_keypair());

    assert_eq!(parsed, expected);
}

#[test]
fn address_from_bytes() {
    let bytes: Vec<u8> =
        prefix_hex::decode("0xcc2196ee1fa156836daf9bb021d88d648a0023fa387e695d3701667a634a331f")
            .unwrap();

    let parsed = Address::try_from(bytes.as_slice()).unwrap();
    let expected = Address::from(&test_keypair());

    assert_eq!(parsed, expected);
}

//
// Utility functions.
//

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]))
}

fn test_digest() -> Digest {
    Digest::sign(b"hello").unwrap()
}

fn other_digest() -> Digest {
    Digest::sign(b"world").unwrap()
}
