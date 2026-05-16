use std::collections::HashMap;

use meow_types::{
    address::Address,
    identifier::Identifier,
    system_framework::{
        meow_coin::MeowCoin,
        meow_object::{MEOW_OBJECT_MODULE_ADDRESS, MEOW_OBJECT_MODULE_PATH},
    },
};
use meow_vm_adapter::{
    builder,
    external_context::ExternalContext,
    runner::{self, RunResult, VmError},
};
use meow_vm_types::{
    convert::{struct_from_rust, value_to_rust},
    module::Module,
    types::Value,
};

//
// ─── Tests ───
//

#[test]
fn compile_meow_coin() {
    let _ = meow_coin_module();
}

#[test]
fn mint_creates_coin_transferred_to_owner() {
    let owner = Address::fill(0x01);
    let result = run_privileged("mint", vec![Value::U64(100), Value::Address(owner.into())])
        .expect("mint must succeed");

    assert_eq!(result.transfers.len(), 1, "one coin must be transferred");
    assert_eq!(result.transfers[0].1, owner, "coin must go to owner");
    assert_eq!(
        coin_balance(&result.transfers[0].0),
        100,
        "coin balance must be 100"
    );
    assert!(result.destroyed.is_empty());
    assert_eq!(result.gas_spent, 91);
}

#[test]
fn burn_destroys_coin() {
    for balance in [0, 50, 1000] {
        let result = run("burn", vec![make_coin(Address::fill(0xAA), balance)])
            .unwrap_or_else(|_| panic!("burn must succeed for balance={balance}"));

        assert_eq!(result.destroyed.len(), 1);
        assert!(result.transfers.is_empty());
        assert_eq!(result.gas_spent, 39);
    }
}

#[test]
fn transfer_changes_owner() {
    let new_owner = Address::fill(0x02);
    let result = run(
        "transfer",
        vec![
            make_coin(Address::fill(0xBB), 75),
            Value::Address(new_owner.into()),
        ],
    )
    .expect("transfer must succeed");

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, new_owner);
    assert_eq!(coin_balance(&result.transfers[0].0), 75);
    assert_eq!(result.gas_spent, 45);
}

#[test]
fn split_with_sufficient_balance() {
    let result = run(
        "split",
        vec![make_coin(Address::fill(0xCC), 100), Value::U64(40)],
    )
    .expect("split must succeed");

    // The input coin (from) survives with reduced balance.
    let final_coin = result.final_args[0]
        .as_ref()
        .expect("from coin must survive");
    assert_eq!(
        coin_balance(final_coin),
        60,
        "from.balance must be reduced to 60"
    );

    // A new coin with amount 40 is transferred to the sender (Address::ZERO in runner).
    assert_eq!(
        result.transfers.len(),
        1,
        "one new coin must be transferred"
    );
    assert_eq!(
        result.transfers[0].1,
        Address::ZERO,
        "new coin goes to sender"
    );
    assert_eq!(
        coin_balance(&result.transfers[0].0),
        40,
        "new coin has amount 40"
    );
    assert_eq!(result.gas_spent, 150);
}

#[test]
fn split_with_insufficient_balance() {
    let err = run(
        "split",
        vec![make_coin(Address::fill(0xDD), 10), Value::U64(20)],
    )
    .expect_err("split must fail with insufficient balance");

    assert!(
        matches!(&err, VmError::Aborted { code: 1, message, .. } if message.contains("The balance is insufficient")),
        "must abort with code 1 and insufficient-balance message, got: {err:?}"
    );
}

#[test]
fn split_and_transfer_to_recipient() {
    let recipient = Address::fill(0x03);
    let result = run(
        "split_and_transfer",
        vec![
            make_coin(Address::fill(0xEE), 100),
            Value::U64(30),
            Value::Address(recipient.into()),
        ],
    )
    .expect("split_and_transfer must succeed");

    // The input coin (from) survives with reduced balance.
    let final_coin = result.final_args[0]
        .as_ref()
        .expect("from coin must survive");
    assert_eq!(coin_balance(final_coin), 70, "from.balance must be 70");

    // A new coin with amount 30 is transferred to the recipient.
    assert_eq!(result.transfers.len(), 1);
    assert_eq!(
        result.transfers[0].1, recipient,
        "new coin goes to recipient"
    );
    assert_eq!(coin_balance(&result.transfers[0].0), 30);
    assert_eq!(result.gas_spent, 130);
}

#[test]
fn mint_with_zero_balance_succeeds() {
    let result = run_privileged(
        "mint",
        vec![Value::U64(0), Value::Address(Address::fill(0x01).into())],
    )
    .expect("mint with zero balance must succeed");

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(coin_balance(&result.transfers[0].0), 0);
    assert!(result.destroyed.is_empty());
    assert_eq!(result.gas_spent, 91);
}

