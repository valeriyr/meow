use std::collections::HashMap;

use meow_types::{
    address::Address,
    identifier::Identifier,
    system_framework::meow_coin::{
        MEOW_COIN_BALANCE_BYTECODE_TYPE_NAME, MEOW_COIN_MODULE_ADDRESS, MeowCoin, MeowCoinBalance,
        meow_coin_object,
    },
};
use meow_vm_adapter::{
    external_context::ExternalContext,
    runner::{self, RunResult, VmError},
};
use meow_vm_types::types::Value;

//
// ─── compile ───
//

#[test]
fn compile_meow_coin() {
    let _ = meow_framework::meow_coin_module();
}

//
// ─── mint ───
//

#[test]
fn mint_creates_coin_transferred_to_owner() {
    let owner = Address::suffixed(0xE1);
    let result = run_privileged("mint", vec![Value::U64(100), Value::Address(owner.into())])
        .expect("mint must succeed");

    assert_eq!(result.transfers.len(), 1, "one coin must be transferred");
    assert_eq!(result.transfers[0].1, owner, "coin must go to owner");
    assert_eq!(
        meow_coin_object::balance_from_value(&result.transfers[0].0)
            .expect("must be a MeowCoin value"),
        100,
        "coin balance must be 100"
    );
    assert!(result.destroyed.is_empty());
    assert_eq!(result.gas_spent, 91);
}

#[test]
fn mint_with_zero_balance_succeeds() {
    let result = run_privileged(
        "mint",
        vec![
            Value::U64(0),
            Value::Address(Address::suffixed(0xE1).into()),
        ],
    )
    .expect("mint with zero balance must succeed");

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(
        meow_coin_object::balance_from_value(&result.transfers[0].0)
            .expect("must be a MeowCoin value"),
        0
    );
    assert!(result.destroyed.is_empty());
    assert_eq!(result.gas_spent, 91);
}

#[test]
fn mint_without_privilege_is_rejected() {
    let err = run(
        "mint",
        vec![
            Value::U64(100),
            Value::Address(Address::suffixed(0xE1).into()),
        ],
    )
    .expect_err("mint must be rejected without privilege");

    assert!(
        matches!(err, VmError::PrivateFunction(ref name) if name == "mint"),
        "must fail with PrivateFunction(\"mint\"), got: {err:?}"
    );
}

//
// ─── balance ───
//

#[test]
fn balance_returns_coin_and_balance() {
    let result = run(
        "balance",
        vec![MeowCoin::new(Address::suffixed(0xF1), 77).into()],
    )
    .expect("balance must succeed");

    let rv = result.return_value.expect("must have return value");
    let elems = rv.as_tuple().expect("expected tuple return value");
    assert_eq!(
        meow_coin_object::balance_from_value(&elems[0]).expect("must be a MeowCoin value"),
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
    assert_eq!(result.gas_spent, 9);
}

//
// ─── burn ───
//

#[test]
fn burn_destroys_coin() {
    for balance in [0, 50, 1000] {
        let result = run(
            "burn",
            vec![MeowCoin::new(Address::suffixed(0xF1), balance).into()],
        )
        .unwrap_or_else(|_| panic!("burn must succeed for balance={balance}"));

        assert_eq!(result.destroyed.len(), 1);
        assert!(result.transfers.is_empty());
        assert_eq!(result.gas_spent, 39);
    }
}

//
// ─── transfer ───
//

#[test]
fn transfer_changes_owner() {
    let new_owner = Address::suffixed(0xE1);
    let result = run(
        "transfer",
        vec![
            MeowCoin::new(Address::suffixed(0xF1), 75).into(),
            Value::Address(new_owner.into()),
        ],
    )
    .expect("transfer must succeed");

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, new_owner);
    assert_eq!(
        meow_coin_object::balance_from_value(&result.transfers[0].0)
            .expect("must be a MeowCoin value"),
        75
    );
    assert_eq!(result.gas_spent, 45);
}

//
// ─── merge_and_transfer ───
//

