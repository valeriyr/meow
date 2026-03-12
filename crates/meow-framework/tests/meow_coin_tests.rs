use std::{cell::RefCell, rc::Rc};

use meow_types::{address::Address, system_framework::MeowCoin};
use meow_vm::{
    compiler::Compiler,
    convert,
    types::Value,
    vm::{GasMeter, GasSchedule, NativeFnEntry, NativeResult, Vm, error::VmError},
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
    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    let owner: [u8; 32] = [0x01u8; 32];
    vm.call(
        "mint",
        vec![Value::U64(100), Value::Address(owner)],
        &mut gas,
    )
    .expect("mint must succeed");

    let ctx = ctx.borrow();
    assert_eq!(ctx.transferred.len(), 1, "one coin must be transferred");
    assert_eq!(ctx.transferred[0].1, owner, "coin must go to owner");
    assert_eq!(
        coin_balance(&ctx.transferred[0].0),
        100,
        "coin balance must be 100"
    );
    assert!(ctx.destroyed.is_empty());
}

#[test]
fn burn_destroys_coin() {
    let id: [u8; 32] = [0xAAu8; 32];

    for balance in [0u64, 50, 1000] {
        let coin = make_coin(id, balance);
        let ctx = Rc::new(RefCell::new(TestCtx::default()));
        let vm = build_vm(ctx.clone());
        let mut gas = GasMeter::unlimited();

        vm.call("burn", vec![coin], &mut gas)
            .unwrap_or_else(|_| panic!("burn must succeed for balance={balance}"));

        let ctx = ctx.borrow();
        assert_eq!(ctx.destroyed.len(), 1);
        assert!(ctx.transferred.is_empty());
    }
}

#[test]
fn transfer_changes_owner() {
    let id: [u8; 32] = [0xBBu8; 32];
    let coin = make_coin(id, 75);
    let new_owner: [u8; 32] = [0x02u8; 32];

    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    vm.call("transfer", vec![coin, Value::Address(new_owner)], &mut gas)
        .expect("transfer must succeed");

    let ctx = ctx.borrow();
    assert_eq!(ctx.transferred.len(), 1);
    assert_eq!(ctx.transferred[0].1, new_owner);
    assert_eq!(coin_balance(&ctx.transferred[0].0), 75);
}

#[test]
fn split_with_sufficient_balance() {
    let id: [u8; 32] = [0xCCu8; 32];
    let coin = make_coin(id, 100);

    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    let result = vm
        .call("split", vec![coin, Value::U64(40)], &mut gas)
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

    // A new coin with amount 40 is transferred to the sender.
    let ctx = ctx.borrow();
    assert_eq!(ctx.transferred.len(), 1, "one new coin must be transferred");
    assert_eq!(ctx.transferred[0].1, SENDER, "new coin goes to sender");
    assert_eq!(
        coin_balance(&ctx.transferred[0].0),
        40,
        "new coin has amount 40"
    );
}

#[test]
fn split_with_insufficient_balance() {
    let id: [u8; 32] = [0xDDu8; 32];
    let coin = make_coin(id, 10);

    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    let err = vm
        .call("split", vec![coin, Value::U64(20)], &mut gas)
        .expect_err("split must fail with insufficient balance");

    assert!(
        matches!(err, VmError::Aborted { code: 1, .. }),
        "must abort with code 1"
    );
}

#[test]
fn split_and_transfer_to_recipient() {
    let id: [u8; 32] = [0xEEu8; 32];
    let coin = make_coin(id, 100);
    let recipient: [u8; 32] = [0x03u8; 32];

    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    let result = vm
        .call(
            "split_and_transfer",
            vec![coin, Value::U64(30), Value::Address(recipient)],
            &mut gas,
        )
        .expect("split_and_transfer must succeed");

    // The input coin (from) survives with reduced balance.
    let final_coin = result.final_args[0]
        .as_ref()
        .expect("from coin must survive");
    assert_eq!(coin_balance(final_coin), 70, "from.balance must be 70");

    // A new coin with amount 30 is transferred to the recipient.
    let ctx = ctx.borrow();
    assert_eq!(ctx.transferred.len(), 1);
    assert_eq!(
        ctx.transferred[0].1, recipient,
        "new coin goes to recipient"
    );
    assert_eq!(coin_balance(&ctx.transferred[0].0), 30);
}

#[test]
fn mint_with_zero_balance_succeeds() {
    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    let owner: [u8; 32] = [0x01u8; 32];
    vm.call("mint", vec![Value::U64(0), Value::Address(owner)], &mut gas)
        .expect("mint with zero balance must succeed");

    let ctx = ctx.borrow();
    assert_eq!(ctx.transferred.len(), 1);
    assert_eq!(coin_balance(&ctx.transferred[0].0), 0);
    assert!(ctx.destroyed.is_empty());
}

