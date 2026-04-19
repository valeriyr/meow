mod utils;

use meow_vm::{error::VmError, gas_meter::GasMeter};
use meow_vm_types::types::Value;

//
// ─── Gas metering ───
//

#[test]
fn gas_meter_initial_state() {
    let gas = GasMeter::new(10);

    assert_eq!(gas.spent(), 0);
    assert_eq!(gas.remaining(), 10);
    assert_eq!(gas.limit(), 10);
}

#[test]
fn gas_meter_spent_and_remaining() {
    let mut gas = GasMeter::new(10);

    gas.charge(3).unwrap();

    assert_eq!(gas.spent(), 3);
    assert_eq!(gas.remaining(), 7);
    assert_eq!(gas.limit(), 10);

    gas.charge(6).unwrap();

    assert_eq!(gas.spent(), 9);
    assert_eq!(gas.remaining(), 1);
    assert_eq!(gas.limit(), 10);

    assert!(matches!(
        gas.charge(2).unwrap_err(),
        VmError::OutOfGas {
            spent: 10,
            limit: 10
        }
    ));

    assert_eq!(gas.spent(), 10);
    assert_eq!(gas.remaining(), 0);
    assert_eq!(gas.limit(), 10);
}

#[test]
fn gas_is_spent_during_execution() {
    let vm = utils::vm_with_natives(SRC, vec![]);
    let mut gas = GasMeter::new(10_000);
    vm.call("add", vec![Value::U64(1), Value::U64(2)], &mut gas)
        .unwrap();
    assert_eq!(gas.spent(), 6);
}

#[test]
fn out_of_gas_returns_error() {
    let vm = utils::vm_with_natives(SRC, vec![]);
    let mut gas = GasMeter::new(1);
    let err = vm
        .call("add", vec![Value::U64(1), Value::U64(2)], &mut gas)
        .unwrap_err();
    assert!(matches!(err, VmError::OutOfGas { limit: 1, spent: 1 }));
}

//
// ─── Utilities ───
//

const SRC: &str = r#"
        mod gas_test;
        pub fn add(a: u64, b: u64) -> u64 { a + b }
    "#;
