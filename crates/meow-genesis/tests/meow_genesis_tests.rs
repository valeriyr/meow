use meow_genesis::Genesis;
use meow_types::{
    address::Address,
    object::{object_owner::ObjectOwner, object_type::ObjectType},
    system_framework::{
        meow_coin::{self, MEOW_COIN_MODULE_ADDRESS, meow_coin_object},
        meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    },
};

//
// ─── Genesis::build ───
//

#[test]
fn empty_allocations_produces_only_framework_modules() {
    let genesis = Genesis::build(&[]).unwrap();

    // meow_object + meow_coin — no coin objects
    assert_eq!(genesis.objects().len(), 2);
}

#[test]
fn create_genesis() {
    const ADDRESS1: Address = Address::suffixed(0xE1);
    const ADDRESS2: Address = Address::suffixed(0xE2);
    const ADDRESS3: Address = Address::suffixed(0xE3);

    let mint = vec![(ADDRESS1, 100), (ADDRESS2, 200), (ADDRESS3, 300)];

    let genesis = Genesis::build(&mint).unwrap();

    // meow_object module + meow_coin module + 3 coin objects
    assert_eq!(genesis.objects().len(), 5);

    let meow_object_module = &genesis.objects()[0];
    assert_eq!(meow_object_module.address(), &MEOW_OBJECT_MODULE_ADDRESS);
    assert_eq!(meow_object_module.type_(), &ObjectType::Module);

    let meow_coin_module = &genesis.objects()[1];
    assert_eq!(meow_coin_module.address(), &MEOW_COIN_MODULE_ADDRESS);
    assert_eq!(meow_coin_module.type_(), &ObjectType::Module);

    let meow_coin1 = &genesis.objects()[2];
    let meow_coin2 = &genesis.objects()[3];
    let meow_coin3 = &genesis.objects()[4];

    assert!(meow_coin::is_meow_coin_object(meow_coin1));
    assert!(meow_coin::is_meow_coin_object(meow_coin2));
    assert!(meow_coin::is_meow_coin_object(meow_coin3));

    assert_eq!(meow_coin1.owner(), &ObjectOwner::Address(ADDRESS1));
    assert_eq!(meow_coin2.owner(), &ObjectOwner::Address(ADDRESS2));
    assert_eq!(meow_coin3.owner(), &ObjectOwner::Address(ADDRESS3));

    assert_eq!(meow_coin_object::balance_from_object(meow_coin1), Some(100));
    assert_eq!(meow_coin_object::balance_from_object(meow_coin2), Some(200));
    assert_eq!(meow_coin_object::balance_from_object(meow_coin3), Some(300));
}

#[test]
fn identical_allocations_produce_distinct_object_ids() {
    // Two allocations with the same (address, amount) must still mint two separate
    // coins with distinct object IDs. Without per-allocation salting their mint
    // transactions would be identical, deriving the same object ID and silently
    // collapsing into a single object when loaded into a store.
    const ADDRESS: Address = Address::suffixed(0xE1);

    let genesis = Genesis::build(&[(ADDRESS, 100), (ADDRESS, 100)]).unwrap();

    // 2 framework modules + 2 coin objects.
    assert_eq!(genesis.objects().len(), 4);

    let meow_coin1 = &genesis.objects()[2];
    let meow_coin2 = &genesis.objects()[3];

    assert!(meow_coin::is_meow_coin_object(meow_coin1));
    assert!(meow_coin::is_meow_coin_object(meow_coin2));

    // Both belong to the same owner with the same balance...
    assert_eq!(meow_coin1.owner(), &ObjectOwner::Address(ADDRESS));
    assert_eq!(meow_coin2.owner(), &ObjectOwner::Address(ADDRESS));
    assert_eq!(meow_coin_object::balance_from_object(meow_coin1), Some(100));
    assert_eq!(meow_coin_object::balance_from_object(meow_coin2), Some(100));

    // ...but their object IDs must differ — this is the collision guard.
    assert_ne!(
        meow_coin1.address(),
        meow_coin2.address(),
        "identical allocations must not collide on object ID"
    );
}
