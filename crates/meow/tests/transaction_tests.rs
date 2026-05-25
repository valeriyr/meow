use std::path::PathBuf;

use base64::{Engine, engine::general_purpose};
use meow::{
    commands::DEFAULT_NODE_URL,
    output_encoder::OutputEncoder,
    transaction::{
        TransactionCommand,
        output::{EncodedTransactionOutput, TransactionCommandOutput},
    },
};
use meow_node_client::NodeClient;
use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    keystore::Keystore,
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{
        SignedTransaction, Transaction, call::Call, transaction_type::TransactionType, validator,
    },
};
use rand::{SeedableRng, rngs::StdRng};
use temp_dir::TempDir;

//
// ─── Sign tests ───
//

#[tokio::test]
async fn sign_produces_a_valid_signed_transaction() {
    let tmp = TempDir::new().unwrap();
    let keypair = test_keypair();
    let sender = Address::from(&keypair);
    write_keystore(&tmp, keypair);

    let output = sign(&tmp, make_call_transaction(sender)).await.unwrap();

    let TransactionCommandOutput::Encoded(EncodedTransactionOutput { transaction }) = output else {
        panic!("expected Encoded output");
    };
    let bytes = general_purpose::STANDARD.decode(&transaction).unwrap();
    let signed: SignedTransaction = bcs::from_bytes(&bytes).unwrap();
    assert!(validator::validate_signed_transaction(&signed).is_ok());
}

#[tokio::test]
async fn sign_with_missing_sender_key_returns_error() {
    let tmp = TempDir::new().unwrap();
    // No keystore file written — file_based returns an empty keystore for non-existent paths.
    let err = sign(&tmp, make_call_transaction(Address::suffixed(0xE1)))
        .await
        .unwrap_err();

    assert!(
        err.to_string()
            .contains("A key has not been found in the keystore"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn sign_invalid_base64_returns_error() {
    let tmp = TempDir::new().unwrap();

    let err = TransactionCommand::Sign {
        keystore_path: Some(keystore_path(&tmp)),
        transaction: "not valid base64 !!!".to_string(),
    }
    .run(&fake_client(), OutputEncoder::Base64, false)
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("Invalid symbol"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn sign_valid_base64_but_invalid_bcs_returns_error() {
    let tmp = TempDir::new().unwrap();

    let junk = general_purpose::STANDARD.encode(b"this is not a BCS-encoded Transaction");

    let err = TransactionCommand::Sign {
        keystore_path: Some(keystore_path(&tmp)),
        transaction: junk,
    }
    .run(&fake_client(), OutputEncoder::Base64, false)
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("unexpected end of input"),
        "unexpected error: {err}"
    );
}

//
// ─── Simulate tests ───
//

#[tokio::test]
async fn simulate_invalid_base64_returns_error() {
    let err = TransactionCommand::Simulate {
        transaction: "not valid base64 !!!".to_string(),
    }
    .run(&fake_client(), OutputEncoder::Base64, false)
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("Invalid symbol"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn simulate_valid_base64_but_invalid_bcs_returns_error() {
    let junk = general_purpose::STANDARD.encode(b"this is not a BCS-encoded Transaction");

    let err = TransactionCommand::Simulate { transaction: junk }
        .run(&fake_client(), OutputEncoder::Base64, false)
        .await
        .unwrap_err();

    assert!(
        err.to_string().contains("unexpected end of input"),
        "unexpected error: {err}"
    );
}

//
// ─── ExecuteLocally tests ───
//

#[tokio::test]
async fn execute_locally_invalid_base64_returns_error() {
    let err = TransactionCommand::ExecuteLocally {
        transaction: "not valid base64 !!!".to_string(),
        seed: Digest::ZERO,
        timestamp: None,
    }
    .run(&fake_client(), OutputEncoder::Base64, false)
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("Invalid symbol"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn execute_locally_valid_base64_but_invalid_bcs_returns_error() {
    let junk = general_purpose::STANDARD.encode(b"this is not a BCS-encoded Transaction");

    let err = TransactionCommand::ExecuteLocally {
        transaction: junk,
        seed: Digest::ZERO,
        timestamp: None,
    }
    .run(&fake_client(), OutputEncoder::Base64, false)
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("unexpected end of input"),
        "unexpected error: {err}"
    );
}

//
// ─── Utility functions ───
//

fn fake_client() -> NodeClient {
    NodeClient::with_url(DEFAULT_NODE_URL.parse().unwrap())
}

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]))
}

fn make_call_transaction(sender: Address) -> Transaction {
    let gas_coin = ObjectRef::new(Address::suffixed(0xF1), ObjectVersion::ONE, Digest::ZERO);
    let call = Call::new(
        Address::suffixed(0xFD),
        Identifier::new("run").unwrap(),
        vec![],
    );
    Transaction::new(sender, gas_coin, TransactionType::MeowCall(call))
}

async fn sign(tmp: &TempDir, transaction: Transaction) -> anyhow::Result<TransactionCommandOutput> {
    let transaction_b64 = general_purpose::STANDARD.encode(bcs::to_bytes(&transaction).unwrap());
    TransactionCommand::Sign {
        keystore_path: Some(keystore_path(tmp)),
        transaction: transaction_b64,
    }
    .run(&fake_client(), OutputEncoder::Base64, false)
    .await
}

fn keystore_path(tmp: &TempDir) -> PathBuf {
    tmp.path().join("keystore.json")
}

fn write_keystore(tmp: &TempDir, keypair: KeyPair) {
    let mut keystore = Keystore::file_based(&keystore_path(tmp)).unwrap();
    keystore.add_key(keypair).unwrap();
}
