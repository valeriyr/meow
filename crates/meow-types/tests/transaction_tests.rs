use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, error::KeyPairError, signature_scheme::SignatureScheme},
    transaction::{SignedTransaction, Transaction, error::TransactionError},
};
use rand::{SeedableRng, rngs::StdRng};

//
// Transaction creation tests.
//

#[test]
fn transaction_sender() {
    let tx = Transaction::new(test_address());
    assert_eq!(tx.sender(), &test_address());
}

#[test]
fn transaction_equality() {
    let tx1 = Transaction::new(test_address());
    let tx2 = Transaction::new(test_address());
    assert_eq!(tx1, tx2);
}

#[test]
fn transaction_inequality() {
    let tx1 = Transaction::new(test_address());
    let tx2 = Transaction::new(Address::new([2; 32]));
    assert_ne!(tx1, tx2);
}

//
// Transaction digest tests.
//

#[test]
fn transaction_digest_is_deterministic() {
    let tx = Transaction::new(test_address());
    assert_eq!(tx.digest(), tx.digest());
}

#[test]
fn transaction_digest_is_not_zero() {
    let tx = Transaction::new(test_address());
    assert_ne!(tx.digest(), Digest::ZERO);
}

#[test]
fn transaction_digest_differs_by_sender() {
    let tx1 = Transaction::new(test_address());
    let tx2 = Transaction::new(Address::new([2; 32]));
    assert_ne!(tx1.digest(), tx2.digest());
}

//
// SignedTransaction tests.
//

#[test]
fn signed_transaction_verify_valid() {
    let tx = Transaction::new(test_address());
    let sig = test_keypair().sign(tx.digest().as_ref());
    let signed_tx = SignedTransaction::new(tx, sig);

    assert!(signed_tx.verify().is_ok());
}

#[test]
fn signed_transaction_verify_invalid() {
    let tx1 = Transaction::new(test_address());
    let tx2 = Transaction::new(Address::new([2; 32]));

    let sig = test_keypair().sign(tx1.digest().as_ref());
    let signed_tx = SignedTransaction::new(tx2, sig);

    assert!(matches!(
        signed_tx.verify().unwrap_err(),
        TransactionError::KeyPairError(KeyPairError::Ed25519ConsensusError(_))
    ));
}

#[test]
fn signed_transaction_equality() {
    let tx = Transaction::new(test_address());
    let sig = test_keypair().sign(tx.digest().as_ref());

    let signed_tx1 = SignedTransaction::new(tx.clone(), sig.clone());
    let signed_tx2 = SignedTransaction::new(tx, sig);

    assert_eq!(signed_tx1, signed_tx2);
}

//
// Utility functions.
//

fn test_address() -> Address {
    Address::new([1; 32])
}

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]))
}
