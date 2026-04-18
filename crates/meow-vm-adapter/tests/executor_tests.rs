use std::str::FromStr;

use meow_types::{
    address::Address,
    config::{MAX_BCS_SERIALIZED_MODULE_SIZE, NATIVE_FUNCTION_NAMES},
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_ref::ObjectRef,
        object_type::ObjectType, object_version::ObjectVersion,
    },
    system_framework::meow_coin::{self, MEOW_COIN_MODULE_ADDRESS},
    transaction::{
        Transaction,
        call::Call,
        execution_result::{ExecutionResult, ExecutionStatus},
        input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::{
    Value, builder,
    executor::{self, error::ExecutorError},
    external_context::{DEFAULT_RAND_SEED, ExternalContext, RandSeed},
};
use meow_vm_types::identifier::RESERVED_FUNCTION_NAMES;

//
// ─── Happy path tests ───
//

#[test]
fn mint_succeeds_and_creates_object() {
    // mint is private — use the genesis execution path (privileged config, no gas deduction).
    let module_obj = make_default_module_object();
    let tx = make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&100u64).unwrap(), // balance
            Input::raw(&SENDER).unwrap(), // owner
        ],
    );

    let result = executor::execute_genesis_transaction(&tx, vec![module_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "mint must create one coin"
    );
    assert_eq!(result.changed_objects().len(), 0); // genesis does not deduct gas
    assert_eq!(result.destroyed_objects().len(), 0);
    let created = &result.created_objects()[0];
    assert_eq!(meow_coin::gas_meow_coin_balance(created).unwrap(), 100);
    assert_eq!(created.owner().address(), Some(&SENDER));
    assert_eq!(result.gas_used(), 91);
}

#[test]
fn burn_succeeds_and_destroys_object() {
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(Address::fill(0xCC), SENDER, 50);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);
    let result = execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.destroyed_objects().len(),
        1,
        "burn must destroy one coin"
    );
    assert_eq!(result.created_objects().len(), 0);
    assert_eq!(result.changed_objects().len(), 1);
    assert_eq!(result.changed_objects()[0].address(), &GAS_ADDR);
    assert_eq!(result.gas_used(), 1034);
}

#[test]
fn transfer_changes_owner() {
    let coin_id = Address::fill(0xDD);
    let new_owner = Address::fill(0x02);
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 75);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction(
        "transfer",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&new_owner).unwrap(),
        ],
    );
    let result = execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.changed_objects().len(), 2);
    assert_eq!(result.created_objects().len(), 0);
    assert_eq!(result.destroyed_objects().len(), 0);
    let transferred = result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &coin_id)
        .unwrap();
    assert_eq!(transferred.owner().address(), Some(&new_owner));
    assert_eq!(meow_coin::gas_meow_coin_balance(transferred).unwrap(), 75);
    assert_eq!(result.gas_used(), 1045);
}

#[test]
fn split_with_sufficient_balance() {
    let coin_id = Address::fill(0xEE);
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 100);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction(
        "split",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&40u64).unwrap(),
        ],
    );
    let result = execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "one new coin must be created"
    );
    let new_coin = &result.created_objects()[0];
    assert_eq!(meow_coin::gas_meow_coin_balance(new_coin).unwrap(), 40);
    assert_eq!(new_coin.owner().address(), Some(&SENDER));

    let original = result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &coin_id)
        .expect("original coin must appear as changed");
    assert_eq!(meow_coin::gas_meow_coin_balance(original).unwrap(), 60);
    assert!(
        result
            .changed_objects()
            .iter()
            .any(|o| o.address() == &GAS_ADDR),
        "gas coin must appear as changed"
    );
    assert_eq!(result.gas_used(), 1150);
}

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
// ─── Gas coin validation tests (resolvers.rs) ───
//

