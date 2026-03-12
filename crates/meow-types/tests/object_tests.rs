use meow_types::{
    address::Address,
    digest::Digest,
    object::{
        Object, identifier::Identifier, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner,
        object_ref::ObjectRef, object_type::ObjectType, object_version::ObjectVersion,
    },
    system_framework::{MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_OBJECT_NAME},
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
fn object_is_gas_coin() {
    let object = test_gas_coin_object();

    assert!(object.is_gas_coin());
}

#[test]
fn object_is_not_gas_coin() {
    let object = test_object();

    assert!(!object.is_gas_coin());
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

fn test_gas_coin_object() -> Object {
    Object::new(
        test_address(),
        test_owner(),
        Digest::compute(b"test transaction").unwrap(),
        ObjectVersion::ZERO,
        ObjectType::Object(ObjectDeclRef::new(
            MEOW_COIN_MODULE_ADDRESS,
            Identifier::try_from(MEOW_COIN_OBJECT_NAME).unwrap(),
        )),
        vec![1, 2, 3],
    )
}
