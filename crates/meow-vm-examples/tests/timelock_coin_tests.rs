use std::collections::HashMap;

use meow_types::{
    address::Address,
    identifier::Identifier,
    system_framework::{
        meow_coin::{MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_MODULE_PATH, MeowCoin},
        meow_object::{MEOW_OBJECT_MODULE_ADDRESS, MEOW_OBJECT_MODULE_PATH, MeowObjectId},
    },
};
use meow_vm_adapter::{
    builder,
    external_context::{DEFAULT_RAND_SEED, ExternalContext},
    runner::{self, RunResult, VmError},
};
use meow_vm_types::{convert::struct_from_rust, module::Module, types::Value};

const TIMELOCK_SRC: &str = include_str!("../modules/timelock_coin.meow");

//
// ─── Tests ───
//

#[test]
fn compile_timelock_coin() {
    let _ = timelock_module();
}

#[test]
fn lock_creates_timelock_transferred_to_sender() {
    let coin = make_coin(Address::fill(0x01), 500);
    let result = run("lock", vec![coin, Value::U64(1000)]).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, Address::ZERO); // sender is zero in default context
    // to_balance destroys the input coin
    assert_eq!(result.destroyed, vec![Address::fill(0x01)]);

    let lock = &result.transfers[0].0;
    let Value::Struct { type_name, .. } = lock else {
        panic!("expected Struct, got {lock:?}");
    };
    assert_eq!(type_name, "TimelockCoin");
    // timestamp=0 + delay_ms=1000
    assert_eq!(field_u64(field(lock, "balance"), "amount"), 500);
    assert_eq!(field_u64(lock, "unlock_time"), 1000);
}

#[test]
fn claim_before_unlock_aborts() {
    // unlock_time=1000, timestamp=0 → 0 < 1000 → abort
    let lock = make_timelock(Address::fill(0x01), 500, 1000);
    let err = run("claim", vec![lock]).unwrap_err();
    assert!(
        matches!(&err, VmError::Aborted { code: 1, .. }),
        "expected abort code 1, got {err:?}"
    );
}

#[test]
fn claim_at_unlock_time_returns_coin_to_sender() {
    // unlock_time=1000, timestamp=1000 → 1000 >= 1000 → claim succeeds
    let lock = make_timelock(Address::fill(0x01), 500, 1000);
    let result = run_with_timestamp("claim", vec![lock], 1000).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, Address::ZERO); // sender is zero in default context
    assert_eq!(result.destroyed.len(), 1);
    assert_eq!(result.destroyed[0], Address::fill(0x01));

    let coin = &result.transfers[0].0;
    let Value::Struct { type_name, .. } = coin else {
        panic!("expected Struct, got {coin:?}");
    };
    assert_eq!(type_name, "MeowCoin");
    assert_eq!(field_u64(coin, "balance"), 500);
}

#[test]
fn claim_after_unlock_time_returns_coin_to_sender() {
    // unlock_time=1000, timestamp=9999 → 9999 >= 1000 → claim succeeds
    let lock = make_timelock(Address::fill(0x02), 250, 1000);
    let result = run_with_timestamp("claim", vec![lock], 9999).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, Address::ZERO);
    assert_eq!(result.destroyed, vec![Address::fill(0x02)]);

    let coin = &result.transfers[0].0;
    let Value::Struct { type_name, .. } = coin else {
        panic!("expected Struct, got {coin:?}");
    };
    assert_eq!(type_name, "MeowCoin");
    assert_eq!(field_u64(coin, "balance"), 250);
}

#[test]
fn transfer_sends_timelock_to_recipient() {
    let recipient = Address::fill(0x42);
    let lock = make_timelock(Address::fill(0x01), 500, 1000);
    let result = run("transfer", vec![lock, Value::Address(recipient.into())]).unwrap();

    assert_eq!(result.transfers.len(), 1);
    assert_eq!(result.transfers[0].1, recipient);
    assert!(result.destroyed.is_empty());
}

//
// ─── Utilities ───
//

fn meow_object_module() -> Module {
    builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[]).expect("meow_object must compile")
}

fn meow_coin_module() -> Module {
    let dep = meow_object_module();
    builder::build_from_file(MEOW_COIN_MODULE_PATH, &[(MEOW_OBJECT_MODULE_ADDRESS, &dep)])
        .expect("meow_coin must compile")
}

fn timelock_module() -> (Module, HashMap<Address, Module>) {
    let obj = meow_object_module();
    let coin = meow_coin_module();
    let deps = [
        (MEOW_OBJECT_MODULE_ADDRESS, &obj),
        (MEOW_COIN_MODULE_ADDRESS, &coin),
    ];
    let module = builder::build(TIMELOCK_SRC, &deps).expect("timelock_coin.meow must compile");
    let deps_map = HashMap::from([
        (MEOW_OBJECT_MODULE_ADDRESS, obj),
        (MEOW_COIN_MODULE_ADDRESS, coin),
    ]);
    (module, deps_map)
}

fn make_coin(id: Address, balance: u64) -> Value {
    struct_from_rust(&MeowCoin::new(id, balance)).expect("MeowCoin must convert to Value")
}

fn make_timelock(id: Address, amount: u64, unlock_time: u64) -> Value {
    Value::Struct {
        type_name: "TimelockCoin".to_string(),
        fields: vec![
            (
                "id".to_string(),
                MeowObjectId::from(id).to_qualified_vm_value(),
            ),
            (
                "balance".to_string(),
                Value::Struct {
                    type_name: "MeowCoinBalance".to_string(),
                    fields: vec![("amount".to_string(), Value::U64(amount))],
                },
            ),
            ("unlock_time".to_string(), Value::U64(unlock_time)),
        ],
    }
}

fn field<'a>(v: &'a Value, name: &str) -> &'a Value {
    match v {
        Value::Struct { fields, .. } => fields
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("field '{name}' not found")),
        _ => panic!("expected struct"),
    }
}

fn field_u64(v: &Value, name: &str) -> u64 {
    field(v, name)
        .as_u64()
        .unwrap_or_else(|| panic!("field '{name}' must be u64"))
}

fn run(fn_name: &str, args: Vec<Value>) -> runner::Result<RunResult> {
    let (module, deps) = timelock_module();
    let id = Identifier::new(fn_name).expect("must be valid identifier");

    runner::run(module, &id, args, deps, ExternalContext::default())
}

fn run_with_timestamp(
    fn_name: &str,
    args: Vec<Value>,
    timestamp: u64,
) -> runner::Result<RunResult> {
    let (module, deps) = timelock_module();
    let id = Identifier::new(fn_name).expect("must be valid identifier");
    let external_context = ExternalContext::new(DEFAULT_RAND_SEED, timestamp);

    runner::run(module, &id, args, deps, external_context)
}
