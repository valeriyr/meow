use std::{fs, path::Path};

use meow_types::{
    address::Address,
    keypair::{signature_scheme::SignatureScheme, KeyPair},
    keystore::{error::KeystoreError, Keystore},
};
use rand::{rngs::StdRng, SeedableRng};
use temp_dir::TempDir;

//
// In-memory keystore tests.
//

#[test]
fn empty_in_memory_keystore() {
    let keystore = Keystore::in_memory();

    assert_eq!(keystore.iter().count(), 0);
}

#[test]
fn add_keys_to_in_memory_keystore() {
    let mut keystore = Keystore::in_memory();

    // Add two keys.
    let (_, keypair1) = test_keypair1();
    let (_, keypair2) = test_keypair2();

    keystore.add_key(keypair1).unwrap();
    keystore.add_key(keypair2).unwrap();

    // Check the keystore iterator.
    let (address1, keypair1) = test_keypair1();
    let (address2, keypair2) = test_keypair2();

    assert_eq!(keystore.iter().count(), 2);

    let mut iter = keystore.iter();

    assert_eq!(iter.next().unwrap(), (&address2, &keypair2));
    assert_eq!(iter.next().unwrap(), (&address1, &keypair1));
    assert_eq!(iter.next(), None);

    // Check the keystore getters.
    let (address1, keypair1) = test_keypair1();
    let (address2, keypair2) = test_keypair2();

    assert_eq!(keystore.get_key(&address1), Some(&keypair1));
    assert_eq!(keystore.get_key(&address2), Some(&keypair2));
}

#[test]
fn add_duplicate_key_to_in_memory_keystore() {
    let mut keystore = Keystore::in_memory();

    let (_, keypair1) = test_keypair1();
    let (_, keypair2) = test_keypair1();

    keystore.add_key(keypair1).unwrap();

    assert!(matches!(
        keystore.add_key(keypair2),
        Err(KeystoreError::KeyPairAlreadyExists { .. })
    ));
}

//
// File-based keystore tests.
//

#[test]
fn load_file_based_keystore_from_non_existent_file() {
    let tmp_dir = TempDir::new().unwrap();

    let keystore = test_file_based_keystore(tmp_dir.path());

    assert_eq!(keystore.iter().count(), 0);
}

#[test]
fn load_file_based_keystore() {
    let tmp_dir = TempDir::new().unwrap();

    // Add two keys.
    {
        let mut keystore = test_file_based_keystore(tmp_dir.path());

        let (_, keypair1) = test_keypair1();
        let (_, keypair2) = test_keypair2();

        keystore.add_key(keypair1).unwrap();
        keystore.add_key(keypair2).unwrap();
    }

    // Check the keystore.
    {
        let keystore = test_file_based_keystore(tmp_dir.path());

        // Check the keystore iterator.
        let (address1, keypair1) = test_keypair1();
        let (address2, keypair2) = test_keypair2();

        assert_eq!(keystore.iter().count(), 2);

        let mut iter = keystore.iter();

        assert_eq!(iter.next().unwrap(), (&address2, &keypair2));
        assert_eq!(iter.next().unwrap(), (&address1, &keypair1));
        assert_eq!(iter.next(), None);

        // Check the keystore getters.
        let (address1, keypair1) = test_keypair1();
        let (address2, keypair2) = test_keypair2();

        assert_eq!(keystore.get_key(&address1), Some(&keypair1));
        assert_eq!(keystore.get_key(&address2), Some(&keypair2));
    }
}

#[test]
fn load_file_based_keystore_from_invalid_file() {
    let tmp_dir = TempDir::new().unwrap();

    let keystore_file = tmp_dir.path().join("keystore.json");

    fs::write(&keystore_file, "meow!").unwrap();

    assert!(matches!(
        Keystore::file_based(&keystore_file),
        Err(KeystoreError::SerdeJsonError { .. })
    ));
}

#[test]
fn add_duplicate_key_to_file_based_keystore() {
    let tmp_dir = TempDir::new().unwrap();

    let mut keystore = test_file_based_keystore(tmp_dir.path());

    let (_, keypair1) = test_keypair1();
    let (_, keypair2) = test_keypair1();

    keystore.add_key(keypair1).unwrap();

    assert!(matches!(
        keystore.add_key(keypair2),
        Err(KeystoreError::KeyPairAlreadyExists { .. })
    ));
}

//
// Utility functions.
//

fn test_file_based_keystore(tmp_dir: &Path) -> Keystore {
    let keystore_file = tmp_dir.join("keystore.json");
    Keystore::file_based(&keystore_file).unwrap()
}

fn test_keypair1() -> (Address, KeyPair) {
    let keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]));
    (keypair.public().into(), keypair)
}

fn test_keypair2() -> (Address, KeyPair) {
    let keypair = KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([1; 32]));
    (keypair.public().into(), keypair)
}
