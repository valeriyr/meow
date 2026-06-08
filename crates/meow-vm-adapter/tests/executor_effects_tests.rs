mod utils;

use meow_types::{
    address::Address,
    object::{Object, object_version::ObjectVersion},
    system_framework::{
        meow_coin::{MEOW_COIN_MODULE_ADDRESS, meow_coin_object},
        meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    },
    transaction::{execution_result::ExecutionStatus, input::Input},
};
use meow_vm_adapter::builder;

//
// ─── Object lifecycle tracking (effects.rs) ───
//

#[test]
fn fresh_object_appears_in_created_objects() {
    // An object produced by meow_vm_fresh_id + meow_vm_transfer must appear in
    // created_objects with the sender as owner.
    let meow_object_module = meow_framework::meow_object_module();
    let meow_object_obj = meow_framework::meow_object_module_object();
    let module = builder::build(
        r#"
            mod effects_test;

            use meow_object@0x10;

            pub struct Widget { id: meow_object::Id, value: u64 }

            pub fn create(value: u64) {
                let w = Widget { id: meow_vm_fresh_id(), value: value };
                meow_vm_transfer(w, meow_vm_sender());
            }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("must compile");
    let module_obj = utils::make_module_object(Address::ZERO, bcs::to_bytes(&module).unwrap());
    let gas_obj = utils::make_gas_coin_object();
    let transaction =
        utils::make_call_transaction(Address::ZERO, "create", vec![Input::raw(&42u64).unwrap()]);

    let result = utils::execute(&transaction, vec![meow_object_obj, module_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "transferred fresh object must appear in created_objects"
    );
    assert_eq!(
        result.created_objects()[0].owner().address(),
        Some(&utils::SENDER)
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
fn transferred_input_object_appears_in_changed_objects() {
    // A coin passed to a function that transfers it back appears in changed_objects
    // with a bumped version.
    let meow_object_mod = meow_framework::meow_object_module();
    let meow_coin_mod = meow_framework::meow_coin_module();
    let module = builder::build(
        &format!(
            r#"
                mod effects_test;

                use meow_coin@{MEOW_COIN_MODULE_ADDRESS};

                pub fn touch(coin: meow_coin::MeowCoin) {{
                    meow_coin::transfer(coin, meow_vm_sender());
                }}
            "#
        ),
        &[
            (MEOW_OBJECT_MODULE_ADDRESS, &meow_object_mod),
            (MEOW_COIN_MODULE_ADDRESS, &meow_coin_mod),
        ],
    )
    .expect("must compile");
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object(module_addr, bcs::to_bytes(&module).unwrap());
    let [fw_object_obj, fw_coin_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();

    let coin_addr = Address::suffixed(0xF1);
    let coin_obj = utils::make_coin_object(coin_addr, utils::SENDER, 42);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(
        module_addr,
        "touch",
        vec![Input::Object(coin_obj.object_ref())],
    );

    let result = utils::execute(
        &transaction,
        vec![fw_object_obj, fw_coin_obj, module_obj, coin_obj, gas_obj],
    )
    .unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert!(result.created_objects().is_empty());
    assert!(result.destroyed_objects().is_empty());
    assert_eq!(
        result.changed_objects().len(),
        2,
        "coin and gas coin must both appear in changed_objects"
    );
    let coin_out = result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &coin_addr)
        .expect("coin must be in changed_objects");
    assert_eq!(
        coin_out.version(),
        &ObjectVersion::ONE.next().unwrap(),
        "transferred input object must have its version bumped"
    );
}

#[test]
fn destroyed_input_object_appears_in_destroyed_objects() {
    // An input object whose ID is destroyed via meow_vm_destroy must appear in
    // destroyed_objects and not in changed_objects (except for the gas coin).
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let coin_obj = utils::make_coin_object(Address::suffixed(0xF1), utils::SENDER, 50);
    let gas_obj = utils::make_gas_coin_object();
    let transaction =
        utils::make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result =
        utils::execute(&transaction, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.destroyed_objects().len(),
        1,
        "destroyed input object must appear in destroyed_objects"
    );
    assert!(result.created_objects().is_empty());
    assert_eq!(
        result.changed_objects().len(),
        1,
        "only the gas coin must be in changed_objects"
    );
    assert_eq!(result.changed_objects()[0].address(), &utils::GAS_ADDR);
}

#[test]
fn aborted_transaction_input_objects_not_in_effects() {
    // When a transaction aborts, input objects must not appear in any effects:
    // no created, no destroyed, and only the gas coin in changed_objects.
    let meow_object_mod = meow_framework::meow_object_module();
    let meow_coin_mod = meow_framework::meow_coin_module();
    let module = builder::build(
        &format!(
            r#"
                mod abort_test;

                use meow_coin@{MEOW_COIN_MODULE_ADDRESS};

                pub fn transfer_then_abort(coin: meow_coin::MeowCoin) {{
                    meow_coin::transfer(coin, meow_vm_sender());
                    meow_vm_abort(false, 1, "abort");
                }}
            "#
        ),
        &[
            (MEOW_OBJECT_MODULE_ADDRESS, &meow_object_mod),
            (MEOW_COIN_MODULE_ADDRESS, &meow_coin_mod),
        ],
    )
    .expect("must compile");
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object(module_addr, bcs::to_bytes(&module).unwrap());
    let [fw_object_obj, fw_coin_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let coin_obj = utils::make_coin_object(Address::suffixed(0xF1), utils::SENDER, 50);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(
        module_addr,
        "transfer_then_abort",
        vec![Input::Object(coin_obj.object_ref())],
    );

    let result = utils::execute(
        &transaction,
        vec![fw_object_obj, fw_coin_obj, module_obj, coin_obj, gas_obj],
    )
    .unwrap();

    assert!(matches!(result.status(), ExecutionStatus::Failure(_)));
    assert!(result.created_objects().is_empty());
    assert!(result.destroyed_objects().is_empty());
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned on abort"
    );
    assert_eq!(result.changed_objects()[0].address(), &utils::GAS_ADDR);
}

//
// ─── Gas deduction (gas.rs) ───
//

#[test]
fn gas_coin_balance_reduced_after_successful_transaction() {
    // After any successful transaction the gas coin must appear in changed_objects
    // with a strictly lower balance.
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod noop;

            pub fn run() {}
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(Address::ZERO, "run", vec![]);

    let result = utils::execute(&transaction, vec![module_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    let final_balance =
        meow_coin_object::balance_from_object(utils::find_gas_coin(&result)).unwrap();
    assert!(
        final_balance < utils::GAS_BALANCE,
        "gas coin balance must decrease after a successful transaction"
    );
}

#[test]
fn gas_coin_version_is_bumped_after_successful_transaction() {
    // The gas coin is modified by every transaction (balance deducted), so its
    // version must increment regardless of what the transaction does.
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod noop;

            pub fn run() {}
        "#,
    );
    let gas_obj = utils::make_gas_coin_object(); // starts at ObjectVersion::ONE
    let transaction = utils::make_call_transaction(Address::ZERO, "run", vec![]);

    let result = utils::execute(&transaction, vec![module_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        utils::find_gas_coin(&result).version(),
        &ObjectVersion::ONE.next().unwrap(),
        "gas coin version must be bumped after a successful transaction"
    );
}
