use meow_types::system_framework::meow_coin::MeowCoin;
use meow_vm_adapter::runner::{
    self, Compiler, Module, Value, VmError, object_from_rust, value_to_rust,
};

//
// Tests.
//

#[test]
fn compile_meow_coin() {
    Compiler::compile("meow_coin", MEOW_COIN_SRC).expect("compilation must succeed");
}

#[test]
fn mint_creates_coin_transferred_to_owner() {
    let owner = [0x01u8; 32];
    let result = runner::run(
        meow_coin_module(),
        "mint",
        vec![Value::U64(100), Value::Address(owner.into())],
    )
    .expect("mint must succeed");

    assert_eq!(result.transfers.len(), 1, "one coin must be transferred");
    assert_eq!(result.transfers[0].1, owner, "coin must go to owner");
    assert_eq!(
        coin_balance(&result.transfers[0].0),
        100,
        "coin balance must be 100"
    );
    assert!(result.destroyed.is_empty());
}

#[test]
fn burn_destroys_coin() {
    let id = [0xAAu8; 32];

    for balance in [0u64, 50, 1000] {
        let result = runner::run(meow_coin_module(), "burn", vec![make_coin(id, balance)])
            .unwrap_or_else(|_| panic!("burn must succeed for balance={balance}"));

        assert_eq!(result.destroyed.len(), 1);
        assert!(result.transfers.is_empty());
    }
}

#[test]
fn transfer_changes_owner() {
    let id = [0xBBu8; 32];
    let new_owner = [0x02u8; 32];
    let result = runner::run(
        meow_coin_module(),
        "transfer",
        vec![make_coin(id, 75), Value::Address(new_owner.into())],
    )
    .expect("transfer must succeed");

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, new_owner);
    assert_eq!(coin_balance(&result.transfers[0].0), 75);
}

#[test]
fn split_with_sufficient_balance() {
    let id = [0xCCu8; 32];
    let result = runner::run(
        meow_coin_module(),
        "split",
        vec![make_coin(id, 100), Value::U64(40)],
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
    assert_eq!(result.transfers[0].1, [0; 32], "new coin goes to sender");
    assert_eq!(
        coin_balance(&result.transfers[0].0),
        40,
        "new coin has amount 40"
    );
}

#[test]
fn split_with_insufficient_balance() {
    let id = [0xDDu8; 32];
    let err = runner::run(
        meow_coin_module(),
        "split",
        vec![make_coin(id, 10), Value::U64(20)],
    )
    .expect_err("split must fail with insufficient balance");

    assert!(
        matches!(err, VmError::Aborted { code: 1, .. }),
        "must abort with code 1"
    );
}

#[test]
fn split_and_transfer_to_recipient() {
    let id = [0xEEu8; 32];
    let recipient = [0x03u8; 32];
    let result = runner::run(
        meow_coin_module(),
        "split_and_transfer",
        vec![
            make_coin(id, 100),
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
}

#[test]
fn mint_with_zero_balance_succeeds() {
    let owner = [0x01u8; 32];
    let result = runner::run(
        meow_coin_module(),
        "mint",
        vec![Value::U64(0), Value::Address(owner.into())],
    )
    .expect("mint with zero balance must succeed");

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(coin_balance(&result.transfers[0].0), 0);
    assert!(result.destroyed.is_empty());
}

#[test]
fn merge_combines_balances() {
    let from = make_coin([0x11u8; 32], 60);
    let to = make_coin([0x22u8; 32], 40);
    let result =
        runner::run(meow_coin_module(), "merge", vec![from, to]).expect("merge must succeed");

    // `from` is destroyed; `to` survives with the combined balance.
    assert!(result.transfers.is_empty());
    assert_eq!(result.destroyed.len(), 1, "from must be destroyed");

    assert!(result.final_args[0].is_none(), "from must be consumed");
    let final_to = result.final_args[1].as_ref().expect("to must survive");
    assert_eq!(coin_balance(final_to), 100);
}

#[test]
fn split_with_exact_balance_zeroes_from() {
    let id = [0xFFu8; 32];
    let result = runner::run(
        meow_coin_module(),
        "split",
        vec![make_coin(id, 50), Value::U64(50)],
    )
    .expect("split must succeed");

    // from survives with balance=0.
    let final_from = result.final_args[0].as_ref().expect("from must survive");
    assert_eq!(coin_balance(final_from), 0);

    // A new coin (balance=50) is transferred to the sender.
    assert_eq!(result.transfers.len(), 1);
    assert_eq!(coin_balance(&result.transfers[0].0), 50);
    assert!(result.destroyed.is_empty());
}

//
// Utilities.
//

const MEOW_COIN_SRC: &str = include_str!("../modules/meow_coin.meow");

fn meow_coin_module() -> Module {
    Compiler::compile("meow_coin", MEOW_COIN_SRC).expect("meow_coin.meow must compile")
}

fn make_coin(id: [u8; 32], balance: u64) -> Value {
    object_from_rust(&MeowCoin::new(id.into(), balance)).expect("MeowCoin must convert to Value")
}

fn coin_balance(v: &Value) -> u64 {
    value_to_rust::<MeowCoin>(v)
        .expect("must convert to MeowCoin")
        .balance()
}
