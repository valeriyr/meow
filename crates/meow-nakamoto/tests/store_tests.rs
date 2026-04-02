use meow_nakamoto::store::Store;
use meow_types::{
    address::Address,
    digest::Digest,
    object::Object,
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
// ─── Utility functions ───
//

const ADDRESS1: Address = Address::fill(0xAA);
const ADDRESS2: Address = Address::fill(0xBB);

fn make_object(addr: Address) -> Object {
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
