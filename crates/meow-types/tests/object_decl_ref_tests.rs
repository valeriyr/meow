//
// ─── ObjectDeclRef conversion tests ───
//

use meow_types::{
    address::Address, identifier::Identifier, object::object_decl_ref::ObjectDeclRef,
};

#[test]
fn object_decl_ref_display() {
    let object_decl_ref = test_object_decl_ref();

    assert_eq!(
        object_decl_ref.to_string(),
        "0x0000000000000000000000000000000000000000000000000000000000000000::Coin"
    );
}

//
// ─── Utility functions ───
//

fn test_object_decl_ref() -> ObjectDeclRef {
    ObjectDeclRef::new(Address::ZERO, Identifier::try_from("Coin").unwrap())
}
