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
    let obj = make_object(ADDRESS1);

    let store = Store::with_objects([obj.clone()]);

    assert!(store.contains(obj.address()));
    assert_eq!(store.get_object(obj.address()), Some(&obj));
    assert_eq!(store.objects().count(), 1);
}

#[test]
fn apply_created_objects() {
    let mut store = Store::default();

    let obj = make_object(ADDRESS1);
    let execution_result = make_execution_result(vec![obj.clone()], vec![], vec![]);

    store.apply_execution_result(&execution_result);

    assert!(store.contains(obj.address()));
    assert_eq!(store.get_object(obj.address()), Some(&obj));
    assert_eq!(store.objects().count(), 1);
}

#[test]
fn apply_changed_objects_overwrites() {
    let obj_v1 = make_object(ADDRESS1);

    let mut store = Store::with_objects([obj_v1.clone()]);

    let obj_v2 = Object::fresh_module(*obj_v1.address(), Digest::ZERO, vec![]);

    store.apply_execution_result(&make_execution_result(vec![], vec![obj_v2.clone()], vec![]));

    assert!(store.contains(obj_v1.address()));
    assert_eq!(store.get_object(obj_v1.address()), Some(&obj_v2));
    assert_eq!(store.objects().count(), 1);
}

#[test]
fn apply_destroyed_objects_removes() {
    let obj = make_object(ADDRESS1);

    let mut store = Store::with_objects([obj.clone()]);

    store.apply_execution_result(&make_execution_result(vec![], vec![], vec![obj.clone()]));

    assert!(!store.contains(obj.address()));
    assert_eq!(store.get_object(obj.address()), None);
    assert_eq!(store.objects().count(), 0);
}

#[test]
fn apply_does_not_touch_unrelated_objects() {
    let obj_a = make_object(ADDRESS1);
    let obj_b = make_object(ADDRESS2);

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
    let immutable_module = make_module_object(ADDRESS2);

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
// ─── Utility functions ───
//

const ADDRESS1: Address = Address::fill(0xAA);
const ADDRESS2: Address = Address::fill(0xBB);
const ADDRESS3: Address = Address::fill(0xCC);
const OWNER1: Address = Address::fill(0x11);
const OWNER2: Address = Address::fill(0x22);

fn make_object(addr: Address) -> Object {
    Object::fresh_module(addr, Digest::ZERO, vec![])
}

fn make_owned_object(addr: Address, owner: Address) -> Object {
    Object::fresh_object(
        addr,
        owner,
        Digest::ZERO,
        ObjectDeclRef::new(Address::fill(0x1), Identifier::new("Object").unwrap()),
        vec![],
    )
}

fn make_module_object(addr: Address) -> Object {
    Object::fresh_module(addr, Digest::ZERO, vec![])
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
