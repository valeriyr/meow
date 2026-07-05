#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path};

use meow_types::{
    address::Address,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    keystore::{Keystore, error::KeystoreError},
};
use rand::{SeedableRng, rngs::StdRng};
use temp_dir::TempDir;

//
// ─── In-memory keystore tests ───
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

#[test]
fn remove_key_from_in_memory_keystore() {
    let mut keystore = Keystore::in_memory();

    let (_, keypair1) = test_keypair1();
    let (address2, keypair2) = test_keypair2();

    keystore.add_key(keypair1).unwrap();
    keystore.add_key(keypair2).unwrap();

    assert_eq!(keystore.iter().count(), 2);

    keystore.remove_key(&address2).unwrap();

    let (address1, keypair1) = test_keypair1();

    assert_eq!(keystore.iter().count(), 1);

    assert_eq!(keystore.get_key(&address1), Some(&keypair1));
    assert_eq!(keystore.get_key(&address2), None);
}

//
// ─── File-based keystore tests ───
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

#[test]
fn remove_key_from_file_based_keystore() {
    let tmp_dir = TempDir::new().unwrap();

    // Add two keys.
    {
        let mut keystore = test_file_based_keystore(tmp_dir.path());

        let (_, keypair1) = test_keypair1();
        let (_, keypair2) = test_keypair2();

        keystore.add_key(keypair1).unwrap();
        keystore.add_key(keypair2).unwrap();
    }

    // Remove a key from the keystore.
    {
        let mut keystore = test_file_based_keystore(tmp_dir.path());

        assert_eq!(keystore.iter().count(), 2);

        let (address1, keypair1) = test_keypair1();
        let (address2, _) = test_keypair2();

        keystore.remove_key(&address2).unwrap();

        assert_eq!(keystore.iter().count(), 1);

        assert_eq!(keystore.get_key(&address1), Some(&keypair1));
        assert_eq!(keystore.get_key(&address2), None);
    }

    // Check the keystore after removing the key.
    {
        let keystore = test_file_based_keystore(tmp_dir.path());

        let (address1, keypair1) = test_keypair1();
        let (address2, _) = test_keypair2();

        assert_eq!(keystore.iter().count(), 1);

        assert_eq!(keystore.get_key(&address1), Some(&keypair1));
        assert_eq!(keystore.get_key(&address2), None);
    }
}

// The two tests below verify that a failed disk write does not corrupt in-memory state.
// They are Unix-only because they rely on file permission manipulation to force a write failure.

#[cfg(unix)]
#[test]
fn add_key_save_failure_rolls_back_in_memory_state() {
    let tmp_dir = TempDir::new().unwrap();
    let keystore_file = tmp_dir.path().join("keystore.json");

    let mut keystore = Keystore::file_based(&keystore_file).unwrap();
    let (_, keypair1) = test_keypair1();
    keystore.add_key(keypair1).unwrap();

    // Make the file read-only to force a save failure on the next write.
    let mut perms = fs::metadata(&keystore_file).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&keystore_file, perms).unwrap();

    let (address2, keypair2) = test_keypair2();
    assert!(
        keystore.add_key(keypair2).is_err(),
        "add_key must return an error when saving fails"
    );

    // In-memory state must be rolled back — only the first key remains.
    let (address1, keypair1) = test_keypair1();
    assert_eq!(keystore.iter().count(), 1);
    assert_eq!(keystore.get_key(&address1), Some(&keypair1));
    assert_eq!(keystore.get_key(&address2), None);

    // Restore permissions so the temp dir can be cleaned up.
    let mut perms = fs::metadata(&keystore_file).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&keystore_file, perms).unwrap();
}

#[cfg(unix)]
#[test]
fn remove_key_save_failure_rolls_back_in_memory_state() {
    let tmp_dir = TempDir::new().unwrap();
    let keystore_file = tmp_dir.path().join("keystore.json");

    let mut keystore = Keystore::file_based(&keystore_file).unwrap();
    let (address1, keypair1) = test_keypair1();
    keystore.add_key(keypair1).unwrap();

    // Make the file read-only to force a save failure on the next write.
    let mut perms = fs::metadata(&keystore_file).unwrap().permissions();
    perms.set_mode(0o444);
    fs::set_permissions(&keystore_file, perms).unwrap();

    assert!(
        keystore.remove_key(&address1).is_err(),
        "remove_key must return an error when saving fails"
    );

    // In-memory state must be rolled back — the key must still be present.
    let (address1, keypair1) = test_keypair1();
    assert_eq!(keystore.iter().count(), 1);
    assert_eq!(keystore.get_key(&address1), Some(&keypair1));

    // Restore permissions so the temp dir can be cleaned up.
    let mut perms = fs::metadata(&keystore_file).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&keystore_file, perms).unwrap();
}

//
// ─── Utility functions ───
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
