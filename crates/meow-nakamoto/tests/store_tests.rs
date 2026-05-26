mod utils;

use meow_nakamoto::store::Store;
use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::Object,
    object::object_decl_ref::ObjectDeclRef,
    transaction::execution_result::{ExecutionResult, ExecutionStatus},
};

//
// ─── Store tests ───
//

#[test]
fn with_objects_populates_store() {
    let obj = utils::test_module_object(ADDRESS1);

    let store = Store::with_objects([obj.clone()]);

    assert!(store.contains(obj.address()));
    assert_eq!(store.get_object(obj.address()), Some(&obj));
    assert_eq!(store.objects().count(), 1);
}

#[test]
fn apply_created_objects() {
    let mut store = Store::default();

    let obj = utils::test_module_object(ADDRESS1);
    let execution_result = make_execution_result(vec![obj.clone()], vec![], vec![]);

    store.apply_execution_result(&execution_result);

    assert!(store.contains(obj.address()));
    assert_eq!(store.get_object(obj.address()), Some(&obj));
    assert_eq!(store.objects().count(), 1);
}

#[test]
fn apply_changed_objects_overwrites() {
    let obj_v1 = utils::test_module_object(ADDRESS1);

    let mut store = Store::with_objects([obj_v1.clone()]);

    let obj_v2 = utils::test_module_object(*obj_v1.address());

    store.apply_execution_result(&make_execution_result(vec![], vec![obj_v2.clone()], vec![]));

    assert!(store.contains(obj_v1.address()));
    assert_eq!(store.get_object(obj_v1.address()), Some(&obj_v2));
    assert_eq!(store.objects().count(), 1);
}

#[test]
fn apply_destroyed_objects_removes() {
    let obj = utils::test_module_object(ADDRESS1);

    let mut store = Store::with_objects([obj.clone()]);

    store.apply_execution_result(&make_execution_result(vec![], vec![], vec![obj.clone()]));

    assert!(!store.contains(obj.address()));
    assert_eq!(store.get_object(obj.address()), None);
    assert_eq!(store.objects().count(), 0);
}

#[test]
fn apply_does_not_touch_unrelated_objects() {
    let obj_a = utils::test_module_object(ADDRESS1);
    let obj_b = utils::test_module_object(ADDRESS2);

    let mut store = Store::with_objects([obj_a.clone(), obj_b.clone()]);

    store.apply_execution_result(&make_execution_result(vec![], vec![], vec![obj_a.clone()]));

    assert!(!store.contains(obj_a.address()));
    assert_eq!(store.get_object(obj_a.address()), None);
    assert!(store.contains(obj_b.address()));
    assert_eq!(store.get_object(obj_b.address()), Some(&obj_b));
    assert_eq!(store.objects().count(), 1);
}

//
// ─── Get Objects tests ───
//

#[test]
fn get_objects_returns_empty_for_unknown_owner() {
    let obj = make_owned_object(ADDRESS1, OWNER1);
    let store = Store::with_objects([obj]);

    let objects: Vec<_> = store.get_objects(&OWNER2).collect();

    assert!(objects.is_empty());
}

#[test]
fn get_objects_returns_single_object_owned_by_address() {
    let obj = make_owned_object(ADDRESS1, OWNER1);
    let store = Store::with_objects([obj.clone()]);

    let objects: Vec<_> = store.get_objects(&OWNER1).collect();

    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].address(), obj.address());
    assert_eq!(objects[0].owner().address(), Some(&OWNER1));
}

#[test]
fn get_objects_returns_multiple_objects_owned_by_address() {
    let obj_a = make_owned_object(ADDRESS1, OWNER1);
    let obj_b = make_owned_object(ADDRESS2, OWNER1);
    let obj_c = make_owned_object(ADDRESS3, OWNER2);

    let store = Store::with_objects([obj_a.clone(), obj_b.clone(), obj_c]);

    let objects: Vec<_> = store.get_objects(&OWNER1).collect();

    assert_eq!(objects.len(), 2);
    let addresses: Vec<_> = objects.iter().map(|o| o.address()).collect();
    assert!(addresses.contains(&obj_a.address()));
    assert!(addresses.contains(&obj_b.address()));

    for obj in objects {
        assert_eq!(obj.owner().address(), Some(&OWNER1));
    }
}

