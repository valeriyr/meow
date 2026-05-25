mod utils;

use std::str::FromStr;

use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{Object, object_ref::ObjectRef, object_version::ObjectVersion},
    system_framework::meow_coin::MEOW_COIN_MODULE_ADDRESS,
    transaction::{
        Transaction, call::Call, execution_result::ExecutionStatus, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::executor::error::ExecutorError;
use meow_vm_compiler::Compiler;
use meow_vm_types::{address::Address as VmAddress, config::CompilerConfig, module::Module};

//
// ─── Gas coin validation tests (resolvers.rs) ───
//

#[test]
fn execute_with_gas_coin_not_found_returns_error() {
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("mint").unwrap(),
        vec![
            Input::raw(&10u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );
    let transaction = Transaction::new(
        utils::SENDER,
        ObjectRef::new(Address::suffixed(0xF9), ObjectVersion::ONE, Digest::ZERO),
        TransactionType::MeowCall(call),
    );

    let err = utils::execute(&transaction, vec![dep_obj, module_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::GasCoinNotFound));
}

#[test]
fn execute_with_invalid_gas_coin_returns_error() {
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let gas_obj = utils::make_invalid_gas_coin_object();
    let transaction = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&10u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );

    let err = utils::execute(&transaction, vec![dep_obj, module_obj, gas_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::InvalidGasCoin));
}

#[test]
fn execute_with_invalid_gas_coin_owner_returns_error() {
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let gas_obj = utils::make_valid_gas_coin_object(Address::suffixed(0xE2));
    let transaction = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&10u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );

    let err = utils::execute(&transaction, vec![dep_obj, module_obj, gas_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::InvalidGasCoinOwner));
}

#[test]
fn execute_with_gas_coin_at_max_version_returns_error() {
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let gas_obj = utils::make_gas_coin_object_at_version(ObjectVersion::MAX);
    let transaction = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&10u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );

    let err = utils::execute(&transaction, vec![dep_obj, module_obj, gas_obj]).unwrap_err();
    assert!(
        matches!(err, ExecutorError::ObjectAtMaxVersion(address) if address == utils::GAS_ADDR)
    );
}

#[test]
fn execute_with_gas_coin_wrong_version_returns_error() {
    // Gas coin is at version ZERO but the transaction references version ONE.
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let gas_obj = utils::make_gas_coin_object_at_version(ObjectVersion::ZERO);
    let transaction = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&10u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );

    let err = utils::execute(&transaction, vec![dep_obj, module_obj, gas_obj]).unwrap_err();
    assert!(matches!(
        err,
        ExecutorError::InvalidObjectVersion { address, expected, found }
            if address == utils::GAS_ADDR
            && expected == ObjectVersion::ONE
            && found == ObjectVersion::ZERO
    ));
}

#[test]
fn execute_with_gas_coin_wrong_digest_returns_error() {
    // Gas coin with correct address and version, but the ObjectRef in the
    // transaction carries a stale / wrong digest.
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let gas_obj = utils::make_gas_coin_object();
    let expected_digest = gas_obj.digest();

    let wrong_ref = ObjectRef::new(utils::GAS_ADDR, ObjectVersion::ONE, Digest::ZERO);
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("mint").unwrap(),
        vec![
            Input::raw(&10u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );
    let transaction = Transaction::new(utils::SENDER, wrong_ref, TransactionType::MeowCall(call));

    let err = utils::execute(&transaction, vec![dep_obj, module_obj, gas_obj]).unwrap_err();
    assert!(matches!(
        err,
        ExecutorError::InvalidObjectDigest { address, expected, found }
            if address == utils::GAS_ADDR
            && expected == Digest::ZERO
            && found == expected_digest
    ));
}

//
// ─── Module resolution tests (resolvers.rs) ───
//

#[test]
fn execute_meow_call_with_unrelated_module_in_inputs_succeeds() {
    // An unrelated module object in inputs (not declared in main module's imports)
    // must be silently ignored and must not prevent successful execution.
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let coin_obj = utils::make_coin_object(Address::suffixed(0xF1), utils::SENDER, 50);
    let unrelated_bytes = utils::compile_to_bytes(
        r#"
            mod unrelated;

            pub fn noop() {}
        "#,
    );
    let unrelated_obj = utils::make_module_object(Address::suffixed(0xF2), unrelated_bytes);
    let gas_obj = utils::make_gas_coin_object();
    let transaction =
        utils::make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result = utils::execute(
        &transaction,
        vec![dep_obj, module_obj, coin_obj, unrelated_obj, gas_obj],
    )
    .unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "unrelated module in inputs must not prevent successful execution"
    );
}

