mod utils;

use std::str::FromStr;

use meow_vm::gas_meter::GasMeter;
use meow_vm_types::{
    address::Address,
    natives::{NativeFnEntry, NativeParam, NativeResult},
    types::{Type, Value},
};

//
// ─── address ───
//

#[test]
fn address_literal_in_source() {
    let addr = Address::from_str("0x42").unwrap();
    assert_eq!(
        utils::run(
            r#"
                mod test;

                pub fn get_addr() -> address { @0x42 }
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

                pub fn same() -> bool { let a = @0x42; let b = @0x42; a == b }
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

                pub fn different() -> bool { let a = @0x42; let b = @0x43; a == b }
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
    let addr = Address::suffixed(0x42);
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
    let addr = Address::suffixed(0xAA);
    let other = Address::suffixed(0xBB);
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
// ─── bool ───
//

#[test]
fn bool_literal_true() {
    let src = r#"
        mod test;

        pub fn yes() -> bool { true }
    "#;
    assert_eq!(utils::run(src, "yes", vec![]), Some(Value::Bool(true)));
}

#[test]
fn bool_literal_false() {
    let src = r#"
        mod test;

        pub fn no() -> bool { false }
    "#;
    assert_eq!(utils::run(src, "no", vec![]), Some(Value::Bool(false)));
}

#[test]
fn bool_round_trip() {
    let src = r#"
        mod test;

        pub fn identity(b: bool) -> bool { b }
    "#;
    assert_eq!(
        utils::run(src, "identity", vec![Value::Bool(true)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(src, "identity", vec![Value::Bool(false)]),
        Some(Value::Bool(false))
    );
}

//
// ─── string ───
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
    let src = r#"
        mod test;

        pub fn send_msg() { log_native("hello from meow"); }
    "#;
    let received_ptr = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let ptr = received_ptr.clone();
    let log = NativeFnEntry {
        name: "log_native".to_string(),
        params: vec![NativeParam::Concrete(Type::Str)],
        return_type: None,
        gas_cost: 0,
        func: Box::new(move |args| {
            *ptr.lock().unwrap() = args[0].as_str().unwrap_or("").to_string();
            NativeResult::Return(None)
        }),
    };
    let vm = utils::vm_with_natives(src, vec![log]);
    let mut gas = GasMeter::unlimited();
    vm.call("send_msg", vec![], &mut gas).unwrap();
    assert_eq!(*received_ptr.lock().unwrap(), "hello from meow");
}

//
// ─── u64 ───
//

#[test]
fn u64_literal_return() {
    let src = r#"
        mod test;

        pub fn answer() -> u64 { 42 }
    "#;
    assert_eq!(utils::run(src, "answer", vec![]), Some(Value::U64(42)));
}

#[test]
fn u64_round_trip() {
    let src = r#"
        mod test;

        pub fn identity(n: u64) -> u64 { n }
    "#;
    assert_eq!(
        utils::run(src, "identity", vec![Value::U64(12345)]),
        Some(Value::U64(12345))
    );
}

#[test]
fn u64_max_value_round_trip() {
    let src = r#"
        mod test;

        pub fn identity(n: u64) -> u64 { n }
    "#;
    assert_eq!(
        utils::run(src, "identity", vec![Value::U64(u64::MAX)]),
        Some(Value::U64(u64::MAX))
    );
}
