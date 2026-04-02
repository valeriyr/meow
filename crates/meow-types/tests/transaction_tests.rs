use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    keypair::{KeyPair, error::KeyPairError, signature_scheme::SignatureScheme},
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{
        SignedTransaction, Transaction, call::Call, error::TransactionError,
        transaction_type::TransactionType,
    },
};
use rand::{SeedableRng, rngs::StdRng};

//
// ─── Transaction digest ───
//

#[test]
fn transaction_digest_is_deterministic() {
    let tx = test_tx();
    assert_eq!(tx.digest(), tx.digest());
}

#[test]
fn transaction_digest_is_not_zero() {
    assert_ne!(test_tx().digest(), Digest::ZERO);
}

#[test]
fn known_transaction_digest() {
    assert_eq!(
        test_tx().digest().to_string(),
        "Efw9yuB4Z8JGXwLmBGYwrvtZBsW7wruQrNqdnCVNKEpP"
    );
}

//
// ─── SignedTransaction ───
//

#[test]
fn signed_transaction_verify_valid() {
    let tx = test_tx();
    let sig = test_keypair().sign(tx.digest().as_ref());
    let signed = SignedTransaction::new(tx, sig);
    assert!(signed.verify().is_ok());
}

#[test]
fn signed_transaction_verify_wrong_digest() {
    let tx1 = Transaction::new(Address::new([1; 32]), gas(), test_tx_type());
    let tx2 = Transaction::new(Address::new([9; 32]), gas(), test_tx_type());
    let sig = test_keypair().sign(tx1.digest().as_ref());
    let signed = SignedTransaction::new(tx2, sig);
    assert!(matches!(
        &signed.verify().unwrap_err(),
        TransactionError::KeyPairError(KeyPairError::Ed25519ConsensusError(e))
            if e.to_string() == "Invalid signature."
    ));
}

//
// ─── Utility functions ───
//

fn sender() -> Address {
    Address::fill(1)
}

fn gas() -> ObjectRef {
    ObjectRef::new(Address::fill(2), ObjectVersion::ONE, Digest::ZERO)
}

fn module() -> Address {
    Address::fill(3)
}

fn function() -> Identifier {
    Identifier::new("transfer").unwrap()
}

fn test_tx_type() -> TransactionType {
    TransactionType::MeowCall(Call::new(module(), function(), vec![]))
}

fn test_tx() -> Transaction {
    Transaction::new(sender(), gas(), test_tx_type())
}

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]))
}
