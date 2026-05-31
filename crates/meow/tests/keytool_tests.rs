use meow::keytool::{KeyToolCommand, output::KeyToolCommandOutput};
use meow_types::{
    address::Address, keypair::signature_scheme::SignatureScheme, keystore::Keystore,
};

//
// ─── Generate tests ───
//

#[test]
fn generate_key_is_added_to_keystore() {
    let mut keystore = Keystore::in_memory();

    let output = generate(&mut keystore);

    assert!(matches!(output, KeyToolCommandOutput::Generate { .. }));
    assert_eq!(keystore.iter().count(), 1);
}

#[test]
fn generate_key_output_contains_address_and_scheme() {
    let mut keystore = Keystore::in_memory();

    let output = generate(&mut keystore);

    let key = match output {
        KeyToolCommandOutput::Generate { key, .. } => key,
        _ => panic!("expected Generate output"),
    };
    assert!(!key.address.is_empty());
    assert_eq!(key.scheme, "ed25519");
}

#[test]
fn generate_multiple_keys_all_added_to_keystore() {
    let mut keystore = Keystore::in_memory();

    generate(&mut keystore);
    generate(&mut keystore);
    generate(&mut keystore);

    assert_eq!(keystore.iter().count(), 3);
}

//
// ─── Recover tests ───
//

#[test]
fn recover_adds_key_to_keystore() {
    let mut keystore = Keystore::in_memory();
    let phrase = generate_phrase(&mut Keystore::in_memory());

    recover(&mut keystore, phrase);

    assert_eq!(keystore.iter().count(), 1);
}

#[test]
fn recover_produces_same_address_as_generate() {
    let mut ks1 = Keystore::in_memory();
    let mut ks2 = Keystore::in_memory();

    let phrase = generate_phrase(&mut ks1);
    let original_addr = *ks1.iter().next().unwrap().0;

    recover(&mut ks2, phrase);
    let recovered_addr = *ks2.iter().next().unwrap().0;

    assert_eq!(original_addr, recovered_addr);
}

#[test]
fn recover_duplicate_phrase_returns_error() {
    let mut keystore = Keystore::in_memory();
    let phrase = generate_phrase(&mut keystore);

    // Second recovery of the same phrase must fail — address already in keystore.
    assert!(recover_result(&mut keystore, phrase).is_err());
}

#[test]
fn recover_invalid_phrase_returns_error() {
    let mut keystore = Keystore::in_memory();

    assert!(recover_result(&mut keystore, "not a valid bip39 phrase".to_owned()).is_err());
}

//
// ─── List tests ───
//

#[test]
fn list_empty_keystore_returns_empty_list() {
    let mut keystore = Keystore::in_memory();

    let output = KeyToolCommand::List.run(&mut keystore).unwrap();

    assert!(matches!(output, KeyToolCommandOutput::List(keys) if keys.is_empty()));
}

#[test]
fn list_returns_all_generated_keys() {
    let mut keystore = Keystore::in_memory();

    let addr1 = generate_key(&mut keystore);
    let addr2 = generate_key(&mut keystore);

    let keys = match KeyToolCommand::List.run(&mut keystore).unwrap() {
        KeyToolCommandOutput::List(keys) => keys,
        _ => panic!("expected List output"),
    };

    assert_eq!(keys.len(), 2);
    let addresses: Vec<&str> = keys.iter().map(|k| k.address.as_str()).collect();
    assert!(addresses.contains(&addr1.to_string().as_str()));
    assert!(addresses.contains(&addr2.to_string().as_str()));
}

//
// ─── Remove tests ───
//

#[test]
fn remove_existing_key_returns_it() {
    let mut keystore = Keystore::in_memory();

    let address = generate_key(&mut keystore);

    let removed = match (KeyToolCommand::Remove { address })
        .run(&mut keystore)
        .unwrap()
    {
        KeyToolCommandOutput::Remove(Some(key)) => key,
        _ => panic!("expected Remove(Some(_)) output"),
    };

    assert_eq!(removed.address, address.to_string());
    assert_eq!(keystore.iter().count(), 0);
}

#[test]
fn remove_missing_key_returns_none() {
    let mut keystore = Keystore::in_memory();

    let output = KeyToolCommand::Remove {
        address: Address::suffixed(0xE1),
    }
    .run(&mut keystore)
    .unwrap();

    assert!(matches!(output, KeyToolCommandOutput::Remove(None)));
}

#[test]
fn remove_key_leaves_others_intact() {
    let mut keystore = Keystore::in_memory();

    let address = generate_key(&mut keystore);
    generate_key(&mut keystore);

    KeyToolCommand::Remove { address }
        .run(&mut keystore)
        .unwrap();

    assert_eq!(keystore.iter().count(), 1);
}

//
// ─── Utility functions ───
//

fn generate(keystore: &mut Keystore) -> KeyToolCommandOutput {
    KeyToolCommand::Generate {
        scheme: SignatureScheme::Ed25519,
        derivation_path: None,
        word_length: None,
    }
    .run(keystore)
    .expect("generate must succeed")
}

fn generate_key(keystore: &mut Keystore) -> Address {
    match generate(keystore) {
        KeyToolCommandOutput::Generate { key, .. } => {
            key.address.parse().expect("address must be valid")
        }
        _ => panic!("expected Generate output"),
    }
}

fn generate_phrase(keystore: &mut Keystore) -> String {
    match generate(keystore) {
        KeyToolCommandOutput::Generate { phrase, .. } => phrase,
        _ => panic!("expected Generate output"),
    }
}

fn recover(keystore: &mut Keystore, phrase: String) -> KeyToolCommandOutput {
    recover_result(keystore, phrase).expect("recover must succeed")
}

fn recover_result(
    keystore: &mut Keystore,
    phrase: String,
) -> Result<KeyToolCommandOutput, anyhow::Error> {
    KeyToolCommand::Recover {
        scheme: SignatureScheme::Ed25519,
        phrase: phrase.to_string(),
        derivation_path: None,
    }
    .run(keystore)
}
