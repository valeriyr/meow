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
        "S1HprSsfhWRVDkmF1jQ1RR6VhhrzuyuX63QWwEbuBkx"
    );
}

//
// ─── Utility functions ───
//

fn test_tx() -> Transaction {
    Transaction::new(
        Address::suffixed(0xE1),
        ObjectRef::new(Address::suffixed(0xF1), ObjectVersion::ONE, Digest::ZERO),
        TransactionType::MeowCall(Call::new(
            Address::suffixed(0xFD),
            Identifier::new("transfer").unwrap(),
            vec![],
        )),
    )
}
