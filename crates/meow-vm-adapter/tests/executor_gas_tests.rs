//! Exact gas accounting tests for the executor.
//!
//! Each test verifies that `ExecutionResult::gas_used()` equals the expected total:
//! `BASE_TRANSACTION_GAS_COST (1000) + per-instruction VM gas`.
//!
//! The per-instruction values are sourced from `meow_coin_tests.rs`, which
//! measures them through the runner (no base cost added there). The executor
//! adds the base cost on top.
//!
//! Tests also verify the end-to-end invariant: gas coin balance is reduced
//! by exactly `gas_used()`.
//!
//! Genesis transactions are an exception: they skip `BASE_TRANSACTION_GAS_COST`
//! and involve no gas coin, so `gas_used()` equals the raw VM instruction gas.

mod utils;

use meow_framework::framework_module_objects;
use meow_types::{
    address::Address,
    config::MAX_BCS_SERIALIZED_MODULE_SIZE,
    object::Object,
    system_framework::meow_coin::meow_coin_object,
    transaction::{execution_result::ExecutionStatus, input::Input},
};
use meow_vm_adapter::executor;

//
// ─── Base cost ───
//

#[test]
fn noop_transaction_charges_exactly_base_plus_return() {
    // A no-op function contains only a Return instruction (cost 2).
    // Total gas = BASE (1000) + Return (2) = 1002.
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod noop;

            pub fn run() {}
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(Address::ZERO, "run", vec![]);

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), BASE + 2);
    assert_eq!(
        utils::GAS_BALANCE - gas_coin_final_balance(&result),
        result.gas_used(),
        "gas coin balance must be reduced by exactly gas_used()"
    );
}

//
// ─── Gas coin deduction invariant ───
//

#[test]
fn gas_used_always_equals_gas_coin_deduction() {
    // For any successful transaction, the gas coin balance must decrease by
    // exactly gas_used() — the two measurements must be in sync.
    let [dep_obj, module_obj] = framework_objects();
    let coin_obj = coin(0xF1, 75);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_call_transaction(
        "transfer",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result = utils::execute(&tx, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        utils::GAS_BALANCE - gas_coin_final_balance(&result),
        result.gas_used(),
    );
}

//
// ─── Exact gas per MeowCoin operation ───
//

#[test]
fn burn_charges_base_plus_vm_gas() {
    let [dep_obj, module_obj] = framework_objects();
    let coin_obj = coin(0xF1, 100);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result = utils::execute(&tx, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), BASE + 39);
}

#[test]
fn transfer_charges_base_plus_vm_gas() {
    let [dep_obj, module_obj] = framework_objects();
    let coin_obj = coin(0xF1, 75);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_call_transaction(
        "transfer",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result = utils::execute(&tx, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), BASE + 45);
}

