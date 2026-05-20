use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_ref::ObjectRef,
        object_type::ObjectType, object_version::ObjectVersion,
    },
};

//
// ─── Object content-related tests ───
//

#[test]
fn object_is_module() {
    let object = test_module();

    assert_eq!(object.address(), &TEST_ADDRESS);
    assert_eq!(object.owner(), &ObjectOwner::Immutable);
    assert_eq!(object.version(), &ObjectVersion::ONE);
    assert_eq!(object.type_(), &ObjectType::Module);
    assert_eq!(object.content(), &test_content());
}

#[test]
fn simple_object() {
    let object = test_object();

    assert_eq!(object.address(), &TEST_ADDRESS);
    assert_eq!(object.owner(), &ObjectOwner::Address(TEST_OWNER));
    assert_eq!(object.version(), &ObjectVersion::ONE);
    assert_eq!(
        object.type_(),
        &ObjectType::Object(test_other_object_decl_ref())
    );
    assert_eq!(object.content(), &test_content());
}

//
// ─── ObjectRef accessor tests ───
//

#[test]
fn known_object_ref() {
    let object = test_object();
    let expected_object_ref = ObjectRef::new(TEST_ADDRESS, ObjectVersion::ONE, object.digest());

    assert_eq!(object.object_ref(), expected_object_ref);
}

//
// ─── Utility functions ───
//

const TEST_ADDRESS: Address = Address::suffixed(0xF1);
const TEST_OWNER: Address = Address::suffixed(0xE1);

fn test_content() -> Vec<u8> {
    vec![1, 2, 3]
}

fn test_transaction_digest() -> Digest {
    Digest::compute(b"test transaction").unwrap()
}

fn test_module() -> Object {
    Object::fresh_module(TEST_ADDRESS, test_transaction_digest(), test_content())
}

fn test_object() -> Object {
    Object::fresh_object(
        TEST_ADDRESS,
        TEST_OWNER,
        test_transaction_digest(),
        test_other_object_decl_ref(),
        test_content(),
    )
}

fn test_other_object_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(Address::ZERO, Identifier::try_from("Coin").unwrap())
}