#[test]
fn merge_combines_balances() {
    let from = make_coin([0x11u8; 32], 60);
    let to = make_coin([0x22u8; 32], 40);

    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    let result = vm
        .call("merge", vec![from, to], &mut gas)
        .expect("merge must succeed");

    // `from` is destroyed; `to` survives with the combined balance.
    let ctx = ctx.borrow();
    assert!(ctx.transferred.is_empty());
    assert_eq!(ctx.destroyed.len(), 1, "from must be destroyed");

    assert!(result.final_args[0].is_none(), "from must be consumed");
    let final_to = result.final_args[1].as_ref().expect("to must survive");
    assert_eq!(coin_balance(final_to), 100);
}

#[test]
fn split_with_exact_balance_zeroes_from() {
    let id: [u8; 32] = [0xFFu8; 32];
    let coin = make_coin(id, 50);

    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    let result = vm
        .call("split", vec![coin, Value::U64(50)], &mut gas)
        .expect("split must succeed");

    // from survives with balance=0.
    let final_from = result.final_args[0].as_ref().expect("from must survive");
    assert_eq!(coin_balance(final_from), 0);

    // A new coin (balance=50) is transferred to the sender.
    let ctx = ctx.borrow();
    assert_eq!(ctx.transferred.len(), 1);
    assert_eq!(coin_balance(&ctx.transferred[0].0), 50);
    assert!(ctx.destroyed.is_empty());
}

//
// Conversion tests.
//

#[test]
fn round_trip_meow_coin() {
    let id: [u8; 32] = [0xFFu8; 32];
    let balance = 50;

    let coin = make_coin(id, balance);

    let rust_coin = convert::value_to_rust::<MeowCoin>(&coin).expect("must convert to Rust");

    assert_eq!(rust_coin.id(), &Address::from(id));
    assert_eq!(rust_coin.balance(), balance);

    assert_eq!(
        coin,
        convert::object_from_rust(&rust_coin).expect("must convert back to Value")
    );
}

//
// Utilities.
//

const MEOW_COIN_SRC: &str = include_str!("../modules/meow_coin.meow");

const SENDER: [u8; 32] = [0x42u8; 32];

#[derive(Default)]
struct TestCtx {
    id_counter: u64,
    transferred: Vec<(Value, [u8; 32])>,
    destroyed: Vec<Value>,
}

impl TestCtx {
    fn next_id(&mut self) -> [u8; 32] {
        let mut id = [0u8; 32];
        id[0..8].copy_from_slice(&self.id_counter.to_le_bytes());
        self.id_counter += 1;
        id
    }
}

/// Build a Vm with mock blockchain natives. `meow_vm_abort` is intentionally
/// omitted — the VM provides a built-in default implementation.
fn build_vm(ctx: Rc<RefCell<TestCtx>>) -> Vm {
    let module =
        Compiler::compile("meow_coin", MEOW_COIN_SRC).expect("meow_coin.meow must compile");

    let fresh_id = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_fresh_id".to_string(),
            param_count: 0,
            gas_cost: 10,
            func: Box::new(move |_| {
                let id = c.borrow_mut().next_id();
                NativeResult::Return(Some(Value::Address(id)))
            }),
        }
    };

    let transfer = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_transfer".to_string(),
            param_count: 2,
            gas_cost: 20,
            func: Box::new(move |mut args| {
                let owner = match args.pop().unwrap() {
                    Value::Address(a) => a,
                    other => {
                        return NativeResult::Error(format!(
                            "meow_vm_transfer: expected address owner, got {}",
                            other.type_name()
                        ));
                    }
                };
                let obj = args.pop().unwrap();
                c.borrow_mut().transferred.push((obj, owner));
                NativeResult::Return(None)
            }),
        }
    };

    let destroy = {
        let c = ctx.clone();
        NativeFnEntry {
            name: "meow_vm_destroy".to_string(),
            param_count: 1,
            gas_cost: 10,
            func: Box::new(move |args| {
                c.borrow_mut().destroyed.push(args[0].clone());
                NativeResult::Return(None)
            }),
        }
    };

    let sender = NativeFnEntry {
        name: "meow_vm_sender".to_string(),
        param_count: 0,
        gas_cost: 1,
        func: Box::new(|_| NativeResult::Return(Some(Value::Address(SENDER)))),
    };

    Vm::new(
        module,
        vec![fresh_id, transfer, destroy, sender],
        GasSchedule::default(),
    )
}

fn make_coin(id: [u8; 32], balance: u64) -> Value {
    Value::Object {
        type_name: "MeowCoin".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("balance".to_string(), Value::U64(balance)),
        ],
    }
}

fn coin_balance(v: &Value) -> u64 {
    match v {
        Value::Object { fields, .. } => fields
            .iter()
            .find(|(n, _)| n == "balance")
            .map(|(_, v)| v.as_u64().unwrap())
            .unwrap_or(0),
        _ => panic!("not an object"),
    }
}
