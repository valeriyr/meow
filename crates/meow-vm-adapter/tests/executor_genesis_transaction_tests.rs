mod utils;

use meow_framework::framework_module_objects;
use meow_types::{
    address::Address,
    object::Object,
    transaction::{execution_result::ExecutionStatus, input::Input},
};
use meow_vm_adapter::executor;

//
// ─── execute_genesis_transaction ───
//

#[test]
fn genesis_transaction_bypasses_gas_and_allows_private_functions() {
    // execute_genesis_transaction uses a privileged VM config that allows calling
    // private functions (meow_coin::mint is private). It also bypasses gas
    // accounting — changed_objects must be empty (no gas coin involved).
    let [dep_obj, module_obj]: [Object; 2] = framework_module_objects().try_into().unwrap();
    let tx = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&100u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );

    let result = executor::execute_genesis_transaction(&tx, vec![dep_obj, module_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert!(
        result.changed_objects().is_empty(),
        "genesis transaction must not produce a changed gas coin"
    );
    assert_eq!(result.created_objects().len(), 1);
}

#[test]
fn genesis_transaction_reports_zero_gas_used() {
    // execute_genesis_transaction reports zero gas — the VM still runs under an
    // unlimited meter but the cost is not surfaced to the caller.
    let [dep_obj, module_obj]: [Object; 2] = framework_module_objects().try_into().unwrap();
    let tx = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&100u64).unwrap(),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result = executor::execute_genesis_transaction(&tx, vec![dep_obj, module_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), 0);
    assert!(
        result.changed_objects().is_empty(),
        "genesis transaction must not involve a gas coin"
    );
}
