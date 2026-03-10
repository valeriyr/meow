use meow_types::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, error::KeyPairError, signature_scheme::SignatureScheme},
    object::identifier::Identifier,
    transaction::{
        SignedTransaction, Transaction,
        call::{Call, Input},
        error::TransactionError,
    },
};
use rand::{SeedableRng, rngs::StdRng};

//
// Transaction construction.
//

#[test]
fn transaction_sender() {
    let tx = Transaction::new(sender(), gas(), test_call());
    assert_eq!(tx.sender(), &sender());
}

#[test]
fn transaction_call() {
    let call = Call::new(module(), function(), vec![Input::Raw(vec![1, 2, 3])]);
    let tx = Transaction::new(sender(), gas(), call.clone());
    assert_eq!(tx.call(), &call);
}

#[test]
fn transaction_equality() {
    let tx1 = test_tx();
    let tx2 = test_tx();
    assert_eq!(tx1, tx2);
}

#[test]
fn transaction_inequality_by_sender() {
    let tx1 = Transaction::new(Address::new([1; 32]), gas(), test_call());
    let tx2 = Transaction::new(Address::new([9; 32]), gas(), test_call());
    assert_ne!(tx1, tx2);
}

#[test]
fn transaction_inequality_by_gas() {
    let tx1 = Transaction::new(sender(), Address::new([2; 32]), test_call());
    let tx2 = Transaction::new(sender(), Address::new([9; 32]), test_call());
    assert_ne!(tx1, tx2);
}

#[test]
fn transaction_inequality_by_call() {
    let call1 = Call::new(module(), function(), vec![]);
    let call2 = Call::new(module(), function(), vec![Input::Raw(vec![42])]);
    let tx1 = Transaction::new(sender(), gas(), call1);
    let tx2 = Transaction::new(sender(), gas(), call2);
    assert_ne!(tx1, tx2);
}

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
fn transaction_digest_differs_by_sender() {
    let tx1 = Transaction::new(Address::new([1; 32]), gas(), test_call());
    let tx2 = Transaction::new(Address::new([9; 32]), gas(), test_call());
    assert_ne!(tx1.digest(), tx2.digest());
}

#[test]
fn transaction_digest_differs_by_gas() {
    let tx1 = Transaction::new(sender(), Address::new([2; 32]), test_call());
    let tx2 = Transaction::new(sender(), Address::new([9; 32]), test_call());
    assert_ne!(tx1.digest(), tx2.digest());
}

#[test]
fn transaction_digest_differs_by_call_arguments() {
    let call1 = Call::new(module(), function(), vec![]);
    let call2 = Call::new(module(), function(), vec![Input::Raw(vec![0xff])]);
    let tx1 = Transaction::new(sender(), gas(), call1);
    let tx2 = Transaction::new(sender(), gas(), call2);
    assert_ne!(tx1.digest(), tx2.digest());
}

#[test]
fn known_transaction_digest() {
    assert_eq!(
        test_tx().digest().to_string(),
        "E9DNRvNRj9kJ5SgWsie3D5y4N1gAhkguSmW1EpaQ8umu"
    );
}

//
// Call.
//

#[test]
fn call_module() {
    let call = Call::new(module(), function(), vec![]);
    assert_eq!(call.module(), &module());
}

#[test]
fn call_function() {
    let call = Call::new(module(), function(), vec![]);
    assert_eq!(call.function(), &function());
}

#[test]
fn call_arguments_empty() {
    let call = Call::new(module(), function(), vec![]);
    assert!(call.arguments().is_empty());
}

#[test]
fn call_arguments_object_input() {
    let obj = Address::new([10; 32]);
    let call = Call::new(module(), function(), vec![Input::Object(obj.clone())]);
    assert_eq!(call.arguments(), &[Input::Object(obj)]);
}

#[test]
fn call_arguments_raw_input() {
    let data = vec![1, 2, 3, 4];
    let call = Call::new(module(), function(), vec![Input::Raw(data.clone())]);
    assert_eq!(call.arguments(), &[Input::Raw(data)]);
}

#[test]
fn call_arguments_mixed_inputs() {
    let obj = Address::new([10; 32]);
    let args = vec![Input::Object(obj), Input::Raw(vec![0xde, 0xad])];
    let call = Call::new(module(), function(), args.clone());
    assert_eq!(call.arguments(), args.as_slice());
}

#[test]
fn call_equality() {
    let c1 = Call::new(module(), function(), vec![Input::Raw(vec![1])]);
    let c2 = Call::new(module(), function(), vec![Input::Raw(vec![1])]);
    assert_eq!(c1, c2);
}

#[test]
fn call_inequality_by_module() {
    let c1 = Call::new(Address::new([3; 32]), function(), vec![]);
    let c2 = Call::new(Address::new([4; 32]), function(), vec![]);
    assert_ne!(c1, c2);
}

#[test]
fn call_inequality_by_function() {
    let c1 = Call::new(module(), Identifier::new("foo").unwrap(), vec![]);
    let c2 = Call::new(module(), Identifier::new("bar").unwrap(), vec![]);
    assert_ne!(c1, c2);
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
    let tx1 = Transaction::new(Address::new([1; 32]), gas(), test_call());
    let tx2 = Transaction::new(Address::new([9; 32]), gas(), test_call());
    let sig = test_keypair().sign(tx1.digest().as_ref());
    let signed = SignedTransaction::new(tx2, sig);
    assert!(matches!(
        signed.verify().unwrap_err(),
        TransactionError::KeyPairError(KeyPairError::Ed25519ConsensusError(_))
    ));
}

#[test]
fn signed_transaction_accessors() {
    let tx = test_tx();
    let sig = test_keypair().sign(tx.digest().as_ref());
    let signed = SignedTransaction::new(tx.clone(), sig.clone());
    assert_eq!(signed.transaction(), &tx);
    assert_eq!(signed.signature(), &sig);
}

#[test]
fn signed_transaction_equality() {
    let tx = test_tx();
    let sig = test_keypair().sign(tx.digest().as_ref());
    let s1 = SignedTransaction::new(tx.clone(), sig.clone());
    let s2 = SignedTransaction::new(tx, sig);
    assert_eq!(s1, s2);
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

fn test_call() -> Call {
    Call::new(module(), function(), vec![])
}

fn test_tx() -> Transaction {
    Transaction::new(sender(), gas(), test_call())
}

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]))
}