#[test]
fn execute_with_gas_coin_not_found_returns_error() {
    let module_obj = make_default_module_object();
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("mint").unwrap(),
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );
    let tx = Transaction::new(
        SENDER,
        ObjectRef::new(Address::fill(0xFE), ObjectVersion::ONE, Digest::ZERO),
        TransactionType::MeowCall(call),
    );

    let err = execute(&tx, vec![module_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::GasCoinNotFound));
}

#[test]
fn execute_with_invalid_gas_coin_returns_error() {
    let module_obj = make_default_module_object();
    let gas_obj = make_invalid_gas_coin_object();
    let tx = make_meow_call_transaction(
        "mint",
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );

    let err = execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::InvalidGasCoin));
}

#[test]
fn execute_with_invalid_gas_coin_owner_returns_error() {
    let module_obj = make_default_module_object();
    let gas_obj = make_valid_gas_coin_object(Address::fill(0xFF));
    let tx = make_meow_call_transaction(
        "mint",
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );

    let err = execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::InvalidGasCoinOwner));
}

#[test]
fn execute_with_gas_coin_at_max_version_returns_error() {
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object_at_version(ObjectVersion::MAX);
    let tx = make_meow_call_transaction(
        "mint",
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );

    let err = execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::ObjectAtMaxVersion(address) if address == GAS_ADDR));
}

#[test]
fn execute_with_gas_coin_wrong_version_returns_error() {
    // Gas coin is at version ZERO but the transaction references version ONE.
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object_at_version(ObjectVersion::ZERO);
    let tx = make_meow_call_transaction(
        "mint",
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );

    let err = execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(
        err,
        ExecutorError::InvalidObjectVersion { address, expected, found }
            if address == GAS_ADDR
            && expected == ObjectVersion::ONE
            && found == ObjectVersion::ZERO
    ));
}

#[test]
fn execute_with_gas_coin_wrong_digest_returns_error() {
    // Gas coin with correct address and version, but the ObjectRef in the
    // transaction carries a stale / wrong digest.
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object();
    let expected_digest = gas_obj.digest();

    let wrong_ref = ObjectRef::new(GAS_ADDR, ObjectVersion::ONE, Digest::ZERO);
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("mint").unwrap(),
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );
    let tx = Transaction::new(SENDER, wrong_ref, TransactionType::MeowCall(call));

    let err = execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(
        err,
        ExecutorError::InvalidObjectDigest { address, expected, found }
            if address == GAS_ADDR
            && expected == Digest::ZERO
            && found == expected_digest
    ));
}

//
// ─── Module resolution tests (resolvers.rs) ───
//

