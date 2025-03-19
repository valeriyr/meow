use std::str::FromStr;

use bip32::DerivationPath;
use meow_types::keypair::{ed25519::Ed25519KeyPair, error::KeyPairError};
use rand::{rngs::StdRng, SeedableRng};

/// A seed used in the tests.
const SEED: &str = "34e52ea12212a4b6ce7301eba2cbd9c089886ffb2af0c8835cd565106039a28d0319351451f493e4e9472f77d7ce4d910d552c5c4987e9600c5c436a93f59a24";
/// A derivation path used in the tests.
const DERIVATION_PATH: &str = "m/44'/9999'/0'/0'/0'";

//
// Ed25519 keypair derivation tests.
//

#[test]
fn ed25519_keypair_derive() {
    let keypair = test_keypair();

    assert_eq!(
        keypair.encode_base64(),
        "mQVhek1Lp7uJq4Z1Rv1pMloOEt0yGT4GGUJ0n0Ts2NI="
    );
    assert_eq!(
        keypair.encode_hex(),
        "9905617a4d4ba7bb89ab867546fd69325a0e12dd32193e061942749f44ecd8d2"
    );
    assert_eq!(
        format!("{keypair:?}"),
        "Ed25519KeyPair { public: Ed25519PublicKey(VerificationKey(\"3d683eed8ee67d24091b27c2de86d7504cdea3fb6f279d64b413b4b011913f82\")), private: <elided secret for Ed25519PrivateKey> }"
    );

    assert_eq!(
        keypair.public().encode_base64(),
        "PWg+7Y7mfSQJGyfC3obXUEzeo/tvJ51ktBO0sBGRP4I="
    );
    assert_eq!(
        keypair.public().encode_hex(),
        "3d683eed8ee67d24091b27c2de86d7504cdea3fb6f279d64b413b4b011913f82"
    );
}

#[test]
fn ed25519_keypair_derive_with_invalid_derivation_path() {
    let seed = hex::decode(SEED).unwrap();
    let path = DerivationPath::from_str("m/44'/9999'/0'/0'/0").unwrap();

    assert!(matches!(
        Ed25519KeyPair::derive(&seed, Some(path)),
        Err(KeyPairError::InvalidDerivationPath { .. })
    ));
}

//
// Ed25519 keypair random generation tests.
//

#[test]
fn ed25519_keypair_random() {
    let keypair = Ed25519KeyPair::random(StdRng::from_seed([0; 32]));

    assert_eq!(
        keypair.encode_base64(),
        "m/SaagdV+VOBH84SXyaD1QQpw7tJ4HQUfgCJpS6uFV8="
    );
    assert_eq!(
        keypair.encode_hex(),
        "9bf49a6a0755f953811fce125f2683d50429c3bb49e074147e0089a52eae155f"
    );

    assert_eq!(
        keypair.public().encode_base64(),
        "ucbuFjDvPnERRKZI2wa7sihPcnTPvuU//O5QPMGkkgA="
    );
    assert_eq!(
        keypair.public().encode_hex(),
        "b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200"
    );
}

//
// Ed25519 keypair conversion tests.
//

#[test]
fn ed25519_keypair_decode_bytes() {
    let keypair = test_keypair();

    assert_eq!(
        Ed25519KeyPair::try_from(keypair.as_bytes()).unwrap(),
        keypair
    );
}

#[test]
fn ed25519_keypair_decode_empty_bytes_array() {
    let bytes = vec![];

    assert!(matches!(
        Ed25519KeyPair::try_from(bytes.as_slice()),
        Err(KeyPairError::Ed25519ConsensusError(
            ed25519_consensus::Error::InvalidSliceLength
        ))
    ));
}

#[test]
fn ed25519_keypair_decode_invalid_bytes() {
    let keypair = test_keypair();

    assert!(matches!(
        Ed25519KeyPair::try_from(&keypair.as_bytes()[1..]),
        Err(KeyPairError::Ed25519ConsensusError(
            ed25519_consensus::Error::InvalidSliceLength
        ))
    ));
}

//
// Utility functions.
//

fn test_keypair() -> Ed25519KeyPair {
    let seed = hex::decode(SEED).unwrap();
    let path = DerivationPath::from_str(DERIVATION_PATH).unwrap();

    Ed25519KeyPair::derive(&seed, Some(path)).unwrap()
}
