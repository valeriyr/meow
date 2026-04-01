use meow_types::{
    address::Address,
    identifier::Identifier,
    object::{object_decl_ref::ObjectDeclRef, object_type::ObjectType},
};

//
// ─── ObjectType conversion tests ───
//

#[test]
fn object_type_address_display() {
    let type_ = ObjectType::Module;

    assert_eq!(type_.to_string(), "module");
}

#[test]
fn object_type_immutable_display() {
    let type_ = ObjectType::Object(test_object_decl_ref());

    assert_eq!(
        type_.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000::Coin"
    );
}

//
// ─── Utility functions ───
//

fn test_object_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(Address::ZERO, Identifier::try_from("Coin").unwrap())
}
