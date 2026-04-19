mod utils;

use std::str::FromStr;

use meow_vm::gas_meter::GasMeter;
use meow_vm_types::{address::Address, types::Value};

//
// ─── Address ───
//

#[test]
fn address_literal_in_source() {
    let addr = Address::from_str("0x01").unwrap();
    assert_eq!(
        utils::run(
            r#"
                mod test;
                pub fn get_addr() -> address { @0x01 }
            "#,
            "get_addr",
            vec![],
        ),
        Some(Value::Address(addr))
    );
}

#[test]
fn address_literal_equality() {
    assert_eq!(
        utils::run(
            r#"
                mod test;
                pub fn same() -> bool { let a = @0x01; let b = @0x01; a == b }
            "#,
            "same",
            vec![],
        ),
        Some(Value::Bool(true))
    );
}

#[test]
fn address_literal_inequality() {
    assert_eq!(
        utils::run(
            r#"
                mod test;
                pub fn different() -> bool { let a = @0x01; let b = @0x02; a == b }
            "#,
            "different",
            vec![],
        ),
        Some(Value::Bool(false))
    );
}

#[test]
fn address_literal_passed_as_parameter() {
    assert_eq!(
        utils::run(
            r#"
                mod test;
                pub fn is_zero(a: address) -> bool { a == @0x00 }
                pub fn check() -> bool { is_zero(@0x00) }
            "#,
            "check",
            vec![],
        ),
        Some(Value::Bool(true))
    );
}

#[test]
fn address_round_trip() {
    let addr = Address::fill(0xAB);
    assert_eq!(
        utils::run(
            r#"
                mod test;
                pub fn identity(a: address) -> address { a }
            "#,
            "identity",
            vec![Value::Address(addr)]
        ),
        Some(Value::Address(addr))
    );
}

#[test]
fn address_equality() {
    let src = r#"
        mod test;
        pub fn same(a: address, b: address) -> bool { a == b }
    "#;
    let addr = Address::fill(1);
    let other = Address::fill(2);
    assert_eq!(
        utils::run(
            src,
            "same",
            vec![Value::Address(addr), Value::Address(addr)]
        ),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(
            src,
            "same",
            vec![Value::Address(addr), Value::Address(other)]
        ),
        Some(Value::Bool(false))
    );
}

//
// ─── String ───
//

#[test]
fn string_literal_return() {
    let src = r#"
        mod test;
        pub fn greeting() -> string { "hello" }
    "#;
    assert_eq!(
        utils::run(src, "greeting", vec![]),
        Some(Value::Str("hello".to_string()))
    );
}

#[test]
fn string_parameter_round_trip() {
    let src = r#"
        mod test;
        pub fn identity(s: string) -> string { s }
    "#;
    assert_eq!(
        utils::run(src, "identity", vec![Value::Str("meow".to_string())]),
        Some(Value::Str("meow".to_string()))
    );
}

#[test]
fn string_passed_to_native() {
    use meow_vm::{NativeFnEntry, NativeResult};
    use utils::vm_with_natives;

    let src = r#"
        mod test;
        pub fn send_msg() { log_native("hello from meow"); }
    "#;
    let received_ptr = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let ptr = received_ptr.clone();
    let log = NativeFnEntry {
        name: "log_native".to_string(),
        param_count: 1,
        gas_cost: 0,
        func: Box::new(move |args| {
            *ptr.lock().unwrap() = args[0].as_str().unwrap_or("").to_string();
            NativeResult::Return(None)
        }),
    };
    let vm = vm_with_natives(src, vec![log]);
    let mut gas = GasMeter::unlimited();
    vm.call("send_msg", vec![], &mut gas).unwrap();
    assert_eq!(*received_ptr.lock().unwrap(), "hello from meow");
}
