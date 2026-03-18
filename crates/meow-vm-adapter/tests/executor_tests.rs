use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::meow_coin::{self, MEOW_COIN_MODULE_ADDRESS},
    transaction::{
        Transaction,
        call::{Call, Input},
        execution_result::ExecutionStatus,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::{
    Value,
    executor::{self, error::ExecutorError},
};
use meow_vm_compiler::Compiler;

//
// ─── Happy path tests ───
//

#[test]
fn mint_succeeds_and_creates_object() {
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&100u64).unwrap()), // balance
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()), // owner
        ],
    );

    let result = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "mint must create one coin"
    );
    assert_eq!(result.changed_objects().len(), 1);
    assert_eq!(result.destroyed_objects().len(), 0);
    let created = &result.created_objects()[0];
    assert_eq!(meow_coin::gas_meow_coin_balance(created).unwrap(), 100);
    let owner: [u8; 32] = created
        .owner()
        .address()
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert_eq!(owner, SENDER);
    assert!(
        meow_coin::gas_meow_coin_balance(find_gas_coin(&result)).unwrap() == 998909,
        "gas must have been deducted from gas coin"
    );
}

#[test]
fn burn_succeeds_and_destroys_object() {
    let coin_id: [u8; 32] = [0xCCu8; 32];
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 50);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);
    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.destroyed_objects().len(),
        1,
        "burn must destroy one coin"
    );
    assert_eq!(result.created_objects().len(), 0);
    assert_eq!(result.changed_objects().len(), 1);
    assert_eq!(
        result.changed_objects()[0].address(),
        &Address::new(GAS_ADDR)
    );
}

#[test]
fn transfer_changes_owner() {
    let coin_id: [u8; 32] = [0xDDu8; 32];
    let new_owner: [u8; 32] = [0x02u8; 32];
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 75);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction(
        "transfer",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::Raw(bcs::to_bytes(&new_owner).unwrap()),
        ],
    );
    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(result.changed_objects().len(), 2);
    assert_eq!(result.created_objects().len(), 0);
    assert_eq!(result.destroyed_objects().len(), 0);
    let transferred = result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &Address::new(coin_id))
        .unwrap();
    let owner: [u8; 32] = transferred
        .owner()
        .address()
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert_eq!(owner, new_owner);
    assert_eq!(meow_coin::gas_meow_coin_balance(transferred).unwrap(), 75);
}

#[test]
fn split_with_sufficient_balance() {
    let coin_id: [u8; 32] = [0xEEu8; 32];
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 100);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction(
        "split",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::Raw(bcs::to_bytes(&40u64).unwrap()),
        ],
    );
    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "one new coin must be created"
    );
    let new_coin = &result.created_objects()[0];
    assert_eq!(meow_coin::gas_meow_coin_balance(new_coin).unwrap(), 40);
    let new_owner: [u8; 32] = new_coin
        .owner()
        .address()
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert_eq!(new_owner, SENDER);

    let original = result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &Address::new(coin_id))
        .expect("original coin must appear as changed");
    assert_eq!(meow_coin::gas_meow_coin_balance(original).unwrap(), 60);
    assert!(
        result
            .changed_objects()
            .iter()
            .any(|o| o.address() == &Address::new(GAS_ADDR)),
        "gas coin must appear as changed"
    );
}

//
// ─── Gas coin validation tests (resolvers.rs) ───
//

#[test]
fn execute_with_gas_coin_not_found_returns_error() {
    let module_obj = make_default_module_object();
    let missing_gas_addr: [u8; 32] = [0xFEu8; 32];
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("mint").unwrap(),
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );
    let tx = Transaction::new(
        Address::new(SENDER),
        Address::new(missing_gas_addr),
        TransactionType::MeowCall(call),
    );

    let err = executor::execute(&tx, vec![module_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::GasCoinNotFound));
}

#[test]
fn execute_with_invalid_gas_coin_returns_error() {
    let module_obj = make_default_module_object();
    let gas_obj = make_invalid_gas_coin_object();
    let tx = make_meow_call_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );

    let err = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::InvalidGasCoin));
}

