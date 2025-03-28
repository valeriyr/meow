use std::str::FromStr;

use bip32::DerivationPath;
use meow_types::keypair::{KeyPair, error::KeyPairError, signature_scheme::SignatureScheme};
use rand::{SeedableRng, rngs::StdRng};

/// A seed used in the tests.
const SEED: &str = "34e52ea12212a4b6ce7301eba2cbd9c089886ffb2af0c8835cd565106039a28d0319351451f493e4e9472f77d7ce4d910d552c5c4987e9600c5c436a93f59a24";
/// A derivation path used in the tests.
const DERIVATION_PATH: &str = "m/44'/9999'/0'/0'/0'";

//
// Keypair derivation tests.
//

#[test]
fn ed25519_keypair_derive() {
    let keypair = test_ed25519_keypair();

    assert_eq!(
        keypair.encode_base64(),
        "AJkFYXpNS6e7iauGdUb9aTJaDhLdMhk+BhlCdJ9E7NjS"
    );
    assert_eq!(
        serde_json::to_string(&keypair).unwrap(),
        "\"AJkFYXpNS6e7iauGdUb9aTJaDhLdMhk+BhlCdJ9E7NjS\""
    );

    assert_eq!(
        format!("{keypair:?}"),
        "Ed25519(Ed25519KeyPair { public: Ed25519PublicKey(VerificationKey(\"3d683eed8ee67d24091b27c2de86d7504cdea3fb6f279d64b413b4b011913f82\")), private: <elided secret for Ed25519PrivateKey> })"
    );

    assert_eq!(
        keypair.public().encode_base64(),
        "PWg+7Y7mfSQJGyfC3obXUEzeo/tvJ51ktBO0sBGRP4I="
    );
    assert_eq!(
        keypair.public().encode_hex(),
        "3d683eed8ee67d24091b27c2de86d7504cdea3fb6f279d64b413b4b011913f82"
    );

    assert_eq!(keypair.public().scheme(), SignatureScheme::Ed25519);
}

#[test]
fn ed25519_keypair_derive_with_invalid_derivation_path() {
    let seed = hex::decode(SEED).unwrap();
    let path = DerivationPath::from_str("m/44'/9999'/0'/0'/0").unwrap();

    assert!(matches!(
        KeyPair::derive(&seed, SignatureScheme::Ed25519, Some(path)),
        Err(KeyPairError::InvalidDerivationPath { .. })
    ));
}

//
// Keypair random generation tests.
//

#[test]
fn ed25519_keypair_random() {
    let keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]));

    assert_eq!(
        keypair.encode_base64(),
        "AJv0mmoHVflTgR/OEl8mg9UEKcO7SeB0FH4AiaUurhVf"
    );
    assert_eq!(
        serde_json::to_string(&keypair).unwrap(),
        "\"AJv0mmoHVflTgR/OEl8mg9UEKcO7SeB0FH4AiaUurhVf\""
    );

    assert_eq!(
        keypair.public().encode_base64(),
        "ucbuFjDvPnERRKZI2wa7sihPcnTPvuU//O5QPMGkkgA="
    );
    assert_eq!(
        keypair.public().encode_hex(),
        "b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200"
    );

    assert_eq!(keypair.public().scheme(), SignatureScheme::Ed25519);
}

//
// Keypair conversion tests.
//

#[test]
fn ed25519_keypair_decode_bytes() {
    let keypair = test_ed25519_keypair();

    assert_eq!(KeyPair::from_bytes(&keypair.to_bytes()).unwrap(), keypair);
}

#[test]
fn ed25519_keypair_decode_empty_bytes_array() {
    let bytes = vec![];

    assert!(matches!(
        KeyPair::from_bytes(&bytes),
        Err(KeyPairError::InvalidKeyPairBytes { .. })
    ));
}

#[test]
fn ed25519_keypair_decode_invalid_bytes() {
    let keypair = test_ed25519_keypair();

    assert!(matches!(
        KeyPair::from_bytes(&keypair.to_bytes()[..32]),
        Err(KeyPairError::Ed25519ConsensusError(
            ed25519_consensus::Error::InvalidSliceLength
        ))
    ));
}

#[test]
fn ed25519_keypair_decode_invalid_signature_scheme() {
    let keypair = test_ed25519_keypair();

    let mut bytes = keypair.to_bytes();
    bytes[0] = 0x11;

    assert!(matches!(
        KeyPair::from_bytes(&bytes),
        Err(KeyPairError::InvalidSignatureSchemeFlag { flag: 0x11 })
    ));
}

#[test]
fn ed25519_keypair_decode_base64() {
    assert_eq!(
        KeyPair::decode_base64("AJkFYXpNS6e7iauGdUb9aTJaDhLdMhk+BhlCdJ9E7NjS").unwrap(),
        test_ed25519_keypair()
    );
}

#[test]
fn ed25519_keypair_deserialize_from_str() {
    assert_eq!(
        serde_json::from_str::<KeyPair>("\"AJkFYXpNS6e7iauGdUb9aTJaDhLdMhk+BhlCdJ9E7NjS\"")
            .unwrap(),
        test_ed25519_keypair()
    );
}

//
// Utility functions.
//

fn test_ed25519_keypair() -> KeyPair {
    let seed = hex::decode(SEED).unwrap();
    let path = DerivationPath::from_str(DERIVATION_PATH).unwrap();

    KeyPair::derive(&seed, SignatureScheme::Ed25519, Some(path)).unwrap()
}
