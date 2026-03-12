use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_ref::ObjectRef,
        object_type::ObjectType, object_version::ObjectVersion,
    },
    system_framework::meow_coin::{MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_OBJECT_NAME},
};

//
// ObjectRef accessor tests.
//

#[test]
fn known_object_ref() {
    let object = test_object();
    let expected_object_ref = ObjectRef::new(test_address(), ObjectVersion::ZERO, object.digest());

    assert_eq!(object.object_ref(), expected_object_ref);
}

//
// Object content-related tests.
//

#[test]
fn object_is_meow_coin() {
    let object = test_meow_coin_object();

    assert!(object.is_meow_coin());
}

#[test]
fn object_is_not_meow_coin() {
    let object = test_object();

    assert!(!object.is_meow_coin());
}

#[test]
fn object_decl_ref_is_meow_coin() {
    let decl_ref = test_meow_coin_object_decl_ref();

    assert!(decl_ref.is_meow_coin());
}

#[test]
fn object_decl_ref_is_not_meow_coin() {
    let decl_ref = test_random_object_decl_ref();

    assert!(!decl_ref.is_meow_coin());
}

//
// Utility functions.
//

fn test_address() -> Address {
    Address::new([1; 32])
}

fn test_owner() -> ObjectOwner {
    ObjectOwner::Address(Address::new([2; 32]))
}

fn test_object() -> Object {
    Object::new(
        test_address(),
        test_owner(),
        Digest::compute(b"test transaction").unwrap(),
        ObjectVersion::ZERO,
        ObjectType::Module,
        vec![1, 2, 3],
    )
}

fn test_meow_coin_object() -> Object {
    Object::new(
        test_address(),
        test_owner(),
        Digest::compute(b"test transaction").unwrap(),
        ObjectVersion::ZERO,
        ObjectType::Object(test_meow_coin_object_decl_ref()),
        vec![1, 2, 3],
    )
}

fn test_meow_coin_object_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::try_from(MEOW_COIN_OBJECT_NAME).unwrap(),
    )
}

fn test_random_object_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::try_from("Coin").unwrap(),
    )
}
