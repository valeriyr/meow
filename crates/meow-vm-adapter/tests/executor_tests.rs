use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::meow_coin::MEOW_COIN_MODULE_ADDRESS,
    transaction::{
        Transaction,
        call::{Call, Input},
        execution_result::ExecutionStatus,
        transaction_type::TransactionType,
    },
};
use meow_vm::{compiler::Compiler, types::Value};
use meow_vm_adapter::executor;

const MEOW_COIN_SRC: &str = include_str!("../../meow-framework/modules/meow_coin.meow");

/// Fixed module address used in all tests.
const MODULE_ADDR: [u8; 32] = [0x01u8; 32];
/// Fixed sender address used in all tests.
const SENDER: [u8; 32] = [0xAAu8; 32];
/// Fixed gas coin address.
const GAS_ADDR: [u8; 32] = [0xBBu8; 32];
/// Initial gas coin balance (more than enough for any test).
const GAS_BALANCE: u64 = 1_000_000;

#[test]
fn mint_succeeds_and_creates_object() {
    let module_obj = make_module_object();
    let gas_obj = make_gas_coin_object();
    let tx = make_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&100u64).unwrap()), // balance
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()), // owner
        ],
    );

    let result = executor::execute(&tx, vec![module_obj, gas_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "mint must create one coin"
    );
    // Only the gas coin appears as changed.
    assert_eq!(result.changed_objects().len(), 1);
    assert_eq!(result.destroyed_objects().len(), 0);
    // Minted coin must have balance 100 and owner = SENDER.
    let created = &result.created_objects()[0];
    assert_eq!(coin_balance_from_content(created.content()), 100);
    let owner: [u8; 32] = created
        .owner()
        .address()
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert_eq!(owner, SENDER);
    // Gas coin must have been charged.
    let gas_changed = result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &Address::new(GAS_ADDR))
        .unwrap();
    assert!(
        coin_balance_from_content(gas_changed.content()) < GAS_BALANCE,
        "gas must have been deducted from gas coin"
    );
}

#[test]
fn burn_succeeds_and_destroys_object() {
    let coin_id: [u8; 32] = [0xCCu8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 50); // any balance is accepted
    let gas_obj = make_gas_coin_object();

    let tx = make_transaction("burn", vec![Input::Object(coin_obj.object_ref())]);

    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.destroyed_objects().len(),
        1,
        "burn must destroy one coin"
    );
    assert_eq!(result.created_objects().len(), 0);
    // Gas coin returned as changed.
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
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 75);
    let gas_obj = make_gas_coin_object();

    let tx = make_transaction(
        "transfer",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::Raw(bcs::to_bytes(&new_owner).unwrap()),
        ],
    );

    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    // Transferred coin + gas coin both appear as changed.
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
    assert_eq!(coin_balance_from_content(transferred.content()), 75);
}

#[test]
fn split_with_sufficient_balance() {
    let coin_id: [u8; 32] = [0xEEu8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 100);
    let gas_obj = make_gas_coin_object();

    let tx = make_transaction(
        "split",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::Raw(bcs::to_bytes(&40u64).unwrap()), // amount
        ],
    );

    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    // A new coin (amount=40) is created and sent to sender.
    assert_eq!(
        result.created_objects().len(),
        1,
        "one new coin must be created"
    );
    let new_coin = &result.created_objects()[0];
    assert_eq!(coin_balance_from_content(new_coin.content()), 40);
    let new_owner: [u8; 32] = new_coin
        .owner()
        .address()
        .unwrap()
        .as_ref()
        .try_into()
        .unwrap();
    assert_eq!(new_owner, SENDER);

    // The original coin (balance 60) and the gas coin both appear as changed.
    let original_coin: Vec<_> = result
        .changed_objects()
        .iter()
        .filter(|o| o.address() == &Address::new(coin_id))
        .collect();
    assert_eq!(
        original_coin.len(),
        1,
        "original coin must appear as changed"
    );
    assert_eq!(coin_balance_from_content(original_coin[0].content()), 60);

    let gas_changed = result
        .changed_objects()
        .iter()
        .find(|o| o.address() == &Address::new(GAS_ADDR));
    assert!(gas_changed.is_some(), "gas coin must appear as changed");
}

#[test]
fn split_with_insufficient_balance_returns_failure() {
    let coin_id: [u8; 32] = [0xFFu8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 10);
    let gas_obj = make_gas_coin_object();

    let tx = make_transaction(
        "split",
        vec![
            Input::Object(coin_obj.object_ref()),
            Input::Raw(bcs::to_bytes(&20u64).unwrap()), // amount > balance
        ],
    );

    let result = executor::execute(&tx, vec![module_obj, coin_obj, gas_obj]);

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(_)),
        "split with insufficient balance must fail"
    );
    // No object effects on failure except gas coin.
    assert!(result.created_objects().is_empty());
    assert!(result.destroyed_objects().is_empty());
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must be returned as changed even on failure"
    );
    assert_eq!(
        result.changed_objects()[0].address(),
        &Address::new(GAS_ADDR)
    );
}

