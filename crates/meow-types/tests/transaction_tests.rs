use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, error::KeyPairError, signature_scheme::SignatureScheme},
    object::identifier::Identifier,
    transaction::{
        SignedTransaction, Transaction, call::Call, error::TransactionError,
        transaction_type::TransactionType,
    },
};
use rand::{SeedableRng, rngs::StdRng};

//
// Transaction digest.
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
        "AR9nyEJL4xuJnNkH8DsEQidfpAK21CiuUgsYNwR2ubEo"
    );
}

//
// SignedTransaction.
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
        signed.verify().unwrap_err(),
        TransactionError::KeyPairError(KeyPairError::Ed25519ConsensusError(_))
    ));
}

//
// Utility functions.
//

fn sender() -> Address {
    Address::new([1; 32])
}

fn gas() -> Address {
    Address::new([2; 32])
}

fn module() -> Address {
    Address::new([3; 32])
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