#[test]
fn merge_and_transfer_to_recipient() {
    let recipient = Address::suffixed(0xE1);
    let from = MeowCoin::new(Address::suffixed(0xF1), 60).into();
    let to = MeowCoin::new(Address::suffixed(0xF2), 40).into();
    let result = run(
        "merge_and_transfer",
        vec![from, to, Value::Address(recipient.into())],
    )
    .expect("merge_and_transfer must succeed");

    assert_eq!(result.destroyed.len(), 1, "from must be destroyed");
    assert_eq!(
        result.transfers.len(),
        1,
        "to must be transferred to recipient"
    );
    assert_eq!(result.transfers[0].1, recipient, "to must go to recipient");
    assert_eq!(
        meow_coin_object::balance_from_value(&result.transfers[0].0)
            .expect("must be a MeowCoin value"),
        100,
        "merged balance must be 100"
    );
    assert_eq!(result.gas_spent, 93);
}

//
// ─── merge ───
//

#[test]
fn merge_combines_balances() {
    let from = MeowCoin::new(Address::suffixed(0xF1), 60).into();
    let to = MeowCoin::new(Address::suffixed(0xF2), 40).into();
    let result = run("merge", vec![from, to]).expect("merge must succeed");

    assert_eq!(result.destroyed.len(), 1, "from must be destroyed");
    assert_eq!(result.transfers.len(), 1, "to must be transferred back");

    let final_to = &result.transfers[0];
    assert_eq!(final_to.1, Address::ZERO, "merged coin must go to sender");
    assert_eq!(
        meow_coin_object::balance_from_value(&final_to.0).expect("must be a MeowCoin value"),
        100,
        "merged balance must be 100"
    );
    assert_eq!(result.gas_spent, 139);
}

//
// ─── split_and_transfer ───
//

#[test]
fn split_and_transfer_to_recipient() {
    let recipient = Address::suffixed(0xE1);
    let result = run(
        "split_and_transfer",
        vec![
            MeowCoin::new(Address::suffixed(0xF1), 100).into(),
            Value::U64(30),
            Value::Address(recipient.into()),
        ],
    )
    .expect("split_and_transfer must succeed");

    assert_eq!(result.transfers.len(), 2);
    assert!(result.destroyed.is_empty());

    let new_coin = &result.transfers[0];
    assert_eq!(new_coin.1, recipient, "new coin goes to recipient");
    assert_eq!(
        meow_coin_object::balance_from_value(&new_coin.0).expect("must be a MeowCoin value"),
        30
    );

    let from_coin = &result.transfers[1];
    assert_eq!(from_coin.1, Address::ZERO, "from coin returns to sender");
    assert_eq!(
        meow_coin_object::balance_from_value(&from_coin.0).expect("must be a MeowCoin value"),
        70,
        "from.balance must be 70"
    );
    assert_eq!(result.gas_spent, 193);
}

#[test]
fn split_and_transfer_with_exact_balance_zeroes_from() {
    let recipient = Address::suffixed(0xE1);
    let result = run(
        "split_and_transfer",
        vec![
            MeowCoin::new(Address::suffixed(0xF1), 50).into(),
            Value::U64(50),
            Value::Address(recipient.into()),
        ],
    )
    .expect("split_and_transfer must succeed");

    assert_eq!(result.transfers.len(), 2);
    assert!(result.destroyed.is_empty());

    let new_coin = &result.transfers[0];
    assert_eq!(new_coin.1, recipient, "new coin must go to recipient");
    assert_eq!(
        meow_coin_object::balance_from_value(&new_coin.0).expect("must be a MeowCoin value"),
        50
    );

    let from_coin = &result.transfers[1];
    assert_eq!(
        from_coin.1,
        Address::ZERO,
        "from coin must return to sender"
    );
    assert_eq!(
        meow_coin_object::balance_from_value(&from_coin.0).expect("must be a MeowCoin value"),
        0,
        "from coin must have zero balance"
    );
}

#[test]
fn split_and_transfer_with_insufficient_balance() {
    let err = run(
        "split_and_transfer",
        vec![
            MeowCoin::new(Address::suffixed(0xF1), 10).into(),
            Value::U64(20),
            Value::Address(Address::suffixed(0xE1).into()),
        ],
    )
    .expect_err("split_and_transfer must fail with insufficient balance");

    assert!(
        matches!(&err, VmError::Aborted { code: 1, message, .. } if message.contains("The balance is insufficient")),
        "must abort with code 1 and insufficient-balance message, got: {err:?}"
    );
}

//
// ─── split ───
//

