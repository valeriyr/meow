mod utils;

use std::collections::HashMap;

use meow_vm::{Vm, error::VmError, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_types::{
    config::VmConfig,
    natives::{NativeFnEntry, NativeResult},
    types::Value,
};

//
// ─── Private function call rejected ───
//

#[test]
fn calling_private_fn_returns_private_function_error() {
    let module = utils::compile(
        r#"
            mod test;
            fn secret() -> u64 { 42 }
        "#,
    );
    let vm = utils::vm(module);
    let mut gas = GasMeter::unlimited();
    let err = vm.call("secret", vec![], &mut gas).unwrap_err();
    assert!(
        matches!(err, VmError::PrivateFunction(ref name) if name == "secret"),
        "expected PrivateFunction, got: {err:?}"
    );
}

#[test]
fn calling_pub_fn_succeeds() {
    let module = utils::compile(
        r#"
            mod test;
            pub fn answer() -> u64 { 42 }
        "#,
    );
    let vm = utils::vm(module);
    let mut gas = GasMeter::unlimited();
    let result = vm.call("answer", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(42)));
}

#[test]
fn private_fn_callable_internally() {
    // Private functions must still be callable from within the same module.
    let module = utils::compile(
        r#"
            mod test;
            fn helper() -> u64 { 7 }
            pub fn run() -> u64 { helper() }
        "#,
    );
    let vm = utils::vm(module);
    let mut gas = GasMeter::unlimited();
    let result = vm.call("run", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(7)));
}

//
// ─── Native function direct call rejected ───
//

#[test]
fn calling_native_fn_directly_returns_native_function_call_direct_error() {
    // meow_vm_abort is always injected by the VM; attempting vm.call("meow_vm_abort", ...)
    // must return NativeFunctionCallDirect, not UndefinedFunction.
    let module = utils::compile(
        r#"
            mod test;
            pub fn noop() {}
        "#,
    );
    let vm = utils::vm(module);
    let mut gas = GasMeter::unlimited();
    let err = vm.call("meow_vm_abort", vec![], &mut gas).unwrap_err();
    assert!(
        matches!(err, VmError::NativeFunctionCallDirect(ref name) if name == "meow_vm_abort"),
        "expected NativeFunctionCallDirect, got: {err:?}"
    );
}

#[test]
fn calling_registered_native_fn_directly_is_rejected() {
    // A caller-registered native must also be rejected when called directly via
    // vm.call, not only the built-in meow_vm_abort.
    let module = utils::compile(
        r#"
            mod test;
            pub fn noop() {}
        "#,
    );
    let native = NativeFnEntry {
        name: "my_native".to_string(),
        params: vec![],
        return_type: None,
        gas_cost: 0,
        func: Box::new(|_| NativeResult::Return(None)),
    };
    let vm = Vm::new(
        module,
        vec![native],
        GasSchedule::default(),
        HashMap::new(),
        VmConfig::default(),
    );
    let mut gas = GasMeter::unlimited();
    let err = vm.call("my_native", vec![], &mut gas).unwrap_err();
    assert!(
        matches!(err, VmError::NativeFunctionCallDirect(ref name) if name == "my_native"),
        "expected NativeFunctionCallDirect, got: {err:?}"
    );
}

//
// ─── enable_call_private_functions config flag ───
//

#[test]
fn enable_call_private_functions_allows_private_fn() {
    // With enable_call_private_functions = true, private
    // functions must be callable via vm.call.
    let module = utils::compile(
        r#"
            mod test;
            fn secret() -> u64 { 99 }
        "#,
    );
    let vm = Vm::new(
        module,
        vec![],
        GasSchedule::default(),
        HashMap::new(),
        VmConfig::default().with_enable_call_private_functions(true),
    );
    let mut gas = GasMeter::unlimited();
    let result = vm.call("secret", vec![], &mut gas).unwrap();
    assert_eq!(result.return_value, Some(Value::U64(99)));
}

#[test]
fn enable_call_private_functions_does_not_affect_natives() {
    // Even with the flag enabled, native functions must still be rejected.
    let module = utils::compile(
        r#"
            mod test;
            fn noop() {}
        "#,
    );
    let vm = Vm::new(
        module,
        vec![],
        GasSchedule::default(),
        HashMap::new(),
        VmConfig::default().with_enable_call_private_functions(true),
    );
    let mut gas = GasMeter::unlimited();
    let err = vm.call("meow_vm_abort", vec![], &mut gas).unwrap_err();
    assert!(
        matches!(err, VmError::NativeFunctionCallDirect(ref name) if name == "meow_vm_abort"),
        "native must still be blocked even with enable_call_private_functions, got: {err:?}"
    );
}
