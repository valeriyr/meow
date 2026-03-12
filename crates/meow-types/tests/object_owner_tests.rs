use meow_types::{address::Address, object::object_owner::ObjectOwner};

//
// ObjectOwner tests.
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
