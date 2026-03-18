mod utils;

use meow_vm::{error::VmError, gas_meter::GasMeter};
use meow_vm_types::types::Value;
use utils::vm_with_natives;

const SRC: &str = "fn add(a: u64, b: u64): u64 { return a + b; }";

//
// ─── Gas metering ───
//

#[test]
fn gas_meter_initial_state() {
    let gas = GasMeter::new(10);

    assert_eq!(gas.consumed(), 0);
    assert_eq!(gas.remaining(), 10);
    assert_eq!(gas.limit(), 10);
}

#[test]
fn gas_meter_consumed_and_remaining() {
    let mut gas = GasMeter::new(10);

    gas.charge(3).unwrap();

    assert_eq!(gas.consumed(), 3);
    assert_eq!(gas.remaining(), 7);
    assert_eq!(gas.limit(), 10);

    gas.charge(6).unwrap();

    assert_eq!(gas.consumed(), 9);
    assert_eq!(gas.remaining(), 1);
    assert_eq!(gas.limit(), 10);

    assert!(matches!(
        gas.charge(2).unwrap_err(),
        VmError::OutOfGas {
            consumed: 11,
            limit: 10
        }
    ));

    assert_eq!(gas.consumed(), 11);
    assert_eq!(gas.remaining(), 0);
    assert_eq!(gas.limit(), 10);
}

#[test]
fn gas_is_consumed_during_execution() {
    let vm = vm_with_natives(SRC, vec![]);
    let mut gas = GasMeter::new(10_000);
    vm.call("add", vec![Value::U64(1), Value::U64(2)], &mut gas)
        .unwrap();
    assert_eq!(gas.consumed(), 6);
}

#[test]
fn out_of_gas_returns_error() {
    let vm = vm_with_natives(SRC, vec![]);
    let mut gas = GasMeter::new(1);
    let err = vm
        .call("add", vec![Value::U64(1), Value::U64(2)], &mut gas)
        .unwrap_err();
    assert!(matches!(err, VmError::OutOfGas { limit: 1, consumed: 2 }));
}