#[test]
fn execute_meow_call_without_module_returns_failure() {
    // No module object — only the gas coin in inputs.
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction(
        "mint",
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );

    let result = execute(&tx, vec![gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not found in inputs")),
        "missing module must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_meow_call_with_unrelated_module_in_inputs_succeeds() {
    // An unrelated module object in inputs (not declared in main module's imports)
    // must be silently ignored and must not prevent successful execution.
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(Address::fill(0xCC), SENDER, 50);
    let unrelated_bytes = compile_to_bytes(
        r#"
            mod unrelated;
            pub fn noop() {}
        "#,
    );
    let unrelated_obj = make_module_object(Address::fill(0x02), unrelated_bytes);
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result = execute(&tx, vec![module_obj, coin_obj, unrelated_obj, gas_obj]).unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "unrelated module in inputs must not prevent successful execution"
    );
}

#[test]
fn execute_meow_call_with_missing_dep_returns_failure() {
    // Module declares a dependency via `use`, but the dep object is not in inputs.
    // The executor must reject the transaction before entering the VM.
    let (_, _, main_module) = make_dep_chain();

    let main_addr = Address::ZERO;
    let main_bytes = bcs::to_bytes(&main_module).unwrap();
    let module_obj = make_module_object(main_addr, main_bytes);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(main_addr, Identifier::new("run").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    // Dep module object is intentionally absent from inputs.
    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("missing dependency")),
        "missing dep must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_meow_call_with_dep_present_succeeds() {
    // Module declares a dependency and the dep object is in inputs — must succeed.
    let (dep_addr, dep_module, main_module) = make_dep_chain();

    let dep_bytes = bcs::to_bytes(&dep_module).unwrap();
    let dep_obj = make_module_object(Address::from(<[u8; 32]>::from(dep_addr)), dep_bytes);

    let main_addr = Address::ZERO;
    let main_bytes = bcs::to_bytes(&main_module).unwrap();
    let module_obj = make_module_object(main_addr, main_bytes);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(main_addr, Identifier::new("run").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    let result = execute(&tx, vec![module_obj, dep_obj, gas_obj]).unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "dep in inputs must allow successful execution, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_meow_call_transitive_dep_missing_returns_failure() {
    // A → B → C: B is absent from inputs; transitive resolution must fail.
    let (_, c_addr, _, c_module, a_module) = make_three_module_chain();

    let c_bytes = bcs::to_bytes(&c_module).unwrap();
    let c_obj = make_module_object(Address::from(<[u8; 32]>::from(c_addr)), c_bytes);

    let a_addr = Address::ZERO;
    let a_bytes = bcs::to_bytes(&a_module).unwrap();
    let a_obj = make_module_object(a_addr, a_bytes);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(a_addr, Identifier::new("run").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    // B is absent; only A, C, and gas are in inputs.
    let result = execute(&tx, vec![a_obj, c_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("missing dependency")),
        "missing transitive dep must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_meow_call_transitive_dep_present_succeeds() {
    // A → B → C: all three module objects are in inputs; execution must succeed.
    let (b_addr, c_addr, b_module, c_module, a_module) = make_three_module_chain();

    let b_bytes = bcs::to_bytes(&b_module).unwrap();
    let b_obj = make_module_object(Address::from(<[u8; 32]>::from(b_addr)), b_bytes);
    let c_bytes = bcs::to_bytes(&c_module).unwrap();
    let c_obj = make_module_object(Address::from(<[u8; 32]>::from(c_addr)), c_bytes);

    let a_addr = Address::ZERO;
    let a_bytes = bcs::to_bytes(&a_module).unwrap();
    let a_obj = make_module_object(a_addr, a_bytes);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(a_addr, Identifier::new("run").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    let result = execute(&tx, vec![a_obj, b_obj, c_obj, gas_obj]).unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "full transitive dep tree in inputs must succeed, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_meow_call_diamond_dep_succeeds() {
    // Diamond dep: A → {B, C}, B → D, C → D.
    // D is reachable through two paths but is the same module — must succeed.
    let (b_addr, c_addr, d_addr, b_module, c_module, d_module, a_module) = make_diamond_dep_chain();

    let a_bytes = bcs::to_bytes(&a_module).unwrap();
    let b_bytes = bcs::to_bytes(&b_module).unwrap();
    let c_bytes = bcs::to_bytes(&c_module).unwrap();
    let d_bytes = bcs::to_bytes(&d_module).unwrap();

    let a_obj = make_module_object(Address::ZERO, a_bytes);
    let b_obj = make_module_object(Address::from(<[u8; 32]>::from(b_addr)), b_bytes);
    let c_obj = make_module_object(Address::from(<[u8; 32]>::from(c_addr)), c_bytes);
    let d_obj = make_module_object(Address::from(<[u8; 32]>::from(d_addr)), d_bytes);
    let gas_obj = make_gas_coin_object();

    let call = Call::new(Address::ZERO, Identifier::new("run").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    let result = execute(&tx, vec![a_obj, b_obj, c_obj, d_obj, gas_obj]).unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "diamond dep must succeed, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_meow_call_cyclic_dep_returns_failure() {
    // Cycle: A → B → C → B.
    // B and C form a cycle; the executor must detect and reject it.
    // These modules are hand-crafted (the compiler prevents cyclic imports).
    let b_addr = meow_vm_types::address::Address::from_str("0x42").unwrap();
    let c_addr = meow_vm_types::address::Address::from_str("0x43").unwrap();

    // Hand-craft modules with a cycle: B imports C, C imports B.
    let mut b_module = meow_vm_types::module::Module::new("b");
    b_module.imports = vec![c_addr];
    let mut c_module = meow_vm_types::module::Module::new("c");
    c_module.imports = vec![b_addr];

    // A is a normal module that imports B (which pulls in the cycle).
    let mut a_module = meow_vm_types::module::Module::new("a");
    a_module.imports = vec![b_addr];

    let a_obj = make_module_object(Address::ZERO, bcs::to_bytes(&a_module).unwrap());
    let b_obj = make_module_object(
        Address::from(<[u8; 32]>::from(b_addr)),
        bcs::to_bytes(&b_module).unwrap(),
    );
    let c_obj = make_module_object(
        Address::from(<[u8; 32]>::from(c_addr)),
        bcs::to_bytes(&c_module).unwrap(),
    );
    let gas_obj = make_gas_coin_object();

    let call = Call::new(Address::ZERO, Identifier::new("run").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    let result = execute(&tx, vec![a_obj, b_obj, c_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("circular")),
        "cyclic dep must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

//
// ─── Function call tests (resolvers.rs, executor/mod.rs) ───
//

#[test]
fn execute_with_function_not_found_returns_failure() {
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction("nonexistent_function", vec![]);

    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
        let module_obj = make_default_module_object();
        let gas_obj = make_gas_coin_object();
        let tx = make_meow_call_transaction(native, vec![]);

        let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
    let module_obj = make_module_object_from_src(
        r#"
            mod priv_test;
            fn secret() -> u64 { return 42; }
        "#,
    );
    let gas_obj = make_gas_coin_object();
    let call = Call::new(module_addr, Identifier::new("secret").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
fn execute_with_input_object_at_max_version_returns_failure() {
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object_at_version(Address::fill(0xCC), SENDER, 50, ObjectVersion::MAX);
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result = execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("is at the maximum version")),
        "object at max version must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_with_input_object_wrong_version_returns_failure() {
    // Coin is at version ONE but the ObjectRef in the call says ZERO.
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(Address::fill(0xCC), SENDER, 50);
    let gas_obj = make_gas_coin_object();
    let wrong_ref = ObjectRef::new(*coin_obj.address(), ObjectVersion::ZERO, coin_obj.digest());
    let tx = make_meow_call_transaction("burn", vec![Input::Object(wrong_ref)]);

    let result = execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("has invalid version")),
        "wrong version argument must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_with_input_object_wrong_digest_returns_failure() {
    // Coin has correct address and version in the ObjectRef but the digest is wrong.
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(Address::fill(0xCC), SENDER, 50);
    let gas_obj = make_gas_coin_object();
    let wrong_ref = ObjectRef::new(*coin_obj.address(), ObjectVersion::ONE, Digest::ZERO);
    let tx = make_meow_call_transaction("burn", vec![Input::Object(wrong_ref)]);

    let result = execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("has invalid digest")),
        "wrong digest argument must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_with_module_as_argument_returns_failure() {
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object();
    // Pass the module object itself as a call argument.
    let tx = make_meow_call_transaction("burn", vec![Input::Object(module_obj.object_ref())]);

    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object();
    // mint expects 2 args; pass only 1.
    let tx = make_meow_call_transaction("mint", vec![Input::raw(&10u64).unwrap()]);

    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
    let module_obj = make_module_object_from_src(src);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(module_addr, Identifier::new("do_abort").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

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

#[test]
fn split_with_insufficient_balance_returns_failure() {
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(Address::fill(0xFF), SENDER, 10);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction(
        "split",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::raw(&20u64).unwrap(), // amount > balance
        ],
    );
    let result = execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

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
    assert_eq!(result.changed_objects()[0].address(), &GAS_ADDR);
}

//
// ─── Object effects tests (effects.rs) ───
//

#[test]
fn fresh_object_not_consumed_returns_failure() {
    // A function that calls meow_vm_fresh_id() but never transfers or destroys
    // the generated object — effects.rs requires all fresh IDs to be consumed.
    let src = r#"
        mod leak_test;
        pub fn generate_id() { let id = meow_vm_fresh_id(); }
    "#;
    let module_addr = Address::ZERO;
    let module_obj = make_module_object_from_src(src);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(module_addr, Identifier::new("generate_id").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );

    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object_at_version_and_balance(ObjectVersion::ZERO, 0);
    let gas_coin_ref = gas_obj.object_ref();
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("mint").unwrap(),
        vec![Input::raw(&10u64).unwrap(), Input::raw(&SENDER).unwrap()],
    );
    let tx = Transaction::new(SENDER, gas_coin_ref, TransactionType::MeowCall(call));

    let result = execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        result
            .changed_objects()
            .iter()
            .any(|o| o.address() == &GAS_ADDR),
        "exhausted gas coin must appear in changed_objects"
    );
    assert!(
        !result
            .destroyed_objects()
            .iter()
            .any(|o| o.address() == &GAS_ADDR),
        "exhausted gas coin must not appear in destroyed_objects"
    );
    // Balance should be floored at 0, not underflowing.
    assert_eq!(
        meow_coin::gas_meow_coin_balance(find_gas_coin(&result)).unwrap(),
        0
    );
}

//
// ─── Module publish tests ───
//

#[test]
fn execute_module_publish_succeeds() {
    let module_bytes = compile_to_bytes(
        r#"
            mod publish_test;
            fn noop() {}
        "#,
    );
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(module_bytes);

    let result = execute(&tx, vec![gas_obj]).unwrap();

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
    assert_eq!(result.destroyed_objects().len(), 0);
    assert!(
        result
            .changed_objects()
            .iter()
            .any(|o| o.address() == &GAS_ADDR),
        "gas coin must appear in changed_objects"
    );
}

#[test]
fn execute_module_publish_charges_gas_per_byte() {
    let module_bytes = compile_to_bytes(
        r#"
            mod charge_test;
            fn noop() {}
        "#,
    );
    let module_size = module_bytes.len() as u64;
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(module_bytes);

    let result = execute(&tx, vec![gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    let spent = GAS_BALANCE - meow_coin::gas_meow_coin_balance(find_gas_coin(&result)).unwrap();
    // BASE_TRANSACTION_GAS_COST = 1000, GAS_PER_MODULE_BYTE = 10.
    assert!(
        spent == (1000 + module_size * 10),
        "gas charged ({spent}) must cover base cost + per-byte cost"
    );
}

#[test]
fn execute_module_publish_fails_when_module_too_large() {
    let module_size = MAX_BCS_SERIALIZED_MODULE_SIZE + 1;
    let oversized = vec![0u8; module_size];
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(oversized);

    let result = execute(&tx, vec![gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("exceeds maximum")),
        "oversized module must produce Failure, got: {:?}",
        result.status()
    );
    let spent = GAS_BALANCE - meow_coin::gas_meow_coin_balance(find_gas_coin(&result)).unwrap();
    // BASE_TRANSACTION_GAS_COST = 1000.
    assert!(spent == 1000, "gas charged ({spent}) must cover base cost");
}

#[test]
fn execute_module_publish_fails_when_module_not_deserializable() {
    let not_a_module = vec![1u8, 2, 3, 4, 5];
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(not_a_module);

    let result = execute(&tx, vec![gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("failed to deserialize module")),
        "invalid module bytes must produce Failure, got: {:?}",
        result.status()
    );
    let spent = GAS_BALANCE - meow_coin::gas_meow_coin_balance(find_gas_coin(&result)).unwrap();
    // BASE_TRANSACTION_GAS_COST = 1000.
    assert!(spent == 1000, "gas charged ({spent}) must cover base cost");
}

#[test]
fn execute_module_publish_derives_address_from_tx_digest() {
    let module_bytes = compile_to_bytes(
        r#"
            mod addr_test;
            fn noop() {}
        "#,
    );
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(module_bytes);
    let tx_digest = tx.digest();

    let result = execute(&tx, vec![gas_obj]).unwrap();

    let expected_addr = Address::derive(tx_digest, 0, 0);
    assert_eq!(
        result.created_objects()[0].address(),
        &expected_addr,
        "published module address must be derived from transaction digest"
    );
}

//
// ─── Utility functions ───
//

const MEOW_COIN_SRC: &str = include_str!("../../meow-framework/modules/meow_coin.meow");

/// Fixed sender address used in all tests.
const SENDER: Address = Address::fill(0xAA);
/// Fixed gas coin address.
const GAS_ADDR: Address = Address::fill(0xBB);
/// Initial gas coin balance (more than enough for any test).
const GAS_BALANCE: u64 = 1_000_000;

/// Build a two-module dep chain for use in dependency resolution tests.
///
/// Returns `(dep_addr, dep_module, main_module)` where:
/// - `dep_module` is compiled at `dep_addr` (0x42) and exports `fn get() -> u64`
/// - `main_module` imports `dep_module` and exports `fn run() -> u64` (delegates to dep)
/// - `main_module` is intended to be deployed at `Address::ZERO`
fn make_dep_chain() -> (
    meow_vm_types::address::Address,
    meow_vm_types::module::Module,
    meow_vm_types::module::Module,
) {
    let dep_addr = meow_vm_types::address::Address::from_str("0x42").unwrap();
    let dep_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod helper;
            pub fn get() -> u64 { return 42; }
        "#,
        &[],
        meow_vm_types::config::CompilerConfig::default(),
    )
    .expect("dep must compile");
    let main_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod main_mod;
            use helper@0x42;
            pub fn run() -> u64 { return helper::get(); }
        "#,
        &[(dep_addr, &dep_module)],
        meow_vm_types::config::CompilerConfig::default(),
    )
    .expect("main must compile");
    (dep_addr, dep_module, main_module)
}

/// Build a three-module chain for transitive dep resolution tests: A → B → C.
///
/// Returns `(b_addr, c_addr, b_module, c_module, a_module)` where:
/// - c_module: `mod c`, exports `fn get() -> u64` (address 0x42)
/// - b_module: `mod b`, imports c, exports `fn run() -> u64` (address 0x43)
/// - a_module: `mod a`, imports b, exports `fn run() -> u64` (address ZERO)
fn make_three_module_chain() -> (
    meow_vm_types::address::Address,
    meow_vm_types::address::Address,
    meow_vm_types::module::Module,
    meow_vm_types::module::Module,
    meow_vm_types::module::Module,
) {
    let cfg = meow_vm_types::config::CompilerConfig::default();
    let c_addr = meow_vm_types::address::Address::from_str("0x42").unwrap();
    let b_addr = meow_vm_types::address::Address::from_str("0x43").unwrap();
    let c_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod c;
            pub fn get() -> u64 { return 42; }
        "#,
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod b;
            use c@0x42;
            pub fn run() -> u64 { return c::get(); }
        "#,
        &[(c_addr, &c_module)],
        cfg.clone(),
    )
    .expect("b must compile");
    let a_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod a;
            use b@0x43;
            pub fn run() -> u64 { return b::run(); }
        "#,
        &[(b_addr, &b_module), (c_addr, &c_module)],
        cfg,
    )
    .expect("a must compile");
    (b_addr, c_addr, b_module, c_module, a_module)
}

/// Build a diamond dep chain for dep resolution tests: A → {B, C}, B → D, C → D.
///
/// Returns `(b_addr, c_addr, d_addr, b_module, c_module, d_module, a_module)` where:
/// - d_module: `mod d`, exports `fn get() -> u64` (address 0x44)
/// - b_module: `mod b`, imports d, exports `fn run() -> u64` (address 0x42)
/// - c_module: `mod c`, imports d, exports `fn run() -> u64` (address 0x43)
/// - a_module: `mod a`, imports b and c, exports `fn run() -> u64` (address ZERO)
#[allow(clippy::type_complexity)]
fn make_diamond_dep_chain() -> (
    meow_vm_types::address::Address,
    meow_vm_types::address::Address,
    meow_vm_types::address::Address,
    meow_vm_types::module::Module,
    meow_vm_types::module::Module,
    meow_vm_types::module::Module,
    meow_vm_types::module::Module,
) {
    let cfg = meow_vm_types::config::CompilerConfig::default();
    let d_addr = meow_vm_types::address::Address::from_str("0x44").unwrap();
    let b_addr = meow_vm_types::address::Address::from_str("0x42").unwrap();
    let c_addr = meow_vm_types::address::Address::from_str("0x43").unwrap();
    let d_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod d;
            pub fn get() -> u64 { return 42; }
        "#,
        &[],
        cfg.clone(),
    )
    .expect("d must compile");
    let b_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod b;
            use d@0x44;
            pub fn run() -> u64 { return d::get(); }
        "#,
        &[(d_addr, &d_module)],
        cfg.clone(),
    )
    .expect("b must compile");
    let c_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod c;
            use d@0x44;
            pub fn run() -> u64 { return d::get(); }
        "#,
        &[(d_addr, &d_module)],
        cfg.clone(),
    )
    .expect("c must compile");
    let a_module = meow_vm_compiler::Compiler::compile(
        r#"
            mod a;
            use b@0x42;
            use c@0x43;
            pub fn run() -> u64 { return b::run(); }
        "#,
        &[
            (b_addr, &b_module),
            (c_addr, &c_module),
            (d_addr, &d_module),
        ],
        cfg,
    )
    .expect("a must compile");
    (
        b_addr, c_addr, d_addr, b_module, c_module, d_module, a_module,
    )
}

fn make_default_module_object() -> Object {
    // The meow_coin module is always placed at MEOW_COIN_MODULE_ADDRESS so that
    // transactions targeting that address can resolve it.
    let bytes = compile_to_bytes(MEOW_COIN_SRC);
    make_module_object(MEOW_COIN_MODULE_ADDRESS, bytes)
}

/// Compile a complete .meow source string (must include `mod NAME;`) into a
/// module object at `Address::ZERO`. Use this for ad-hoc modules in tests where
/// the call is constructed with a matching `Address::ZERO` module address.
fn make_module_object_from_src(src: &str) -> Object {
    let bytes = compile_to_bytes(src);
    make_module_object(Address::ZERO, bytes)
}

fn make_module_object(address: Address, content: Vec<u8>) -> Object {
    Object::fresh_module(address, Digest::ZERO, content)
}

/// Compile a complete .meow source string and return the BCS-serialized bytes.
/// The source must start with a `mod NAME;` declaration.
fn compile_to_bytes(src: &str) -> Vec<u8> {
    let module = builder::build(src, &[]).expect("must compile");
    bcs::to_bytes(&module).expect("module must serialize")
}

fn make_meow_call_transaction(fn_name: &str, arguments: Vec<Input>) -> Transaction {
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new(fn_name).expect("function name must be a valid identifier"),
        arguments,
    );
    Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    )
}

