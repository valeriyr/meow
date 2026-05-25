#![allow(dead_code)]

use meow_genesis::Genesis;
use meow_nakamoto::store::Store;
use meow_types::{
    address::Address,
    keypair::{KeyPair, signature_scheme::SignatureScheme},
    object::{object_owner::ObjectOwner, object_ref::ObjectRef},
};
use meow_vm_adapter::builder;
use rand::{SeedableRng, rngs::StdRng};

pub fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0x01; 32]))
}

pub fn noop_module_bytes() -> Vec<u8> {
    let module = builder::build(
        r#"
            mod noop;
            
            pub fn noop() {}
        "#,
        &[],
    )
    .expect("noop module must compile");
    bcs::to_bytes(&module).expect("module serialization is infallible")
}

/// Build a genesis that pre-allocates a coin to `owner`, returning the store
/// and the coin's `ObjectRef` ready to be used as a gas coin.
pub fn genesis_store_with_coin(owner: Address) -> (Store, ObjectRef) {
    let genesis = Genesis::build(&[(owner, 10_000)]).expect("genesis must build");
    let coin_ref = genesis
        .objects()
        .iter()
        .find(|o| o.owner() == &ObjectOwner::Address(owner))
        .expect("allocation must produce a coin owned by the address")
        .object_ref();
    let store = Store::with_objects(genesis.objects().iter().cloned());
    (store, coin_ref)
}
