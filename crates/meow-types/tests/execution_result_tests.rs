use meow_types::{
    address::Address,
    digest::Digest,
    object::{Object, object_type::ObjectType, object_version::ObjectVersion},
    transaction::execution_result::{ExecutionResult, ExecutionStatus},
};

//
// ExecutionStatus tests.
//

#[test]
fn execution_status_success_equality() {
    assert_eq!(ExecutionStatus::Success, ExecutionStatus::Success);
}

#[test]
fn execution_status_failure_equality() {
    let e1 = ExecutionStatus::Failure("out of gas".to_string());
    let e2 = ExecutionStatus::Failure("out of gas".to_string());
    assert_eq!(e1, e2);
}

#[test]
fn execution_status_failure_inequality_by_message() {
    let e1 = ExecutionStatus::Failure("out of gas".to_string());
    let e2 = ExecutionStatus::Failure("invalid input".to_string());
    assert_ne!(e1, e2);
}

#[test]
fn execution_status_success_ne_failure() {
    assert_ne!(
        ExecutionStatus::Success,
        ExecutionStatus::Failure("error".to_string())
    );
}

//
// ExecutionResult construction and accessors.
//

#[test]
fn execution_result_status_success() {
    let result = test_result_success();
    assert_eq!(result.status(), &ExecutionStatus::Success);
}

#[test]
fn execution_result_status_failure() {
    let result = ExecutionResult::new(
        ExecutionStatus::Failure("out of gas".to_string()),
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure("out of gas".to_string())
    );
}

#[test]
fn execution_result_transaction_digest() {
    let digest = test_digest();
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        digest.clone(),
        vec![],
        vec![],
        vec![],
    );
    assert_eq!(result.transaction_digest(), &digest);
}

#[test]
fn execution_result_created_objects_empty() {
    let result = test_result_success();
    assert!(result.created_objects().is_empty());
}

#[test]
fn execution_result_changed_objects_empty() {
    let result = test_result_success();
    assert!(result.changed_objects().is_empty());
}

#[test]
fn execution_result_destroyed_objects_empty() {
    let result = test_result_success();
    assert!(result.destroyed_objects().is_empty());
}

#[test]
fn execution_result_created_objects() {
    let obj = test_object(Address::new([10; 32]));
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![obj.clone()],
        vec![],
        vec![],
    );
    assert_eq!(result.created_objects(), &[obj]);
}

#[test]
fn execution_result_changed_objects() {
    let obj = test_object(Address::new([11; 32]));
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![obj.clone()],
        vec![],
    );
    assert_eq!(result.changed_objects(), &[obj]);
}

#[test]
fn execution_result_destroyed_objects() {
    let obj = test_object(Address::new([12; 32]));
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![obj.clone()],
    );
    assert_eq!(result.destroyed_objects(), &[obj]);
}

#[test]
fn execution_result_multiple_objects_per_category() {
    let obj1 = test_object(Address::new([1; 32]));
    let obj2 = test_object(Address::new([2; 32]));
    let obj3 = test_object(Address::new([3; 32]));
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![obj1.clone(), obj2.clone()],
        vec![obj3.clone()],
        vec![],
    );
    assert_eq!(result.created_objects().len(), 2);
    assert_eq!(result.changed_objects().len(), 1);
    assert!(result.destroyed_objects().is_empty());
}

//
// ExecutionResult equality.
//

#[test]
fn execution_result_equality() {
    let r1 = test_result_success();
    let r2 = test_result_success();
    assert_eq!(r1, r2);
}

#[test]
fn execution_result_inequality_by_status() {
    let r1 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    let r2 = ExecutionResult::new(
        ExecutionStatus::Failure("error".to_string()),
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    assert_ne!(r1, r2);
}

#[test]
fn execution_result_inequality_by_digest() {
    let r1 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    let r2 = ExecutionResult::new(
        ExecutionStatus::Success,
        other_digest(),
        vec![],
        vec![],
        vec![],
    );
    assert_ne!(r1, r2);
}

#[test]
fn execution_result_inequality_by_created_objects() {
    let obj = test_object(Address::new([5; 32]));
    let r1 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![obj],
        vec![],
        vec![],
    );
    let r2 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    assert_ne!(r1, r2);
}

#[test]
fn execution_result_inequality_by_changed_objects() {
    let obj = test_object(Address::new([6; 32]));
    let r1 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![obj],
        vec![],
    );
    let r2 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    assert_ne!(r1, r2);
}

#[test]
fn execution_result_inequality_by_destroyed_objects() {
    let obj = test_object(Address::new([7; 32]));
    let r1 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![obj],
    );
    let r2 = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    assert_ne!(r1, r2);
}

//
// Serialization (round-trip).
//

#[test]
fn execution_result_serde_round_trip_success() {
    let result = test_result_success();
    let bytes = bcs::to_bytes(&result).unwrap();
    let decoded: ExecutionResult = bcs::from_bytes(&bytes).unwrap();
    assert_eq!(result, decoded);
}

#[test]
fn execution_result_serde_round_trip_failure() {
    let result = ExecutionResult::new(
        ExecutionStatus::Failure("panic in module".to_string()),
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    let bytes = bcs::to_bytes(&result).unwrap();
    let decoded: ExecutionResult = bcs::from_bytes(&bytes).unwrap();
    assert_eq!(result, decoded);
}

#[test]
fn execution_result_serde_round_trip_with_objects() {
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![test_object(Address::new([1; 32]))],
        vec![test_object(Address::new([2; 32]))],
        vec![test_object(Address::new([3; 32]))],
    );
    let bytes = bcs::to_bytes(&result).unwrap();
    let decoded: ExecutionResult = bcs::from_bytes(&bytes).unwrap();
    assert_eq!(result, decoded);
}

//
// Utility functions.
//

fn test_digest() -> Digest {
    Digest::compute(b"test transaction").unwrap()
}

fn other_digest() -> Digest {
    Digest::compute(b"other transaction").unwrap()
}

fn test_object(address: Address) -> Object {
    Object::new(
        address,
        Address::new([42; 32]),
        test_digest(),
        ObjectVersion::ZERO,
        ObjectType::Module,
        vec![],
    )
}

fn test_result_success() -> ExecutionResult {
    ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    )
}