fn make_meow_module_publish_transaction(module: Vec<u8>) -> Transaction {
    Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowModulePublish(module),
    )
}

fn make_gas_coin_object() -> Object {
    make_valid_gas_coin_object(SENDER)
}

fn make_valid_gas_coin_object(owner: Address) -> Object {
    make_coin_object(GAS_ADDR, owner, GAS_BALANCE)
}

fn make_gas_coin_object_at_version(version: ObjectVersion) -> Object {
    make_gas_coin_object_at_version_and_balance(version, GAS_BALANCE)
}

fn make_gas_coin_object_at_version_and_balance(version: ObjectVersion, balance: u64) -> Object {
    make_coin_object_at_version(GAS_ADDR, SENDER, balance, version)
}

fn make_invalid_gas_coin_object() -> Object {
    Object::new(
        GAS_ADDR,
        ObjectOwner::Address(SENDER),
        Digest::ZERO,
        ObjectVersion::ONE,
        ObjectType::Module,
        vec![],
    )
}

fn make_coin_object(id: Address, owner: Address, balance: u64) -> Object {
    make_coin_object_at_version(id, owner, balance, ObjectVersion::ONE)
}

fn make_coin_object_at_version(
    id: Address,
    owner: Address,
    balance: u64,
    version: ObjectVersion,
) -> Object {
    let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(balance))];
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let ident = Identifier::new("MeowCoin").unwrap();
    let decl_ref = ObjectDeclRef::new(MEOW_COIN_MODULE_ADDRESS, ident);
    Object::new(
        id,
        ObjectOwner::Address(owner),
        Digest::ZERO,
        version,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn find_gas_coin(result: &ExecutionResult) -> &Object {
    result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &GAS_ADDR)
        .expect("gas coin must be in changed_objects")
}

