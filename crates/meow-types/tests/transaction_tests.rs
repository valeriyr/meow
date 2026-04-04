use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{Transaction, call::Call, transaction_type::TransactionType},
};

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
