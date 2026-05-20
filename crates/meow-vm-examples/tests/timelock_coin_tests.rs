use std::collections::HashMap;

use meow_framework::{meow_coin_module, meow_object_module};
use meow_types::{
    address::Address,
    identifier::Identifier,
    system_framework::{
        meow_coin::{
            MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME, MeowCoin,
            MeowCoinBalance,
        },
        meow_object::{MEOW_OBJECT_ID_FIELD_NAME, MEOW_OBJECT_MODULE_ADDRESS, MeowObjectId},
    },
};
use meow_vm_adapter::{
    builder,
    external_context::{DEFAULT_RAND_SEED, ExternalContext},
    runner::{self, RunResult, VmError},
};
use meow_vm_types::{module::Module, module_ref, types::Value};

//
// ─── compile ───
//

#[test]
fn compile_timelock_coin() {
    let _ = timelock_module();
}

//
// ─── lock ───
//

#[test]
fn lock_creates_timelock_transferred_to_sender() {
    let coin = MeowCoin::new(Address::fill(0xF1), 500).into();
    let result = run("lock", vec![coin, Value::U64(1000)]).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, Address::ZERO); // sender is zero in default context
    // to_balance destroys the input coin
    assert_eq!(result.destroyed, vec![Address::fill(0xF1)]);

    let lock = &result.transfers[0].0;
    assert_eq!(lock.type_name(), timelock_module_name_qualified());
    // timestamp=0 + delay_ms=1000
    assert_eq!(
        lock.field("balance").unwrap().field_u64("amount").unwrap(),
        500
    );
    assert_eq!(lock.field_u64("unlock_time").unwrap(), 1000);
}

//
// ─── claim ───
//

#[test]
fn claim_at_unlock_time_returns_coin_to_sender() {
    // unlock_time=1000, timestamp=1000 → 1000 >= 1000 → claim succeeds
    let lock = make_timelock(Address::fill(0xF1), 500, 1000);
    let result = run_with_timestamp("claim", vec![lock], 1000).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, Address::ZERO); // sender is zero in default context
    assert_eq!(result.destroyed.len(), 1);
    assert_eq!(result.destroyed[0], Address::fill(0xF1));

    let coin = &result.transfers[0].0;
    assert_eq!(coin.type_name(), MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME);
    assert_eq!(coin.field_u64("balance").unwrap(), 500);
}

#[test]
fn claim_after_unlock_time_returns_coin_to_sender() {
    // unlock_time=1000, timestamp=9999 → 9999 >= 1000 → claim succeeds
    let lock = make_timelock(Address::fill(0xF2), 250, 1000);
    let result = run_with_timestamp("claim", vec![lock], 9999).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, Address::ZERO);
    assert_eq!(result.destroyed, vec![Address::fill(0xF2)]);

    let coin = &result.transfers[0].0;
    assert_eq!(coin.type_name(), MEOW_COIN_OBJECT_BYTECODE_TYPE_NAME);
    assert_eq!(coin.field_u64("balance").unwrap(), 250);
}

#[test]
fn claim_before_unlock_aborts() {
    // unlock_time=1000, timestamp=0 → 0 < 1000 → abort
    let lock = make_timelock(Address::fill(0xF1), 500, 1000);
    let err = run("claim", vec![lock]).unwrap_err();
    assert!(
        matches!(&err, VmError::Aborted { code: 1, message, .. } if message.contains("Coin is still locked")),
        "expected abort code 1 with locked message, got {err:?}"
    );
}

//
// ─── transfer ───
//

#[test]
fn transfer_sends_timelock_to_recipient() {
    let recipient = Address::fill(0xE1);
    let lock = make_timelock(Address::fill(0xF1), 500, 1000);
    let result = run("transfer", vec![lock, Value::Address(recipient.into())]).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, recipient);
    assert!(result.destroyed.is_empty());
}

//
// ─── Utilities ───
//

const TIMELOCK_COIN_SRC: &str = include_str!("../modules/timelock_coin.meow");

fn timelock_module_name_qualified() -> String {
    module_ref::qualify(&Address::ZERO.into(), "TimelockCoin")
}

fn timelock_module() -> (Module, HashMap<Address, Module>) {
    let obj = meow_object_module();
    let coin = meow_coin_module();
    let deps = [
        (MEOW_OBJECT_MODULE_ADDRESS, &obj),
        (MEOW_COIN_MODULE_ADDRESS, &coin),
    ];
    let module = builder::build(TIMELOCK_COIN_SRC, &deps).expect("timelock_coin.meow must compile");
    let deps_map = HashMap::from([
        (MEOW_OBJECT_MODULE_ADDRESS, obj),
        (MEOW_COIN_MODULE_ADDRESS, coin),
    ]);
    (module, deps_map)
}

fn make_timelock(id: Address, amount: u64, unlock_time: u64) -> Value {
    Value::Struct {
        type_name: timelock_module_name_qualified(),
        fields: vec![
            (
                MEOW_OBJECT_ID_FIELD_NAME.to_string(),
                MeowObjectId::from(id).into(),
            ),
            ("balance".to_string(), MeowCoinBalance::new(amount).into()),
            ("unlock_time".to_string(), Value::U64(unlock_time)),
        ],
    }
}

fn run(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let (module, deps) = timelock_module();
    let id = Identifier::new(fn_name).expect("must be valid identifier");

    runner::run(
        (Address::ZERO, module),
        &id,
        args,
        deps,
        ExternalContext::default(),
    )
}

fn run_with_timestamp(
    fn_name: &str,
    args: Vec<Value>,
    timestamp: u64,
) -> runner::Result<RunResult> {
    let (module, deps) = timelock_module();
    let id = Identifier::new(fn_name).expect("must be valid identifier");
    let external_context = ExternalContext::new(DEFAULT_RAND_SEED, timestamp);

    runner::run((Address::ZERO, module), &id, args, deps, external_context)
}
