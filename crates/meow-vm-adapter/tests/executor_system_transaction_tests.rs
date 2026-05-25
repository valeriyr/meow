mod utils;

use meow_types::{
    address::Address,
    object::Object,
    transaction::{execution_result::ExecutionStatus, input::Input},
};
use meow_vm_adapter::{executor, executor::error::ExecutorError};

//
// ─── execute_system_transaction ───
//

#[test]
fn system_transaction_allows_private_functions_and_bypasses_gas() {
    // execute_system_transaction uses a privileged VM config that allows calling
    // private functions (meow_coin::mint is private). It also bypasses gas
    // accounting — changed_objects must be empty (no gas coin involved).
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let transaction = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&100u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );

    let result =
        executor::execute_system_transaction(&transaction, vec![dep_obj, module_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert!(
        result.changed_objects().is_empty(),
        "system transaction must not produce a changed gas coin"
    );
    assert_eq!(result.created_objects().len(), 1);
}

#[test]
fn system_transaction_reports_zero_gas_used() {
    // execute_system_transaction reports zero gas — the VM still runs under an
    // unlimited meter but the cost is not surfaced to the caller.
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let transaction = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&100u64).unwrap(),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result =
        executor::execute_system_transaction(&transaction, vec![dep_obj, module_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), 0);
    assert!(
        result.changed_objects().is_empty(),
        "system transaction must not involve a gas coin"
    );
}

#[test]
fn system_transaction_rejects_module_publish() {
    // Module publishing is not a valid system transaction type — it must be
    // rejected before any execution to prevent privileged package deployment.
    let transaction = utils::make_meow_module_publish_transaction(vec![]);

    let err = executor::execute_system_transaction(&transaction, vec![]).unwrap_err();

    assert!(matches!(err, ExecutorError::ModulePublishNotAllowed));
}