#[test]
fn execute_meow_call_with_dep_present_succeeds() {
    // Module declares a dependency and the dep object is in inputs — must succeed.
    let (dep_addr, dep_module, main_module) = make_dep_chain();

    let dep_bytes = bcs::to_bytes(&dep_module).unwrap();
    let dep_obj = utils::make_module_object(Address::from(dep_addr), dep_bytes);

    let main_addr = Address::ZERO;
    let main_bytes = bcs::to_bytes(&main_module).unwrap();
    let module_obj = utils::make_module_object(main_addr, main_bytes);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(main_addr, "run", vec![]);

    let result = utils::execute(&transaction, vec![module_obj, dep_obj, gas_obj]).unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "dep in inputs must allow successful execution, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_meow_call_transitive_dep_present_succeeds() {
    // A → B → C: all three module objects are in inputs; execution must succeed.
    let (b_addr, c_addr, b_module, c_module, a_module) = make_three_module_chain();

    let b_bytes = bcs::to_bytes(&b_module).unwrap();
    let b_obj = utils::make_module_object(Address::from(b_addr), b_bytes);
    let c_bytes = bcs::to_bytes(&c_module).unwrap();
    let c_obj = utils::make_module_object(Address::from(c_addr), c_bytes);

    let a_addr = Address::ZERO;
    let a_bytes = bcs::to_bytes(&a_module).unwrap();
    let a_obj = utils::make_module_object(a_addr, a_bytes);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(a_addr, "run", vec![]);

    let result = utils::execute(&transaction, vec![a_obj, b_obj, c_obj, gas_obj]).unwrap();

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

    let a_obj = utils::make_module_object(Address::ZERO, a_bytes);
    let b_obj = utils::make_module_object(Address::from(b_addr), b_bytes);
    let c_obj = utils::make_module_object(Address::from(c_addr), c_bytes);
    let d_obj = utils::make_module_object(Address::from(d_addr), d_bytes);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(Address::ZERO, "run", vec![]);

    let result = utils::execute(&transaction, vec![a_obj, b_obj, c_obj, d_obj, gas_obj]).unwrap();

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "diamond dep must succeed, got: {:?}",
        result.status()
    );
}

