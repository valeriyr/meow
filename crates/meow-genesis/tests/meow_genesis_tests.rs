use meow_genesis::Genesis;
use meow_types::{
    object::{object_owner::ObjectOwner, object_type::ObjectType},
    system_framework::meow_coin::{self, MEOW_COIN_MODULE_ADDRESS},
};

#[test]
fn create_genesis() {
    const ADDRESS1: [u8; 32] = [0xAA; 32];
    const ADDRESS2: [u8; 32] = [0xBB; 32];
    const ADDRESS3: [u8; 32] = [0xCC; 32];

    let mint = vec![
        (ADDRESS1.into(), 100),
        (ADDRESS2.into(), 200),
        (ADDRESS3.into(), 300),
    ];

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

    assert_eq!(meow_coin1.owner(), &ObjectOwner::Address(ADDRESS1.into()));
    assert_eq!(meow_coin2.owner(), &ObjectOwner::Address(ADDRESS2.into()));
    assert_eq!(meow_coin3.owner(), &ObjectOwner::Address(ADDRESS3.into()));

    assert_eq!(meow_coin::gas_meow_coin_balance(meow_coin1), Some(100));
    assert_eq!(meow_coin::gas_meow_coin_balance(meow_coin2), Some(200));
    assert_eq!(meow_coin::gas_meow_coin_balance(meow_coin3), Some(300));
}
