use meow_types::{
    digest::Digest,
    transaction::execution_result::{ExecutionResult, ExecutionStatus},
};

//
// ─── ExecutionStatus tests ───
//

#[test]
fn execution_result_serde_round_trip_failure() {
    let result = ExecutionResult::new(
        ExecutionStatus::Failure("panic in module".to_string()),
        test_digest(),
        vec![],
        vec![],
        vec![],
    )
    .with_gas_used(24);
    let bytes = bcs::to_bytes(&result).unwrap();
    let decoded: ExecutionResult = bcs::from_bytes(&bytes).unwrap();
    assert_eq!(result, decoded);
}

//
// ─── gas_used tests ───
//

#[test]
fn execution_result_default_gas_used_is_zero() {
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    );
    assert_eq!(result.gas_used(), 0);
}

#[test]
fn execution_result_with_gas_used_stores_value() {
    let result = ExecutionResult::new(
        ExecutionStatus::Success,
        test_digest(),
        vec![],
        vec![],
        vec![],
    )
    .with_gas_used(42);

    assert_eq!(result.gas_used(), 42);
}

//
// ─── Utility functions ───
//

fn test_digest() -> Digest {
    Digest::compute(b"test transaction").unwrap()
}