pub fn execute(
    transaction: &Transaction,
    inputs: Vec<Object>,
) -> executor::Result<ExecutionResult> {
    executor::execute(transaction, inputs, &ExternalContext::default())
}

fn execute_with_seed(
    transaction: &Transaction,
    inputs: Vec<Object>,
    seed: RandSeed,
) -> executor::Result<ExecutionResult> {
    executor::execute(transaction, inputs, &ExternalContext::new(seed, 0))
}

fn execute_with_timestamp(
    transaction: &Transaction,
    inputs: Vec<Object>,
    timestamp: u64,
) -> executor::Result<ExecutionResult> {
    executor::execute(
        transaction,
        inputs,
        &ExternalContext::new(DEFAULT_RAND_SEED, timestamp),
    )
}

/// Execute the `roll()` function with the given seed.
fn execute_rand_roll(seed: RandSeed) -> ExecutionResult {
    const RAND_MODULE_SRC: &str = r#"
        mod rand_test;

        object RandBox { id: address, value: u64 }

        pub fn roll() {
            let box = RandBox { id: meow_vm_fresh_id(), value: meow_vm_rand() };
            meow_vm_transfer(box, meow_vm_sender());
        }
        "#;

    let module_obj = make_module_object_from_src(RAND_MODULE_SRC);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(Address::ZERO, Identifier::new("roll").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );
    let result = execute_with_seed(&tx, vec![module_obj, gas_obj], seed).unwrap();
    assert_eq!(result.status(), &ExecutionStatus::Success);
    result
}

