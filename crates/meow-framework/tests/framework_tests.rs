use meow_framework::{
    framework_module_entries, meow_coin_module_entry, meow_coin_module_object,
    meow_object_module_entry, meow_object_module_object,
};
use meow_types::system_framework::{
    meow_coin::MEOW_COIN_MODULE_ADDRESS, meow_object::MEOW_OBJECT_MODULE_ADDRESS,
};

//
// ─── Module objects ───
//

#[test]
fn meow_object_module_object_has_correct_address() {
    assert_eq!(
        meow_object_module_object().address(),
        &MEOW_OBJECT_MODULE_ADDRESS
    );
}

#[test]
fn meow_coin_module_object_has_correct_address() {
    assert_eq!(
        meow_coin_module_object().address(),
        &MEOW_COIN_MODULE_ADDRESS
    );
}

//
// ─── Module entries ───
//

#[test]
fn meow_object_module_entry_address_matches_object() {
    let (addr, obj) = meow_object_module_entry();

    assert_eq!(addr, MEOW_OBJECT_MODULE_ADDRESS);
    assert_eq!(obj.address(), &addr);
}

#[test]
fn meow_coin_module_entry_address_matches_object() {
    let (addr, obj) = meow_coin_module_entry();

    assert_eq!(addr, MEOW_COIN_MODULE_ADDRESS);
    assert_eq!(obj.address(), &addr);
}

//
// ─── Framework collections ───
//

#[test]
fn framework_module_entries_are_in_dependency_order() {
    let entries = framework_module_entries();

    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].0, MEOW_OBJECT_MODULE_ADDRESS);
    assert_eq!(entries[1].0, MEOW_COIN_MODULE_ADDRESS);
}

#[test]
fn individual_entries_match_framework_entries() {
    let entries = framework_module_entries();

    let (_, meow_object_obj) = meow_object_module_entry();
    let (_, meow_coin_obj) = meow_coin_module_entry();

    assert_eq!(meow_object_obj, entries[0].1);
    assert_eq!(meow_coin_obj, entries[1].1);
}
