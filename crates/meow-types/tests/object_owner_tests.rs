use meow_types::{address::Address, object::object_owner::ObjectOwner};

//
// ─── ObjectOwner tests ───
//

#[test]
fn object_owned_by_address() {
    let owner = ObjectOwner::Address(Address::ZERO);

    assert!(!owner.is_immutable());
    assert!(owner.is_address_owned());
    assert_eq!(owner.address().unwrap(), &Address::ZERO);
}

#[test]
fn immutable_object() {
    let owner = ObjectOwner::Immutable;

    assert!(owner.is_immutable());
    assert!(!owner.is_address_owned());
    assert!(owner.address().is_none());
}

//
// ─── ObjectOwner conversion tests ───
//

#[test]
fn object_owner_address_display() {
    let owner = ObjectOwner::Address(Address::ZERO);

    assert_eq!(
        owner.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000"
    );
}

#[test]
fn object_owner_immutable_display() {
    let owner = ObjectOwner::Immutable;

    assert_eq!(owner.to_string(), "immutable");
}
