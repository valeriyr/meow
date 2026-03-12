use meow_types::{
    digest::Digest,
    transaction::execution_result::{ExecutionResult, ExecutionStatus},
};

//
// ExecutionStatus tests.
//

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

//
// Utility functions.
//

fn test_digest() -> Digest {
    Digest::compute(b"test transaction").unwrap()
}