#[test]
fn split_with_sufficient_balance() {
    let result = run(
        "split",
        vec![
            MeowCoin::new(Address::suffixed(0xF1), 100).into(),
            Value::U64(40),
        ],
    )
    .expect("split must succeed");

    // Two transfers: new coin (40) and input coin (60) back to sender.
    assert_eq!(result.transfers.len(), 2, "two coins must be transferred");
    assert!(result.destroyed.is_empty());

    let new_coin = &result.transfers[0];
    assert_eq!(new_coin.1, Address::ZERO, "new coin must go to sender");
    assert_eq!(
        meow_coin_object::balance_from_value(&new_coin.0).expect("must be a MeowCoin value"),
        40,
        "new coin has amount 40"
    );

    let from_coin = &result.transfers[1];
    assert_eq!(from_coin.1, Address::ZERO, "from coin must go to sender");
    assert_eq!(
        meow_coin_object::balance_from_value(&from_coin.0).expect("must be a MeowCoin value"),
        60,
        "from.balance must be reduced to 60"
    );
    assert_eq!(result.gas_spent, 239);
}

#[test]
fn split_with_exact_balance_zeroes_from() {
    let result = run(
        "split",
        vec![
            MeowCoin::new(Address::suffixed(0xF1), 50).into(),
            Value::U64(50),
        ],
    )
    .expect("split must succeed");

    assert_eq!(result.transfers.len(), 2);
    assert!(result.destroyed.is_empty());

    let new_coin = &result.transfers[0];
    assert_eq!(new_coin.1, Address::ZERO, "new coin must go to sender");
    assert_eq!(
        meow_coin_object::balance_from_value(&new_coin.0).expect("must be a MeowCoin value"),
        50
    );

    let from_coin = &result.transfers[1];
    assert_eq!(from_coin.1, Address::ZERO, "from coin must go to sender");
    assert_eq!(
        meow_coin_object::balance_from_value(&from_coin.0).expect("must be a MeowCoin value"),
        0,
        "from coin must have zero balance"
    );
    assert_eq!(result.gas_spent, 239);
}

#[test]
fn split_with_insufficient_balance() {
    let err = run(
        "split",
        vec![
            MeowCoin::new(Address::suffixed(0xF1), 10).into(),
            Value::U64(20),
        ],
    )
    .expect_err("split must fail with insufficient balance");

    assert!(
        matches!(&err, VmError::Aborted { code: 1, message, .. } if message.contains("The balance is insufficient")),
        "must abort with code 1 and insufficient-balance message, got: {err:?}"
    );
}

//
// ─── to_balance ───
//

#[test]
fn to_balance_converts_coin_to_balance() {
    let result = run(
        "to_balance",
        vec![MeowCoin::new(Address::suffixed(0xF1), 250).into()],
    )
    .expect("to_balance must succeed");

    let rv = result.return_value.expect("must have return value");
    assert_eq!(rv.type_name(), MEOW_COIN_BALANCE_BYTECODE_TYPE_NAME);
    assert_eq!(rv.field("amount"), Some(&Value::U64(250)));

    assert_eq!(result.destroyed.len(), 1, "coin must be destroyed");
    assert!(result.transfers.is_empty());
    assert_eq!(result.gas_spent, 52);
}

//
// ─── from_balance ───
//

#[test]
fn from_balance_converts_balance_to_coin() {
    let balance_val = MeowCoinBalance::new(150).into();
    let result = run("from_balance", vec![balance_val]).expect("from_balance must succeed");

    let rv = result.return_value.expect("must have return value");
    assert_eq!(
        meow_coin_object::balance_from_value(&rv).expect("must be a MeowCoin value"),
        150
    );
    assert!(result.destroyed.is_empty());
    assert!(result.transfers.is_empty());
    assert_eq!(result.gas_spent, 50);
}

//
// ─── Utilities ───
//

fn run(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let fn_name = Identifier::new(fn_name).expect("function name must be a valid identifier");
    runner::run(
        (MEOW_COIN_MODULE_ADDRESS, meow_framework::meow_coin_module()),
        &fn_name,
        args,
        HashMap::new(),
        ExternalContext::default(),
    )
}

fn run_privileged(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let fn_name = Identifier::new(fn_name).expect("function name must be a valid identifier");
    runner::run_privileged(
        (MEOW_COIN_MODULE_ADDRESS, meow_framework::meow_coin_module()),
        &fn_name,
        args,
        HashMap::new(),
        ExternalContext::default(),
    )
}