#[test]
fn balance_returns_coin_and_balance() {
    let result =
        run("balance", vec![make_coin(Address::fill(0x10), 77)]).expect("balance must succeed");

    // The coin is moved into the return tuple; the input slot is consumed.
    assert!(
        result.final_args[0].is_none(),
        "coin must be consumed from input slot"
    );

    let rv = result.return_value.expect("must have return value");
    let Value::Tuple(elems) = rv else {
        panic!("expected tuple return value");
    };
    assert_eq!(
        coin_balance(&elems[0]),
        77,
        "returned coin balance must be 77"
    );
    assert_eq!(
        elems[1].as_u64(),
        Some(77),
        "returned u64 balance must be 77"
    );

    assert!(result.transfers.is_empty());
    assert!(result.destroyed.is_empty());
}

#[test]
fn to_balance_converts_coin_to_balance() {
    let result = run("to_balance", vec![make_coin(Address::fill(0xAA), 250)])
        .expect("to_balance must succeed");

    let rv = result.return_value.expect("must have return value");
    let Value::Struct {
        type_name,
        ref fields,
    } = rv
    else {
        panic!("expected Struct return value, got: {rv:?}");
    };
    assert_eq!(type_name, "MeowCoinBalance");
    assert_eq!(
        fields.iter().find(|(k, _)| k == "amount").map(|(_, v)| v),
        Some(&Value::U64(250))
    );

    assert_eq!(result.destroyed.len(), 1, "coin must be destroyed");
    assert!(result.transfers.is_empty());
    assert_eq!(result.gas_spent, 52);
}

#[test]
fn from_balance_converts_balance_to_coin() {
    let balance_val = meow_coin_balance(150);
    let result = run("from_balance", vec![balance_val]).expect("from_balance must succeed");

    let rv = result.return_value.expect("must have return value");
    assert_eq!(coin_balance(&rv), 150);
    assert!(result.destroyed.is_empty());
    assert!(result.transfers.is_empty());
    assert_eq!(result.gas_spent, 50);
}

#[test]
fn merge_combines_balances() {
    let from = make_coin(Address::fill(0x11), 60);
    let to = make_coin(Address::fill(0x22), 40);
    let result = run("merge", vec![from, to]).expect("merge must succeed");

    // `from` is destroyed; `to` survives with the combined balance.
    assert!(result.transfers.is_empty());
    assert_eq!(result.destroyed.len(), 1, "from must be destroyed");

    assert!(result.final_args[0].is_none(), "from must be consumed");
    let final_to = result.final_args[1].as_ref().expect("to must survive");
    assert_eq!(coin_balance(final_to), 100);
    assert_eq!(result.gas_spent, 50);
}

#[test]
fn split_and_transfer_with_insufficient_balance() {
    let err = run(
        "split_and_transfer",
        vec![
            make_coin(Address::fill(0xEE), 10),
            Value::U64(20),
            Value::Address(Address::fill(0x03).into()),
        ],
    )
    .expect_err("split_and_transfer must fail with insufficient balance");

    assert!(
        matches!(&err, VmError::Aborted { code: 1, message, .. } if message.contains("The balance is insufficient")),
        "must abort with code 1 and insufficient-balance message, got: {err:?}"
    );
}

#[test]
fn split_with_exact_balance_zeroes_from() {
    let result = run(
        "split",
        vec![make_coin(Address::fill(0xFF), 50), Value::U64(50)],
    )
    .expect("split must succeed");

    // from survives with balance=0.
    let final_from = result.final_args[0].as_ref().expect("from must survive");
    assert_eq!(coin_balance(final_from), 0);

    // A new coin (balance=50) is transferred to the sender.
    assert_eq!(result.transfers.len(), 1);
    assert_eq!(coin_balance(&result.transfers[0].0), 50);
    assert!(result.destroyed.is_empty());
    assert_eq!(result.gas_spent, 150);
}

//
// ─── Utilities ───
//

const MEOW_COIN_SRC: &str = include_str!("../modules/meow_coin.meow");

fn meow_object_module() -> Module {
    builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[]).expect("meow_object.meow must compile")
}

fn meow_coin_module() -> Module {
    let meow_object = meow_object_module();
    builder::build(MEOW_COIN_SRC, &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object)])
        .expect("meow_coin.meow must compile")
}

fn make_coin(id: Address, balance: u64) -> Value {
    struct_from_rust(&MeowCoin::new(id, balance)).expect("MeowCoin must convert to Value")
}

fn meow_coin_balance(amount: u64) -> Value {
    Value::Struct {
        type_name: "MeowCoinBalance".to_string(),
        fields: vec![("amount".to_string(), Value::U64(amount))],
    }
}

fn coin_balance(v: &Value) -> u64 {
    value_to_rust::<MeowCoin>(v)
        .expect("must convert to MeowCoin")
        .balance()
}

pub fn run(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let fn_name = Identifier::new(fn_name).expect("function name must be a valid identifier");
    runner::run(
        meow_coin_module(),
        &fn_name,
        args,
        HashMap::new(),
        ExternalContext::default(),
    )
}

pub fn run_privileged(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let fn_name = Identifier::new(fn_name).expect("function name must be a valid identifier");
    runner::run_privileged(
        meow_coin_module(),
        &fn_name,
        args,
        HashMap::new(),
        ExternalContext::default(),
    )
}
