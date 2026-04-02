use meow_genesis::Genesis;
use meow_types::{
    address::Address,
    object::{object_owner::ObjectOwner, object_type::ObjectType},
    system_framework::meow_coin::{self, MEOW_COIN_MODULE_ADDRESS},
};

#[test]
fn create_genesis() {
    const ADDRESS1: Address = Address::fill(0xAA);
    const ADDRESS2: Address = Address::fill(0xBB);
    const ADDRESS3: Address = Address::fill(0xCC);

    let mint = vec![(ADDRESS1, 100), (ADDRESS2, 200), (ADDRESS3, 300)];

    let genesis = Genesis::build(&mint).unwrap();

    assert_eq!(genesis.objects().len(), 4);

    let meow_module = &genesis.objects()[0];

    assert_eq!(meow_module.address(), &MEOW_COIN_MODULE_ADDRESS);
    assert_eq!(meow_module.type_(), &ObjectType::Module);

    let meow_coin1 = &genesis.objects()[1];
    let meow_coin2 = &genesis.objects()[2];
    let meow_coin3 = &genesis.objects()[3];

    assert!(meow_coin::is_meow_coin_object(meow_coin1));
    assert!(meow_coin::is_meow_coin_object(meow_coin2));
    assert!(meow_coin::is_meow_coin_object(meow_coin3));

    assert_eq!(meow_coin1.owner(), &ObjectOwner::Address(ADDRESS1));
    assert_eq!(meow_coin2.owner(), &ObjectOwner::Address(ADDRESS2));
    assert_eq!(meow_coin3.owner(), &ObjectOwner::Address(ADDRESS3));

    assert_eq!(meow_coin::gas_meow_coin_balance(meow_coin1), Some(100));
    assert_eq!(meow_coin::gas_meow_coin_balance(meow_coin2), Some(200));
    assert_eq!(meow_coin::gas_meow_coin_balance(meow_coin3), Some(300));
}
