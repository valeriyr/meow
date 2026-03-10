use std::{cell::RefCell, rc::Rc};

use meow_vm::{
    compiler::Compiler,
    error::VmError,
    types::Value,
    vm::{GasMeter, NativeFnEntry, NativeResult, Vm},
};

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
    let coin = make_coin(id, 50);

    let ctx = Rc::new(RefCell::new(TestCtx::default()));
    let vm = build_vm(ctx.clone());
    let mut gas = GasMeter::unlimited();

    vm.call("burn", vec![coin.clone()], &mut gas)
        .expect("burn must succeed");

    let ctx = ctx.borrow();
    assert_eq!(ctx.destroyed.len(), 1, "one coin must be destroyed");
    assert_eq!(coin_balance(&ctx.destroyed[0]), 50);
    assert!(ctx.transferred.is_empty());
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

    let abort = NativeFnEntry {
        name: "meow_vm_abort".to_string(),
        param_count: 2,
        gas_cost: 1,
        func: Box::new(|args| {
            let code = args[0].as_u64().unwrap_or(0);
            let message = args[1].as_str().unwrap_or("aborted").to_string();
            NativeResult::Abort { code, message }
        }),
    };

    let sender = NativeFnEntry {
        name: "meow_vm_sender".to_string(),
        param_count: 0,
        gas_cost: 1,
        func: Box::new(|_| NativeResult::Return(Some(Value::Address(SENDER)))),
    };

    Vm::new(module, vec![fresh_id, transfer, destroy, abort, sender])
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
