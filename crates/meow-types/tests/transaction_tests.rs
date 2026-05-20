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
        "4rYRUYG5uYc92vZxm3KHR3enpvXLyuyVnCXUkNGnZWE9"
    );
}

//
// ─── Utility functions ───
//

fn test_tx() -> Transaction {
    Transaction::new(
        Address::fill(0xE1),
        ObjectRef::new(Address::fill(0xF1), ObjectVersion::ONE, Digest::ZERO),
        TransactionType::MeowCall(Call::new(
            Address::fill(0xFD),
            Identifier::new("transfer").unwrap(),
            vec![],
        )),
    )
}