#[test]
fn execute_meow_call_without_module_returns_failure() {
    // No module object — only the gas coin in inputs.
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_meow_call_transaction(
        "mint",
        vec![
            Input::raw(&10u64).unwrap(),
            Input::raw(&utils::SENDER).unwrap(),
        ],
    );

    let result = utils::execute(&transaction, vec![gas_obj]).unwrap();

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
fn execute_meow_call_with_missing_dep_returns_failure() {
    // Module declares a dependency via `use`, but the dep object is not in inputs.
    // The executor must reject the transaction before entering the VM.
    let (_, _, main_module) = make_dep_chain();

    let main_addr = Address::ZERO;
    let main_bytes = bcs::to_bytes(&main_module).unwrap();
    let module_obj = utils::make_module_object(main_addr, main_bytes);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(main_addr, "run", vec![]);

    // Dep module object is intentionally absent from inputs.
    let result = utils::execute(&transaction, vec![module_obj, gas_obj]).unwrap();

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
fn execute_meow_call_transitive_dep_missing_returns_failure() {
    // A → B → C: B is absent from inputs; transitive resolution must fail.
    let (_, c_addr, _, c_module, a_module) = make_three_module_chain();

    let c_bytes = bcs::to_bytes(&c_module).unwrap();
    let c_obj = utils::make_module_object(Address::from(c_addr), c_bytes);

    let a_addr = Address::ZERO;
    let a_bytes = bcs::to_bytes(&a_module).unwrap();
    let a_obj = utils::make_module_object(a_addr, a_bytes);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(a_addr, "run", vec![]);

    // B is absent; only A, C, and gas are in inputs.
    let result = utils::execute(&transaction, vec![a_obj, c_obj, gas_obj]).unwrap();

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

//
// ─── Object argument validation (resolvers.rs) ───
//

#[test]
fn execute_with_object_arg_absent_from_inputs_returns_failure() {
    // The transaction references a coin as a call argument but the coin object
    // is not included in the inputs slice — resolve_arg must fail before the VM runs.
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let coin_obj = utils::make_coin_object(Address::suffixed(0xF1), utils::SENDER, 50);
    let gas_obj = utils::make_gas_coin_object();
    // Coin is referenced in the call but intentionally absent from inputs.
    let transaction =
        utils::make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result = utils::execute(&transaction, vec![dep_obj, module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("not found")),
        "missing input object must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

#[test]
fn execute_with_raw_input_for_struct_parameter_returns_failure() {
    // burn expects a meow_coin::MeowCoin (Struct type), but the transaction passes
    // Input::Raw bytes — resolve_arg must reject the type mismatch.
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_meow_call_transaction("burn", vec![Input::raw(&42u64).unwrap()]);

    let result = utils::execute(&transaction, vec![dep_obj, module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("Raw input cannot be resolved")),
        "raw bytes for struct parameter must produce Failure, got: {:?}",
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
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let coin_obj = utils::make_coin_object_at_version(
        Address::suffixed(0xF1),
        utils::SENDER,
        50,
        ObjectVersion::MAX,
    );
    let gas_obj = utils::make_gas_coin_object();
    let transaction =
        utils::make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result =
        utils::execute(&transaction, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

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
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let coin_obj = utils::make_coin_object(Address::suffixed(0xF1), utils::SENDER, 50);
    let gas_obj = utils::make_gas_coin_object();
    let wrong_ref = ObjectRef::new(*coin_obj.address(), ObjectVersion::ZERO, coin_obj.digest());
    let transaction = utils::make_meow_call_transaction("burn", vec![Input::Object(wrong_ref)]);

    let result =
        utils::execute(&transaction, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

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
    let [dep_obj, module_obj]: [Object; 2] = meow_framework::framework_module_objects()
        .try_into()
        .unwrap();
    let coin_obj = utils::make_coin_object(Address::suffixed(0xF1), utils::SENDER, 50);
    let gas_obj = utils::make_gas_coin_object();
    let wrong_ref = ObjectRef::new(*coin_obj.address(), ObjectVersion::ONE, Digest::ZERO);
    let transaction = utils::make_meow_call_transaction("burn", vec![Input::Object(wrong_ref)]);

    let result =
        utils::execute(&transaction, vec![dep_obj, module_obj, coin_obj, gas_obj]).unwrap();

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

//
// ─── Corrupt module content tests (resolvers.rs) ───
//

#[test]
fn execute_with_corrupt_module_content_returns_failure() {
    // A module object whose BCS content cannot be deserialized must produce a
    // Failure before entering the VM. The gas coin is still returned.
    let corrupt_module_obj = utils::make_module_object(Address::ZERO, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(Address::ZERO, "run", vec![]);

    let result = utils::execute(&transaction, vec![corrupt_module_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("failed to deserialize module")),
        "corrupt module content must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
    assert_eq!(result.changed_objects()[0].address(), &utils::GAS_ADDR);
}

#[test]
fn execute_with_corrupt_dependency_content_returns_failure() {
    // A dependency module object whose BCS content cannot be deserialized must
    // produce a Failure. The executor resolves all declared deps before entering
    // the VM, so corruption is caught even if the function never calls into the dep.
    let (dep_addr, _, main_module) = make_dep_chain();

    let main_bytes = bcs::to_bytes(&main_module).unwrap();
    let main_obj = utils::make_module_object(Address::ZERO, main_bytes);
    let corrupt_dep_obj = utils::make_module_object(
        Address::from(<[u8; 32]>::from(dep_addr)),
        vec![0xDE, 0xAD, 0xBE, 0xEF],
    );
    let gas_obj = utils::make_gas_coin_object();
    let transaction = utils::make_call_transaction(Address::ZERO, "run", vec![]);

    let result = utils::execute(&transaction, vec![main_obj, corrupt_dep_obj, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("failed to deserialize dependency module")),
        "corrupt dependency content must produce Failure, got: {:?}",
        result.status()
    );
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must still be returned"
    );
}

//
// ─── Dep chain builders ───
//

/// Build a two-module dep chain: dep (0xFD) → main (ZERO).
///
/// Returns `(dep_addr, dep_module, main_module)`.
fn make_dep_chain() -> (VmAddress, Module, Module) {
    let d_addr = VmAddress::from_str("0xFD").unwrap();
    let dep_module = Compiler::compile(
        r#"
            mod helper;

            pub fn get() -> u64 { 42 }
        "#,
        &[],
        &[],
        CompilerConfig::default(),
    )
    .expect("dep must compile");
    let main_module = Compiler::compile(
        r#"
            mod main_mod;

            use helper@0xFD;

            pub fn run() -> u64 { helper::get() }
        "#,
        &[(d_addr, &dep_module)],
        &[],
        CompilerConfig::default(),
    )
    .expect("main must compile");
    (d_addr, dep_module, main_module)
}

/// Build a three-module chain: A (ZERO) → B (0xFB) → C (0xFC).
///
/// Returns `(b_addr, c_addr, b_module, c_module, a_module)`.
fn make_three_module_chain() -> (VmAddress, VmAddress, Module, Module, Module) {
    let cfg = CompilerConfig::default();
    let c_addr = VmAddress::from_str("0xFC").unwrap();
    let b_addr = VmAddress::from_str("0xFB").unwrap();
    let c_module = Compiler::compile(
        r#"
            mod c;

            pub fn get() -> u64 { 42 }
        "#,
        &[],
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = Compiler::compile(
        r#"
            mod b;

            use c@0xFC;

            pub fn run() -> u64 { c::get() }
        "#,
        &[(c_addr, &c_module)],
        &[],
        cfg.clone(),
    )
    .expect("b must compile");
    let a_module = Compiler::compile(
        r#"
            mod a;

            use b@0xFB;

            pub fn run() -> u64 { b::run() }
        "#,
        &[(b_addr, &b_module), (c_addr, &c_module)],
        &[],
        cfg,
    )
    .expect("a must compile");
    (b_addr, c_addr, b_module, c_module, a_module)
}

/// Build a diamond dep chain: A (ZERO) → {B (0xFB), C (0xFC)}, B → D (0xFD), C → D.
///
/// Returns `(b_addr, c_addr, d_addr, b_module, c_module, d_module, a_module)`.
#[allow(clippy::type_complexity)]
fn make_diamond_dep_chain() -> (
    VmAddress,
    VmAddress,
    VmAddress,
    Module,
    Module,
    Module,
    Module,
) {
    let cfg = CompilerConfig::default();
    let d_addr = VmAddress::from_str("0xFD").unwrap();
    let b_addr = VmAddress::from_str("0xFB").unwrap();
    let c_addr = VmAddress::from_str("0xFC").unwrap();
    let d_module = Compiler::compile(
        r#"
            mod d;

            pub fn get() -> u64 { 42 }
        "#,
        &[],
        &[],
        cfg.clone(),
    )
    .expect("d must compile");
    let b_module = Compiler::compile(
        r#"
            mod b;

            use d@0xFD;

            pub fn run() -> u64 { d::get() }
        "#,
        &[(d_addr, &d_module)],
        &[],
        cfg.clone(),
    )
    .expect("b must compile");
    let c_module = Compiler::compile(
        r#"
            mod c;

            use d@0xFD;

            pub fn run() -> u64 { d::get() }
        "#,
        &[(d_addr, &d_module)],
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let a_module = Compiler::compile(
        r#"
            mod a;

            use b@0xFB;
            use c@0xFC;

            pub fn run() -> u64 { b::run() }
        "#,
        &[
            (b_addr, &b_module),
            (c_addr, &c_module),
            (d_addr, &d_module),
        ],
        &[],
        cfg,
    )
    .expect("a must compile");
    (
        b_addr, c_addr, d_addr, b_module, c_module, d_module, a_module,
    )
}