#[test]
fn get_objects_filters_out_immutable_objects() {
    let owned_obj = make_owned_object(ADDRESS1, OWNER1);
    let immutable_module = utils::test_module_object(ADDRESS2);

    let store = Store::with_objects([owned_obj.clone(), immutable_module]);

    let objects: Vec<_> = store.get_objects(&OWNER1).collect();

    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].address(), owned_obj.address());
}

#[test]
fn get_objects_works_after_apply_execution_result() {
    let initial_obj = make_owned_object(ADDRESS1, OWNER1);
    let mut store = Store::with_objects([initial_obj.clone()]);

    let new_obj = make_owned_object(ADDRESS2, OWNER1);
    store.apply_execution_result(&make_execution_result(
        vec![new_obj.clone()],
        vec![],
        vec![],
    ));

    let objects: Vec<_> = store.get_objects(&OWNER1).collect();

    assert_eq!(objects.len(), 2);
    let addresses: std::collections::HashSet<_> = objects.iter().map(|o| *o.address()).collect();
    assert!(addresses.contains(initial_obj.address()));
    assert!(addresses.contains(new_obj.address()));
}

//
// ─── apply_execution_result — invariant violations ───
//

/// Creating an object whose address is already in the store is an invariant violation.
#[test]
#[should_panic(expected = "created object ID collision")]
fn apply_panics_on_created_object_id_collision() {
    let obj = utils::test_module_object(ADDRESS1);
    let mut store = Store::with_objects([obj.clone()]);

    store.apply_execution_result(&make_execution_result(vec![obj], vec![], vec![]));
}

/// Changing an object that does not exist in the store is an invariant violation.
#[test]
#[should_panic(expected = "changed object not found in store")]
fn apply_panics_when_changed_object_is_not_in_store() {
    let mut store = Store::default();

    store.apply_execution_result(&make_execution_result(
        vec![],
        vec![utils::test_module_object(ADDRESS1)],
        vec![],
    ));
}

/// Destroying an object that does not exist in the store is an invariant violation.
#[test]
#[should_panic(expected = "destroyed object not found in store")]
fn apply_panics_when_destroyed_object_is_not_in_store() {
    let mut store = Store::default();

    store.apply_execution_result(&make_execution_result(
        vec![],
        vec![],
        vec![utils::test_module_object(ADDRESS1)],
    ));
}

//
// ─── objects() ordering ───
//

/// `objects()` must return all objects in ascending address order regardless of insertion order,
/// because `compute_state_root` hashes them in iterator order — a different order on two nodes
/// would produce a different state root and cause a consensus split.
#[test]
fn objects_are_returned_in_sorted_address_order() {
    // Insert in reverse order to prove ordering is not insertion-dependent.
    let store = Store::with_objects([
        utils::test_module_object(ADDRESS3),
        utils::test_module_object(ADDRESS1),
        utils::test_module_object(ADDRESS2),
    ]);

    let addresses: Vec<_> = store.objects().map(|o| *o.address()).collect();

    assert_eq!(addresses, vec![ADDRESS1, ADDRESS2, ADDRESS3]);
}

//
// ─── with_objects — duplicate address ───
//

/// When two objects share the same address the last one wins (BTreeMap insert semantics).
#[test]
fn with_objects_last_object_wins_on_duplicate_address() {
    let obj_a = Object::fresh_module(ADDRESS1, Digest::ZERO, vec![1]);
    let obj_b = Object::fresh_module(ADDRESS1, Digest::ZERO, vec![2]);

    let store = Store::with_objects([obj_a, obj_b.clone()]);

    assert_eq!(store.objects().count(), 1);
    assert_eq!(store.get_object(&ADDRESS1), Some(&obj_b));
}

//
// ─── Utility functions ───
//

const ADDRESS1: Address = Address::suffixed(0xF1);
const ADDRESS2: Address = Address::suffixed(0xF2);
const ADDRESS3: Address = Address::suffixed(0xF3);
const OWNER1: Address = Address::suffixed(0xE1);
const OWNER2: Address = Address::suffixed(0xE2);

fn make_owned_object(addr: Address, owner: Address) -> Object {
    Object::fresh_object(
        addr,
        owner,
        Digest::ZERO,
        ObjectDeclRef::new(Address::suffixed(0xFD), Identifier::new("Object").unwrap()),
        vec![],
    )
}

fn make_execution_result(
    created: Vec<Object>,
    changed: Vec<Object>,
    destroyed: Vec<Object>,
) -> ExecutionResult {
    ExecutionResult::new(
        ExecutionStatus::Success,
        Digest::ZERO,
        created,
        changed,
        destroyed,
    )
}
