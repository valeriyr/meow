mod utils;

use meow_framework::{framework_module_objects, meow_object_module, meow_object_module_object};
use meow_types::{
    address::Address,
    config::NATIVE_FUNCTION_NAMES,
    identifier::Identifier,
    object::Object,
    system_framework::meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    transaction::{
        Transaction, call::Call, execution_result::ExecutionStatus, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::builder;
use meow_vm_types::identifier::RESERVED_FUNCTION_NAMES;

//
// ─── Function call tests (resolvers.rs, executor/mod.rs) ───
//

#[test]
fn calling_function_returning_primitive_from_transaction_succeeds() {
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod test;

            pub fn get_value() -> u64 { 42 }
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(module_addr, "get_value", vec![]);

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "pub fn returning a primitive must succeed when called from a transaction"
    );
}

#[test]
fn split_with_insufficient_balance_returns_failure() {
    use meow_framework::framework_module_objects;
    use meow_types::object::Object;
    let [dep_obj, module_obj]: [Object; 2] = framework_module_objects().try_into().unwrap();
    let coin_obj = utils::make_coin_object(Address::fill(0xFF), utils::SENDER, 10);
    let gas_obj = utils::make_gas_coin_object();

    let tx = utils::make_meow_call_transaction(
        "split",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&20u64).unwrap(), // amount > balance
        ],
    );
    let result = utils::execute(&tx, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("The balance is insufficient")),
        "split with insufficient balance must produce Failure, got: {:?}",
        result.status()
    );
    assert!(result.created_objects().is_empty());
    assert!(result.destroyed_objects().is_empty());
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
    assert_eq!(result.changed_objects()[0].address(), &utils::GAS_ADDR);
}

#[test]
fn execute_with_function_not_found_returns_failure() {
    let [dep_obj, module_obj]: [Object; 2] = framework_module_objects().try_into().unwrap();
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_meow_call_transaction("nonexistent_function", vec![]);

    let result = utils::execute(&tx, vec![dep_obj, module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("function 'nonexistent_function' not found in module")),
        "missing function must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn calling_native_function_by_name_returns_failure() {
    // Native functions (meow_vm_transfer, meow_vm_fresh_id, etc.) are not part
    // of the compiled module — they live only in the VM's internal native registry.
    // A transaction targeting a native name must be rejected with "not found in module".
    let mut native_functions = RESERVED_FUNCTION_NAMES.to_vec();
    native_functions.extend(NATIVE_FUNCTION_NAMES);

    for native in native_functions {
        let [dep_obj, module_obj]: [Object; 2] = framework_module_objects().try_into().unwrap();
        let gas_obj = utils::make_gas_coin_object();
        let tx = utils::make_meow_call_transaction(native, vec![]);

        let result = utils::execute(&tx, vec![dep_obj, module_obj, gas_obj]).unwrap();

        assert!(
            matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not found in module")),
            "native '{native}' must not be callable as a transaction target, got: {:?}",
            result.status()
        );
    }
}

#[test]
fn calling_private_function_from_transaction_returns_failure() {
    // Private functions are implementation details and cannot be invoked directly
    // from a transaction — only `pub fn` is part of a module's external interface.
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod priv_test;

            fn secret() -> u64 { 42 }
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(module_addr, "secret", vec![]);

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("private")),
        "calling private fn from transaction must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn calling_function_returning_plain_struct_from_transaction_returns_failure() {
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod test;

            pub struct Point { x: u64, y: u64 }

            pub fn origin() -> Point { Point { x: 0, y: 0 } }
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(module_addr, "origin", vec![]);

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("returns a struct")),
        "pub fn returning a plain struct must be rejected as a transaction entry point, got: {:?}",
        result.status()
    );
}

#[test]
fn calling_function_returning_object_in_tuple_from_transaction_returns_failure() {
    // meow_coin::balance returns (MeowCoin, u64) — the object in the tuple makes it
    // ineligible as a transaction entry point.
    let [dep_obj, module_obj]: [Object; 2] = framework_module_objects().try_into().unwrap();
    let coin_obj = utils::make_coin_object(Address::fill(0xAA), utils::SENDER, 50);
    let gas_obj = utils::make_gas_coin_object();
    let tx =
        utils::make_meow_call_transaction("balance", vec![Input::Object(coin_obj.object_ref())]);

    let result = utils::execute(&tx, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("returns a struct")),
        "pub fn returning an object in a tuple must be rejected as a transaction entry point, got: {:?}",
        result.status()
    );
}

