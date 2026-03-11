use meow_types::{
    address::Address,
    digest::Digest,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    transaction::{
        Transaction,
        call::{Call, Input},
        execution_result::ExecutionStatus,
    },
};
use meow_vm::{compiler::Compiler, types::Value};
use meow_vm_adapter::executor::execute;

const MEOW_COIN_SRC: &str = include_str!("../../meow-framework/modules/meow_coin.meow");

/// Fixed module address used in all tests.
const MODULE_ADDR: [u8; 32] = [0x01u8; 32];
/// Fixed sender address used in all tests.
const SENDER: [u8; 32] = [0xAAu8; 32];
/// Fixed gas coin address.
const GAS_ADDR: [u8; 32] = [0xBBu8; 32];

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_module_object() -> Object {
    let module = Compiler::compile("meow_coin", MEOW_COIN_SRC).expect("must compile");
    let content = bcs::to_bytes(&module).expect("module must serialize");
    Object::new(
        Address::new(MODULE_ADDR),
        Address::ZERO,
        Digest::ZERO,
        ObjectVersion::ZERO,
        ObjectType::Module,
        content,
    )
}

fn make_coin_object(id: [u8; 32], owner: [u8; 32], balance: u64) -> Object {
    let fields: Vec<(String, Value)> = vec![
        ("id".to_string(), Value::Address(id)),
        ("balance".to_string(), Value::U64(balance)),
    ];
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let ident = meow_types::object::identifier::Identifier::new("MeowCoin").unwrap();
    let decl_ref = ObjectDeclRef::new(Address::new(MODULE_ADDR), ident);
    Object::new(
        Address::new(id),
        Address::new(owner),
        Digest::ZERO,
        ObjectVersion::ZERO,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn make_transaction(fn_name: &str, arguments: Vec<Input>) -> Transaction {
    let function = meow_types::object::identifier::Identifier::new(fn_name).unwrap();
    let call = Call::new(Address::new(MODULE_ADDR), function, arguments);
    Transaction::new(Address::new(SENDER), Address::new(GAS_ADDR), call)
}

fn coin_balance_from_content(content: &[u8]) -> u64 {
    let fields: Vec<(String, Value)> = bcs::from_bytes(content).unwrap();
    fields
        .iter()
        .find(|(n, _)| n == "balance")
        .map(|(_, v)| v.as_u64().unwrap())
        .unwrap_or(0)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn mint_succeeds_and_creates_object() {
    let module_obj = make_module_object();
    let tx = make_transaction(
        "mint",
        vec![
            Input::Raw(bcs::to_bytes(&100u64).unwrap()), // balance
            Input::Raw(bcs::to_bytes(&SENDER).unwrap()), // owner
        ],
    );

    let result = execute(&tx, vec![module_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.created_objects().len(),
        1,
        "mint must create one coin"
    );
    assert_eq!(result.changed_objects().len(), 0);
    assert_eq!(result.destroyed_objects().len(), 0);
    // Coin must have balance 100 and owner = SENDER.
    let created = &result.created_objects()[0];
    assert_eq!(coin_balance_from_content(created.content()), 100);
    let owner: [u8; 32] = created.owner().as_ref().try_into().unwrap();
    assert_eq!(owner, SENDER);
}

#[test]
fn burn_succeeds_and_destroys_object() {
    let coin_id: [u8; 32] = [0xCCu8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 0);

    let tx = make_transaction("burn", vec![Input::Object(Address::new(coin_id))]);

    let result = execute(&tx, vec![module_obj, coin_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    assert_eq!(
        result.destroyed_objects().len(),
        1,
        "burn must destroy one coin"
    );
    assert_eq!(result.created_objects().len(), 0);
    assert_eq!(result.changed_objects().len(), 0);
}

#[test]
fn transfer_changes_owner() {
    let coin_id: [u8; 32] = [0xDDu8; 32];
    let new_owner: [u8; 32] = [0x02u8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 75);

    let tx = make_transaction(
        "transfer",
        vec![
            Input::Object(Address::new(coin_id)),
            Input::Raw(bcs::to_bytes(&new_owner).unwrap()),
        ],
    );

    let result = execute(&tx, vec![module_obj, coin_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    // A transferred input object shows up as changed (with new owner).
    assert_eq!(result.changed_objects().len(), 1);
    assert_eq!(result.created_objects().len(), 0);
    assert_eq!(result.destroyed_objects().len(), 0);
    let changed = &result.changed_objects()[0];
    let owner: [u8; 32] = changed.owner().as_ref().try_into().unwrap();
    assert_eq!(owner, new_owner);
    assert_eq!(coin_balance_from_content(changed.content()), 75);
}

#[test]
fn split_with_sufficient_balance() {
    let coin_id: [u8; 32] = [0xEEu8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 100);

    let tx = make_transaction(
        "split",
        vec![
            Input::Object(Address::new(coin_id)),
            Input::Raw(bcs::to_bytes(&40u64).unwrap()), // amount
        ],
    );

    let result = execute(&tx, vec![module_obj, coin_obj]);

    assert_eq!(result.status(), &ExecutionStatus::Success);
    // A new coin (amount=40) is created and sent to sender.
    assert_eq!(
        result.created_objects().len(),
        1,
        "one new coin must be created"
    );
    let new_coin = &result.created_objects()[0];
    assert_eq!(coin_balance_from_content(new_coin.content()), 40);
    let new_owner: [u8; 32] = new_coin.owner().as_ref().try_into().unwrap();
    assert_eq!(new_owner, SENDER);

    // The original coin is mutated (balance reduced to 60).
    let changed: Vec<_> = result
        .changed_objects()
        .iter()
        .filter(|o| o.address() == &Address::new(coin_id))
        .collect();
    assert_eq!(changed.len(), 1, "original coin must appear as changed");
    assert_eq!(coin_balance_from_content(changed[0].content()), 60);
}

#[test]
fn burn_non_zero_balance_returns_failure() {
    let coin_id: [u8; 32] = [0xCDu8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 50);

    let tx = make_transaction("burn", vec![Input::Object(Address::new(coin_id))]);

    let result = execute(&tx, vec![module_obj, coin_obj]);

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(_)),
        "burn with non-zero balance must fail"
    );
    assert_eq!(
        result.status(),
        &ExecutionStatus::Failure("Cannot burn a coin with non-zero balance".to_string())
    );
    // No object effects on failure.
    assert!(result.created_objects().is_empty());
    assert!(result.changed_objects().is_empty());
    assert!(result.destroyed_objects().is_empty());
}

#[test]
fn split_with_insufficient_balance_returns_failure() {
    let coin_id: [u8; 32] = [0xFFu8; 32];
    let module_obj = make_module_object();
    let coin_obj = make_coin_object(coin_id, SENDER, 10);

    let tx = make_transaction(
        "split",
        vec![
            Input::Object(Address::new(coin_id)),
            Input::Raw(bcs::to_bytes(&20u64).unwrap()), // amount > balance
        ],
    );

    let result = execute(&tx, vec![module_obj, coin_obj]);

    assert!(
        matches!(result.status(), ExecutionStatus::Failure(_)),
        "split with insufficient balance must fail"
    );
    // No object effects on failure.
    assert!(result.created_objects().is_empty());
    assert!(result.changed_objects().is_empty());
    assert!(result.destroyed_objects().is_empty());
}