#[test]
fn exhausted_gas_coin_goes_to_changed() {
    let module_obj = make_module_object();
    // Gas coin with balance 0: budget is 0, execution fails with OutOfGas,
    // gas coin survives with balance 0 in changed_objects.
    let gas_obj = {
        let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(0))];
        let content = bcs::to_bytes(&fields).expect("fields must serialize");
        let ident = Identifier::new("MeowCoin").unwrap();
        let decl_ref = ObjectDeclRef::new(MEOW_COIN_MODULE_ADDRESS, ident);
        Object::new(
            Address::new(GAS_ADDR),
            ObjectOwner::Address(Address::new(SENDER)),
            Digest::ZERO,
            ObjectVersion::ZERO,
            ObjectType::Object(decl_ref),
            content,
        )
    };
    let tx = make_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );

    let result = executor::execute(&tx, vec![module_obj, gas_obj]);

    let gas_in_changed = result
        .changed_objects()
        .iter()
        .any(|o| o.address() == &Address::new(GAS_ADDR));
    let gas_in_destroyed = result
        .destroyed_objects()
        .iter()
        .any(|o| o.address() == &Address::new(GAS_ADDR));

    assert!(
        gas_in_changed,
        "exhausted gas coin must appear in changed_objects"
    );
    assert!(
        !gas_in_destroyed,
        "exhausted gas coin must not appear in destroyed_objects"
    );
}

//
// ─── Error path tests ───
//

#[test]
fn execute_with_function_not_found() {
    let module_obj = make_module_object();
    let gas_obj = make_gas_coin_object();
    let tx = make_transaction("nonexistent_function", vec![]);

    let result = executor::execute(&tx, vec![module_obj, gas_obj]);

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(_)),
        "calling a missing function must produce Failure"
    );
    // Gas coin is always returned as changed even on failure.
    assert_eq!(
        result.changed_objects().len(),
        1,
        "gas coin must be returned as changed even on failure"
    );
    assert_eq!(
        result.changed_objects()[0].address(),
        &Address::new(GAS_ADDR)
    );
}

#[test]
fn execute_with_no_gas_coin() {
    // Build a transaction whose gas_coin address does not appear in the inputs.
    let module_obj = make_module_object();
    let function = Identifier::new("mint").unwrap();
    let call = Call::new(
        Address::new(MODULE_ADDR),
        function,
        vec![
            Input::Raw(bcs::to_bytes(&10u64).unwrap()),
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()),
        ],
    );
    // gas_coin address points to nothing in inputs.
    let missing_gas_addr: [u8; 32] = [0xFEu8; 32];
    let tx = Transaction::new(
        Address::new(SENDER),
        Address::new(missing_gas_addr),
        TransactionType::MeowCall(call),
    );

    let result = executor::execute(&tx, vec![module_obj]);

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(_)),
        "missing gas coin must produce Failure"
    );
}

//
// ─── Module publish tests ───
//

#[test]
fn execute_module_publish() {
    // Compile a trivial module and BCS-serialize it to bytes for the publish payload.
    let src = "fn noop() {}";
    let module = meow_vm::compiler::Compiler::compile("publish_test", src)
        .expect("module must compile");
    let module_bytes = bcs::to_bytes(&module).expect("module must serialize");

    let gas_obj = make_gas_coin_object();
    let tx = Transaction::new(
        Address::new(SENDER),
        Address::new(GAS_ADDR),
        TransactionType::MeowModulePublish(module_bytes),
    );

    let result = executor::execute(&tx, vec![gas_obj]);

    assert_eq!(
        result.status(),
        &ExecutionStatus::Success,
        "module publish must succeed"
    );
    assert_eq!(
        result.created_objects().len(),
        1,
        "publish must create exactly one module object"
    );
    assert!(
        matches!(
            result.created_objects()[0].type_(),
            meow_types::object::object_type::ObjectType::Module
        ),
        "created object must have type Module"
    );
    assert_eq!(result.destroyed_objects().len(), 0);
    // Gas coin is returned as changed.
    let gas_changed = result
        .changed_objects()
        .iter()
        .any(|o| o.address() == &Address::new(GAS_ADDR));
    assert!(gas_changed, "gas coin must appear in changed_objects");
}

//
// ─── Utility functions ───
//

fn make_module_object() -> Object {
    let module = Compiler::compile("meow_coin", MEOW_COIN_SRC).expect("must compile");
    let content = bcs::to_bytes(&module).expect("module must serialize");
    Object::new(
        Address::new(MODULE_ADDR),
        ObjectOwner::Immutable,
        Digest::ZERO,
        ObjectVersion::ZERO,
        ObjectType::Module,
        content,
    )
}

fn make_coin_object(id: [u8; 32], owner: [u8; 32], balance: u64) -> Object {
    let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(balance))];
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let ident = Identifier::new("MeowCoin").unwrap();
    let decl_ref = ObjectDeclRef::new(Address::new(MODULE_ADDR), ident);
    Object::new(
        Address::new(id),
        ObjectOwner::Address(Address::new(owner)),
        Digest::ZERO,
        ObjectVersion::ZERO,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn make_transaction(fn_name: &str, arguments: Vec<Input>) -> Transaction {
    let function = Identifier::new(fn_name).unwrap();
    let call = Call::new(Address::new(MODULE_ADDR), function, arguments);
    Transaction::new(
        Address::new(SENDER),
        Address::new(GAS_ADDR),
        TransactionType::MeowCall(call),
    )
}

fn make_gas_coin_object() -> Object {
    let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(GAS_BALANCE))];
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let ident = Identifier::new("MeowCoin").unwrap();
    let decl_ref = ObjectDeclRef::new(MEOW_COIN_MODULE_ADDRESS, ident);
    Object::new(
        Address::new(GAS_ADDR),
        ObjectOwner::Address(Address::new(SENDER)),
        Digest::ZERO,
        ObjectVersion::ZERO,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn coin_balance_from_content(content: &[u8]) -> u64 {
    let fields: Vec<(String, Value)> = bcs::from_bytes(content).unwrap();
    fields
        .iter()
        .find(|(n, _)| n == "balance")
        .map(|(_, v)| v.as_u64().unwrap())
        .unwrap_or(0)
}