#[test]
fn execute_with_invalid_gas_coin_owner_returns_error() {
    let module_obj = make_default_module_object();
    let wrong_owner: [u8; 32] = [0xFFu8; 32];
    let gas_obj = make_valid_gas_coin_object(wrong_owner);
    let tx = make_meow_call_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );

    let err = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(err, ExecutorError::InvalidGasCoinOwner));
}

#[test]
fn execute_with_gas_coin_at_max_version_returns_error() {
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object_at_version(ObjectVersion::MAX);
    let tx = make_meow_call_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );

    let err = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap_err();
    assert!(matches!(
        err,
        ExecutorError::ObjectVersionShouldBeLessThanMax
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
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );

    let result = executor::execute(&tx, vec![gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("expected exactly 1 module object in inputs, found 0")),
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
fn execute_meow_call_with_multiple_modules_returns_failure() {
    let module1 = make_default_module_object();
    // Second module object at a different address.
    let module2 = make_module_object(Address::new([0x02u8; 32]), module1.content().to_vec());
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );

    let result = executor::execute(&tx, vec![module1, module2, gas_obj]).unwrap();

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(msg) if msg.contains("expected exactly 1 module object in inputs, found 2")),
        "multiple modules must produce Failure, got: {:?}",
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

    let result = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
fn execute_with_argument_count_mismatch_returns_failure() {
    let module_obj = make_default_module_object();
    let gas_obj = make_gas_coin_object();
    // mint expects 2 args; pass only 1.
    let tx = make_meow_call_transaction("mint", vec![Input::Raw(bcs::to_bytes(&10u64).unwrap())]);

    let result = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
    let src = r#"fn do_abort() { meow_vm_abort(false, 1, "abort message"); }"#;
    let module_obj = make_module_object_from_src("abort_test", src);
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction("do_abort", vec![]);

    let result = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
    let coin_id: [u8; 32] = [0xFFu8; 32];
    let module_obj = make_default_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 10);
    let gas_obj = make_gas_coin_object();

    let tx = make_meow_call_transaction(
        "split",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::Raw(bcs::to_bytes(&20u64).unwrap()), // amount > balance
        ],
    );
    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]).unwrap();

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
    assert_eq!(
        result.changed_objects()[0].address(),
        &Address::new(GAS_ADDR)
    );
}

//
// ─── Object effects tests (effects.rs) ───
//

#[test]
fn fresh_object_not_consumed_returns_failure() {
    // A function that calls meow_vm_fresh_id() but never transfers or destroys
    // the generated object — effects.rs requires all fresh IDs to be consumed.
    let src = "fn generate_id() { let id = meow_vm_fresh_id(); }";
    let module_obj = make_module_object_from_src("leak_test", src);
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_call_transaction("generate_id", vec![]);

    let result = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap();

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
    let tx = make_meow_call_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );

    let result = executor::execute(&tx, vec![module_obj, gas_obj]).unwrap();

    assert!(
        result
            .changed_objects()
            .iter()
            .any(|o| o.address() == &Address::new(GAS_ADDR)),
        "exhausted gas coin must appear in changed_objects"
    );
    assert!(
        !result
            .destroyed_objects()
            .iter()
            .any(|o| o.address() == &Address::new(GAS_ADDR)),
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
    let module_bytes = make_module("publish_test", "fn noop() {}");
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(module_bytes);

    let result = executor::execute(&tx, vec![gas_obj]).unwrap();

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
            .any(|o| o.address() == &Address::new(GAS_ADDR)),
        "gas coin must appear in changed_objects"
    );
}

