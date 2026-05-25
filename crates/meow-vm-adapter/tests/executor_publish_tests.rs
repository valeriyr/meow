mod utils;

use meow_types::{
    address::Address, config::MAX_BCS_SERIALIZED_MODULE_SIZE, object::object_type::ObjectType,
    transaction::execution_result::ExecutionStatus,
};
use meow_vm_adapter::builder;

//
// ─── Module publish tests ───
//

#[test]
fn execute_module_publish_succeeds() {
    let module_bytes = utils::compile_to_bytes(
        r#"
            mod publish_test;
    
            fn noop() {}
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_meow_module_publish_transaction(module_bytes);

    let result = utils::execute(&transaction, vec![gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "publish must create exactly one module object"
    );
    assert!(
        matches!(result.created_objects()[0].type_(), ObjectType::Module),
        "created object must have type Module"
    );
    assert!(result.destroyed_objects().is_empty());
    assert_eq!(
        result.changed_objects().len(),
        1,
        "only the gas coin must be in changed_objects"
    );
    assert_eq!(result.changed_objects()[0].address(), &utils::GAS_ADDR);
}

#[test]
fn execute_module_publish_fails_when_module_too_large() {
    let module_size = MAX_BCS_SERIALIZED_MODULE_SIZE + 1;
    let oversized = vec![0u8; module_size];
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_meow_module_publish_transaction(oversized);

    let result = utils::execute(&transaction, vec![gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("exceeds maximum")),
        "oversized module must produce Failure, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_module_publish_fails_when_module_not_deserializable() {
    let not_a_module = vec![1u8, 2, 3, 4, 5];
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_meow_module_publish_transaction(not_a_module);

    let result = utils::execute(&transaction, vec![gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("failed to deserialize module")),
        "invalid module bytes must produce Failure, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_module_publish_fails_when_bytecode_invalid() {
    // Compile a valid module, then tamper with the bytecode so it violates
    // a verifier rule (duplicate function name). The executor must reject it.
    let mut module = builder::build(
        r#"
            mod tamper_test;

            pub fn noop() {}
        "#,
        &[],
    )
    .expect("must compile");

    let dup = module.functions[0].clone();
    module.functions.push(dup); // duplicate function name — Phase 1 violation

    let module_bytes = bcs::to_bytes(&module).expect("module must serialize");
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_meow_module_publish_transaction(module_bytes);

    let result = utils::execute(&transaction, vec![gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("bytecode verification failed")),
        "tampered module must be rejected by verifier, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_module_publish_derives_address_from_tx_digest() {
    let module_bytes = utils::compile_to_bytes(
        r#"
            mod addr_test;

            fn noop() {}
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_meow_module_publish_transaction(module_bytes);
    let tx_digest = transaction.digest();

    let result = utils::execute(&transaction, vec![gas_obj]).unwrap();

    let expected_addr = Address::derive(tx_digest, 0, 0);
    assert_eq!(
        result.created_objects()[0].address(),
        &expected_addr,
        "published module address must be derived from transaction digest"
    );
}