#[test]
fn calling_function_returning_bare_object_from_transaction_returns_failure() {
    // A pub fn with a direct object return type (not in a tuple) must also be rejected.
    let meow_object_module = meow_object_module();
    let test_module = builder::build(
        r#"
            mod test;

            use meow_object@0x01;

            pub struct Coin { id: meow_object::Id, balance: u64 }

            pub fn passthrough(coin: Coin) -> Coin { coin }
        "#,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .expect("test module must compile");
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object(
        module_addr,
        bcs::to_bytes(&test_module).expect("must serialize"),
    );
    let meow_object_obj = meow_object_module_object();
    let coin_obj = utils::make_coin_object(Address::fill(0xAA), utils::SENDER, 50);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(
        module_addr,
        "passthrough",
        vec![Input::Object(coin_obj.object_ref())],
    );

    let result = utils::execute(&tx, vec![meow_object_obj, module_obj, coin_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("returns a struct")),
        "pub fn returning a bare object must be rejected as a transaction entry point, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_with_module_as_argument_returns_failure() {
    let [dep_obj, module_obj]: [Object; 2] = framework_module_objects().try_into().unwrap();
    let gas_obj = utils::make_gas_coin_object();
    // Pass the module object itself as a call argument.
    let tx =
        utils::make_meow_call_transaction("burn", vec![Input::Object(module_obj.object_ref())]);

    let result = utils::execute(&tx, vec![dep_obj, module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("is a module and cannot be used as a call argument")),
        "module as argument must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_with_argument_count_mismatch_returns_failure() {
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod test;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
    );
    let gas_obj = utils::make_gas_coin_object();
    // add expects 2 args; pass only 1.
    let tx = utils::make_call_transaction(Address::ZERO, "add", vec![Input::raw(&1u64).unwrap()]);

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("argument count mismatch")),
        "wrong argument count must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_vm_abort_returns_failure() {
    // meow_vm_abort(condition: bool, code: u64, message: str) — aborts when condition is false.
    let src = r#"
        mod abort_test;

        pub fn do_abort() { meow_vm_abort(false, 1, "abort message"); }
    "#;
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object_from_src(src);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(module_addr, "do_abort", vec![]);

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("abort message")),
        "vm abort must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

//
// ─── Effects tests (effects.rs) ───
//

#[test]
fn fresh_id_not_consumed_returns_failure() {
    // A function that calls meow_vm_fresh_id() but never transfers or destroys
    // the object it is meant for — effects.rs requires all fresh IDs to be consumed.
    let src = r#"
        mod leak_test;

        pub fn generate_id() { let id = meow_vm_fresh_id(); }
    "#;
    let module_addr = Address::ZERO;
    let module_obj = utils::make_module_object_from_src(src);
    let gas_obj = utils::make_gas_coin_object();
    let tx = utils::make_call_transaction(module_addr, "generate_id", vec![]);

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("created object not consumed")),
        "unconsumed fresh ID must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

//
// ─── Gas spending tests (gas.rs) ───
//

#[test]
fn exhausted_gas_coin_goes_to_changed() {
    // Gas coin with balance 0: budget is 0, base gas charge fails immediately,
    // the gas coin survives with balance 0 in changed_objects.
    use meow_types::{
        object::object_version::ObjectVersion, system_framework::meow_coin::meow_coin_object,
    };
    let module_obj = utils::make_module_object_from_src(
        r#"
            mod noop;

            pub fn run() {}
        "#,
    );
    let gas_obj = utils::make_gas_coin_object_at_version_and_balance(ObjectVersion::ZERO, 0);
    let gas_coin_ref = gas_obj.object_ref();
    let call = Call::new(Address::ZERO, Identifier::new("run").unwrap(), vec![]);
    let tx = Transaction::new(utils::SENDER, gas_coin_ref, TransactionType::MeowCall(call));

    let result = utils::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        result
            .changed_objects()
            .iter()
            .any(|o| o.address() == &utils::GAS_ADDR),
        "exhausted gas coin must appear in changed_objects"
    );
    assert!(
        !result
            .destroyed_objects()
            .iter()
            .any(|o| o.address() == &utils::GAS_ADDR),
        "exhausted gas coin must not appear in destroyed_objects"
    );
    // Balance should be floored at 0, not underflowing.
    assert_eq!(
        meow_coin_object::balance_from_object(utils::find_gas_coin(&result)).unwrap(),
        0
    );
}