#[test]
fn execute_module_publish_charges_gas_per_byte() {
    let module_bytes = make_module("charge_test", "fn noop() {}");
    let module_size = module_bytes.len() as u64;
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(module_bytes);

    let result = executor::execute(&tx, vec![gas_obj]).unwrap();

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
    // MAX_MODULE_SIZE_BYTES = 512 * 1024 = 524_288.
    let oversized = vec![0u8; 512 * 1024 + 1];
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(oversized);

    let result = executor::execute(&tx, vec![gas_obj]).unwrap();

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
fn execute_module_publish_derives_address_from_tx_digest() {
    let module_bytes = make_module("addr_test", "fn noop() {}");
    let gas_obj = make_gas_coin_object();
    let tx = make_meow_module_publish_transaction(module_bytes);
    let tx_digest = tx.digest();

    let result = executor::execute(&tx, vec![gas_obj]).unwrap();

    let expected_addr = Address::derive(tx_digest, 0, 0);
    assert_eq!(
        result.created_objects()[0].address(),
        &expected_addr,
        "published module address must be derived from tx digest"
    );
}

//
// ─── Utility functions ───
//

const MEOW_COIN_SRC: &str = include_str!("../../meow-framework/modules/meow_coin.meow");

/// Fixed sender address used in all tests.
const SENDER: [u8; 32] = [0xAAu8; 32];
/// Fixed gas coin address.
const GAS_ADDR: [u8; 32] = [0xBBu8; 32];
/// Initial gas coin balance (more than enough for any test).
const GAS_BALANCE: u64 = 1_000_000;

fn make_default_module_object() -> Object {
    make_module_object_from_src("meow_coin", MEOW_COIN_SRC)
}

fn make_module_object_from_src(name: &str, src: &str) -> Object {
    let content = make_module(name, src);
    make_module_object(Address::ZERO, content)
}

fn make_module_object(address: Address, content: Vec<u8>) -> Object {
    Object::fresh_module(address, Digest::ZERO, content)
}

fn make_module(name: &str, src: &str) -> Vec<u8> {
    let module = Compiler::compile(name, src).expect("must compile");
    bcs::to_bytes(&module).expect("module must serialize")
}

fn make_meow_call_transaction(fn_name: &str, arguments: Vec<Input>) -> Transaction {
    let call = Call::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new(fn_name).unwrap(),
        arguments,
    );
    Transaction::new(
        Address::new(SENDER),
        Address::new(GAS_ADDR),
        TransactionType::MeowCall(call),
    )
}

fn make_meow_module_publish_transaction(module: Vec<u8>) -> Transaction {
    Transaction::new(
        Address::new(SENDER),
        Address::new(GAS_ADDR),
        TransactionType::MeowModulePublish(module),
    )
}

fn make_gas_coin_object() -> Object {
    make_valid_gas_coin_object(SENDER)
}

fn make_valid_gas_coin_object(owner: [u8; 32]) -> Object {
    make_coin_object(GAS_ADDR, owner, GAS_BALANCE)
}

fn make_coin_object(id: [u8; 32], owner: [u8; 32], balance: u64) -> Object {
    let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(balance))];
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let ident = Identifier::new("MeowCoin").unwrap();
    let decl_ref = ObjectDeclRef::new(MEOW_COIN_MODULE_ADDRESS, ident);
    Object::fresh_object(
        Address::new(id),
        Address::new(owner),
        Digest::ZERO,
        decl_ref,
        content,
    )
}

fn make_gas_coin_object_at_version(version: ObjectVersion) -> Object {
    make_gas_coin_object_at_version_and_balance(version, GAS_BALANCE)
}

fn make_gas_coin_object_at_version_and_balance(version: ObjectVersion, balance: u64) -> Object {
    let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(balance))];
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let ident = Identifier::new("MeowCoin").unwrap();
    let decl_ref = ObjectDeclRef::new(MEOW_COIN_MODULE_ADDRESS, ident);
    Object::new(
        Address::new(GAS_ADDR),
        ObjectOwner::Address(Address::new(SENDER)),
        Digest::ZERO,
        version,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn make_invalid_gas_coin_object() -> Object {
    Object::new(
        Address::new(GAS_ADDR),
        ObjectOwner::Address(Address::new(SENDER)),
        Digest::ZERO,
        ObjectVersion::ONE,
        ObjectType::Module,
        vec![],
    )
}

fn find_gas_coin<'a>(
    result: &'a meow_types::transaction::execution_result::ExecutionResult,
) -> &'a Object {
    result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &Address::new(GAS_ADDR))
        .expect("gas coin must be in changed_objects")
}