/// Execute the `capture()` function with the given block timestamp.
fn execute_timestamp_capture(timestamp: u64) -> ExecutionResult {
    const TIMESTAMP_MODULE_SRC: &str = r#"
        mod timestamp_test;

        object TimestampBox { id: address, value: u64 }

        pub fn capture() {
            let box = TimestampBox { id: meow_vm_fresh_id(), value: meow_vm_timestamp() };
            meow_vm_transfer(box, meow_vm_sender());
        }
        "#;

    let module_obj = make_module_object_from_src(TIMESTAMP_MODULE_SRC);
    let gas_obj = make_gas_coin_object();
    let call = Call::new(Address::ZERO, Identifier::new("capture").unwrap(), vec![]);
    let tx = Transaction::new(
        SENDER,
        make_gas_coin_object().object_ref(),
        TransactionType::MeowCall(call),
    );
    let result = execute_with_timestamp(&tx, vec![module_obj, gas_obj], timestamp).unwrap();
    assert_eq!(result.status(), &ExecutionStatus::Success);
    result
}

/// Read a named `u64` field from the BCS-encoded content of an object.
fn read_u64_field(obj: &Object, field: &str) -> u64 {
    let fields: Vec<(String, Value)> = bcs::from_bytes(obj.content()).unwrap();
    fields
        .iter()
        .find(|(name, _)| name == field)
        .and_then(|(_, val)| val.as_u64())
        .unwrap_or_else(|| panic!("field '{field}' not found or not a u64"))
}
