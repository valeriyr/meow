mod utils;

use meow_vm::{error::VmError, gas_meter::GasMeter};
use meow_vm_types::types::Value;
use utils::vm_with_natives;

const SRC: &str = "fn add(a: u64, b: u64): u64 { return a + b; }";

//
// ─── Gas metering ───
//

#[test]
fn gas_is_consumed_during_execution() {
    let vm = vm_with_natives(SRC, vec![]);
    let mut gas = GasMeter::new(10_000);
    vm.call("add", vec![Value::U64(1), Value::U64(2)], &mut gas)
        .unwrap();
    assert!(gas.consumed() > 0);
}

#[test]
fn out_of_gas_returns_error() {
    let vm = vm_with_natives(SRC, vec![]);
    let mut gas = GasMeter::new(1);
    assert!(matches!(
        vm.call("add", vec![Value::U64(1), Value::U64(2)], &mut gas)
            .unwrap_err(),
        VmError::OutOfGas { .. }
    ));
}
