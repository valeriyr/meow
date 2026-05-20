mod utils;

use meow_framework::{meow_object_module, meow_object_module_object};
use meow_types::{
    address::Address,
    system_framework::meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    transaction::execution_result::{ExecutionResult, ExecutionStatus},
};
use meow_vm_adapter::{builder, external_context::RandSeed};

use meow_vm_adapter::Value;

//
// ─── meow_vm_rand tests ───
//

#[test]
fn meow_vm_rand_is_deterministic_for_same_seed() {
    let seed = [42u8; 32];
    let result1 = execute_rand_roll(seed);
    let result2 = execute_rand_roll(seed);
    let v1 = read_u64_field(&result1.created_objects()[0], "value");
    let v2 = read_u64_field(&result2.created_objects()[0], "value");
    assert_eq!(v1, v2, "same seed must produce the same random value");
}

#[test]
fn meow_vm_rand_differs_with_different_seeds() {
    let result1 = execute_rand_roll([1u8; 32]);
    let result2 = execute_rand_roll([2u8; 32]);
    let v1 = read_u64_field(&result1.created_objects()[0], "value");
    let v2 = read_u64_field(&result2.created_objects()[0], "value");
    assert_ne!(
        v1, v2,
        "different seeds must produce different random values"
    );
}

//
// ─── meow_vm_timestamp tests ───
//

#[test]
fn meow_vm_timestamp_returns_block_timestamp() {
    let ts = 1_700_000_000u64;
    let result = execute_timestamp_capture(ts);
    let captured = read_u64_field(&result.created_objects()[0], "value");
    assert_eq!(captured, ts, "contract must see the block timestamp");
}

#[test]
fn meow_vm_timestamp_differs_with_different_timestamps() {
    let result1 = execute_timestamp_capture(1_000_000);
    let result2 = execute_timestamp_capture(2_000_000);
    let v1 = read_u64_field(&result1.created_objects()[0], "value");
    let v2 = read_u64_field(&result2.created_objects()[0], "value");
    assert_ne!(
        v1, v2,
        "different block timestamps must produce different values"
    );
}

//
// ─── Utilities ───
//

/// Execute the `roll()` function with the given seed.
fn execute_rand_roll(seed: RandSeed) -> ExecutionResult {
    const RAND_MODULE_SRC: &str = r#"
        mod rand_test;

        use meow_object@0x10;

        struct RandBox { id: meow_object::Id, value: u64 }

        pub fn roll() {
            let box = RandBox { id: meow_vm_fresh_id(), value: meow_vm_rand() };
            meow_vm_transfer(box, meow_vm_sender());
        }
    "#;

    let meow_object_module = meow_object_module();
    let meow_object_obj = meow_object_module_object();
    let module = builder::build(
        RAND_MODULE_SRC,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let module_obj = utils::make_module_object(
        Address::ZERO,
        bcs::to_bytes(&module).expect("must serialize"),
    );
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(Address::ZERO, "roll", vec![]);
    let result =
        utils::execute_with_seed(&tx, vec![meow_object_obj, module_obj, gas_obj], seed).unwrap();
    assert_eq!(result.status(), &ExecutionStatus::Success);
    result
}

/// Execute the `capture()` function with the given block timestamp.
fn execute_timestamp_capture(timestamp: u64) -> ExecutionResult {
    const TIMESTAMP_MODULE_SRC: &str = r#"
        mod timestamp_test;

        use meow_object@0x10;

        struct TimestampBox { id: meow_object::Id, value: u64 }

        pub fn capture() {
            let box = TimestampBox { id: meow_vm_fresh_id(), value: meow_vm_timestamp() };
            meow_vm_transfer(box, meow_vm_sender());
        }
    "#;

    let meow_object_module = meow_object_module();
    let meow_object_obj = meow_object_module_object();
    let module = builder::build(
        TIMESTAMP_MODULE_SRC,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let module_obj = utils::make_module_object(
        Address::ZERO,
        bcs::to_bytes(&module).expect("must serialize"),
    );
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(Address::ZERO, "capture", vec![]);
    let result =
        utils::execute_with_timestamp(&tx, vec![meow_object_obj, module_obj, gas_obj], timestamp)
            .unwrap();
    assert_eq!(result.status(), &ExecutionStatus::Success);
    result
}

//
// ─── Helpers ───
//

fn read_u64_field(obj: &meow_types::object::Object, field: &str) -> u64 {
    let fields: Vec<(String, Value)> = bcs::from_bytes(obj.content()).unwrap();
    fields
        .iter()
        .find(|(name, _)| name == field)
        .and_then(|(_, val)| val.as_u64())
        .unwrap_or_else(|| panic!("field '{field}' not found or not a u64"))
}