#[test]
fn merge_and_transfer_charges_base_plus_vm_gas() {
    let [dep_obj, module_obj] = framework_objects();
    let from_obj = coin(0xF1, 60);
    let to_obj = coin(0xF2, 40);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_call_transaction(
        "merge_and_transfer",
        vec![
            Input::Object(from_obj.object_ref()),
            Input::Object(to_obj.object_ref()),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result = utils::execute(&tx, vec![dep_obj, module_obj, from_obj, to_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), BASE + 93);
}

#[test]
fn split_and_transfer_charges_base_plus_vm_gas() {
    let [dep_obj, module_obj] = framework_objects();
    let coin_obj = coin(0xF1, 100);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_call_transaction(
        "split_and_transfer",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&30u64).unwrap(),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result = utils::execute(&tx, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), BASE + 193);
}

//
// ─── Delegation overhead ───
//

#[test]
fn merge_costs_more_than_merge_and_transfer_due_to_call_overhead() {
    // `merge` is a thin wrapper: it loads both args (1+1), calls meow_vm_sender (20),
    // dispatches to merge_and_transfer (20), then returns (2) — 46 gas of overhead.
    let [dep_obj_a, module_obj_a] = framework_objects();
    let [dep_obj_b, module_obj_b] = framework_objects();
    let gas_a = utils::make_gas_coin_object();
    let gas_b = utils::make_gas_coin_object();

    let from_a = coin(0xF1, 60);
    let to_a = coin(0xF2, 40);
    let tx_mat = utils::make_meow_call_transaction(
        "merge_and_transfer",
        vec![
            Input::Object(from_a.object_ref()),
            Input::Object(to_a.object_ref()),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );
    let result_mat =
        utils::execute(&tx_mat, vec![dep_obj_a, module_obj_a, from_a, to_a, gas_a]).unwrap();

    let from_b = coin(0xF3, 60);
    let to_b = coin(0xF4, 40);
    let tx_m = utils::make_meow_call_transaction(
        "merge",
        vec![
            Input::Object(from_b.object_ref()),
            Input::Object(to_b.object_ref()),
        ],
    );
    let result_m =
        utils::execute(&tx_m, vec![dep_obj_b, module_obj_b, from_b, to_b, gas_b]).unwrap();

    assert_eq!(result_mat.status(), &ExecutionStatus::Success);
    assert_eq!(result_m.status(), &ExecutionStatus::Success);
    assert_eq!(result_m.gas_used(), BASE + 139);
    assert_eq!(result_mat.gas_used(), BASE + 93);
    // merge = merge_and_transfer body + wrapper overhead (Call + meow_vm_sender + Return + loads).
    assert_eq!(result_m.gas_used() - result_mat.gas_used(), 46);
}

//
// ─── Abort path ───
//

#[test]
fn aborted_transaction_charges_base_plus_gas_up_to_abort() {
    // split_and_transfer aborts early when balance is insufficient.
    // Gas consumed before the abort is still charged on top of the base cost.
    let [dep_obj, module_obj] = framework_objects();
    let coin_obj = coin(0xF1, 10); // balance 10 < amount 20 → abort
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_call_transaction(
        "split_and_transfer",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&20u64).unwrap(),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result = utils::execute(&tx, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert!(matches!(result.status(), ExecutionStatus::Failure(_)));
    // VM gas up to abort = 28 (measured via runner probe).
    assert_eq!(result.gas_used(), BASE + 28);
    assert_eq!(
        utils::GAS_BALANCE - gas_coin_final_balance(&result),
        result.gas_used(),
        "gas coin balance must be reduced even on abort"
    );
}

//
// ─── Module publish ───
//

#[test]
fn publish_charges_base_plus_per_byte_gas() {
    // gas_used = BASE (1000) + module_size_bytes * GAS_PER_MODULE_BYTE (10).
    let module_bytes = utils::compile_to_bytes(
        r#"
            mod publish_gas_test;

            fn noop() {}
        "#,
    );
    let module_size = module_bytes.len() as u64;
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_module_publish_transaction(module_bytes);

    let result = utils::execute(&tx, vec![gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), BASE + module_size * 10);
    assert_eq!(
        utils::GAS_BALANCE - gas_coin_final_balance(&result),
        result.gas_used(),
        "gas coin balance must be reduced by exactly gas_used()"
    );
}

#[test]
fn publish_oversized_module_charges_only_base_cost() {
    // Size check happens before per-byte gas is charged.
    let oversized = vec![0u8; MAX_BCS_SERIALIZED_MODULE_SIZE + 1];
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_module_publish_transaction(oversized);

    let result = utils::execute(&tx, vec![gas_obj]).unwrap();

    assert!(matches!(result.status(), ExecutionStatus::Failure(_)));
    assert_eq!(result.gas_used(), BASE);
}

#[test]
fn publish_malformed_module_charges_only_base_cost() {
    // Deserialization fails before per-byte gas is charged.
    let not_a_module = vec![1u8, 2, 3, 4, 5];
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_module_publish_transaction(not_a_module);

    let result = utils::execute(&tx, vec![gas_obj]).unwrap();

    assert!(matches!(result.status(), ExecutionStatus::Failure(_)));
    assert_eq!(result.gas_used(), BASE);
}

//
// ─── Genesis path ───
//

#[test]
fn genesis_transaction_charges_vm_gas_but_not_base_cost() {
    // execute_genesis_transaction skips BASE_TRANSACTION_GAS_COST and involves no
    // gas coin. gas_used() equals the raw VM instruction gas — the same value the
    // runner reports in meow_coin_tests.rs (mint costs 91 gas).
    let [dep_obj, module_obj] = framework_objects();
    let tx = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&100u64).unwrap(),
            Input::raw(&Address::suffixed(0xE1)).unwrap(),
        ],
    );

    let result = executor::execute_genesis_transaction(&tx, vec![dep_obj, module_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.gas_used(), 91);
    assert!(
        result.changed_objects().is_empty(),
        "genesis transaction must not involve a gas coin"
    );
}

//
// ─── Helpers ───
//

const BASE: u64 = 1000;

fn framework_objects() -> [Object; 2] {
    framework_module_objects().try_into().unwrap()
}

fn coin(id: u16, balance: u64) -> Object {
    utils::make_coin_object(Address::suffixed(id), utils::SENDER, balance)
}

fn gas_coin_final_balance(
    result: &meow_types::transaction::execution_result::ExecutionResult,
) -> u64 {
    meow_coin_object::balance_from_object(utils::find_gas_coin(result)).unwrap()
}
