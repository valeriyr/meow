mod utils;

use meow_vm::{
    types::Value,
    vm::{GasMeter, GasSchedule, Vm},
};
use utils::{compile, fresh_id_native, run, vm_with_natives};

//
// ─── Address ───
//

#[test]
fn address_round_trip() {
    let addr: [u8; 32] = [0xABu8; 32];
    assert_eq!(
        run(
            "fn identity(a: address): address { return a; }",
            "identity",
            vec![Value::Address(addr)]
        ),
        Some(Value::Address(addr))
    );
}

#[test]
fn address_equality() {
    let src = "fn same(a: address, b: address): bool { return a == b; }";
    let addr: [u8; 32] = [1u8; 32];
    let other: [u8; 32] = [2u8; 32];
    assert_eq!(
        run(
            src,
            "same",
            vec![Value::Address(addr), Value::Address(addr)]
        ),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run(
            src,
            "same",
            vec![Value::Address(addr), Value::Address(other)]
        ),
        Some(Value::Bool(false))
    );
}

//
// ─── Struct ───
//

#[test]
fn struct_construction_and_field_access() {
    let src = r#"
        struct Point { x: u64, y: u64 }

        fn make(x: u64, y: u64): Point { return Point { x: x, y: y }; }
        fn get_x(p: Point): u64 { return p.x; }
    "#;
    let module = compile(src);
    let vm = Vm::new(module, vec![], GasSchedule::default());
    let mut gas = GasMeter::unlimited();

    let point = vm
        .call("make", vec![Value::U64(3), Value::U64(7)], &mut gas)
        .unwrap()
        .return_value
        .unwrap();
    assert_eq!(
        point,
        Value::Struct {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Value::U64(3)),
                ("y".to_string(), Value::U64(7))
            ],
        }
    );

    let r = vm.call("get_x", vec![point], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::U64(3)));
}

#[test]
fn struct_value_semantics() {
    // Structs are freely copyable — the same value can be passed multiple times.
    let src = r#"
        struct Counter { value: u64 }

        fn get_value(c: Counter): u64 { return c.value; }
    "#;
    let vm = Vm::new(compile(src), vec![], GasSchedule::default());
    let mut gas = GasMeter::unlimited();
    let c = test_counter(42);
    assert_eq!(
        vm.call("get_value", vec![c.clone()], &mut gas)
            .unwrap()
            .return_value,
        Some(Value::U64(42))
    );
    assert_eq!(
        vm.call("get_value", vec![c], &mut gas)
            .unwrap()
            .return_value,
        Some(Value::U64(42))
    );
}

#[test]
fn struct_field_mutation() {
    let src = r#"
        struct Counter { value: u64 }

        fn increment(c: Counter): Counter { c.value = c.value + 1; return c; }
    "#;
    assert_eq!(
        run(src, "increment", vec![test_counter(5)]),
        Some(Value::Struct {
            type_name: "Counter".to_string(),
            fields: vec![("value".to_string(), Value::U64(6))],
        })
    );
}

//
// ─── String ───
//

#[test]
fn string_literal_return() {
    let src = r#"fn greeting(): string { return "hello"; }"#;
    assert_eq!(
        run(src, "greeting", vec![]),
        Some(Value::Str("hello".to_string()))
    );
}

#[test]
fn string_parameter_round_trip() {
    let src = r#"fn identity(s: string): string { return s; }"#;
    assert_eq!(
        run(src, "identity", vec![Value::Str("meow".to_string())]),
        Some(Value::Str("meow".to_string()))
    );
}

#[test]
fn string_passed_to_native() {
    use meow_vm::vm::{NativeFnEntry, NativeResult};
    use utils::vm_with_natives;

    let src = r#"fn send_msg() { log_native("hello from meow"); }"#;
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

#[test]
fn string_struct_field_round_trip() {
    let src = r#"
        struct Msg { text: string }

        fn make(text: string): Msg { return Msg { text: text }; }
        fn get_text(m: Msg): string { return m.text; }
    "#;
    let module = compile(src);
    let vm = Vm::new(module, vec![], GasSchedule::default());
    let mut gas = GasMeter::unlimited();

    let msg = vm
        .call("make", vec![Value::Str("hello".to_string())], &mut gas)
        .unwrap()
        .return_value
        .unwrap();
    assert_eq!(
        msg,
        Value::Struct {
            type_name: "Msg".to_string(),
            fields: vec![("text".to_string(), Value::Str("hello".to_string()))],
        }
    );

    let r = vm.call("get_text", vec![msg], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::Str("hello".to_string())));
}

//
// ─── Object ───
//

#[test]
fn object_construction_and_field_access() {
    let src = r#"
        object Coin { id: address, balance: u64 }

        fn make_coin(balance: u64): u64 {
            let c = Coin { id: meow_vm_fresh_id(), balance: balance };
            return c.balance;
        }
    "#;
    let vm = vm_with_natives(src, vec![fresh_id_native()]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("make_coin", vec![Value::U64(100)], &mut gas)
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(100)));
}

#[test]
fn object_field_mutation_reflected_in_final_args() {
    let src = r#"
        object Coin { id: address, balance: u64 }

        fn double_balance(coin: Coin): u64 {
            coin.balance = coin.balance * 2;
            return coin.balance;
        }
    "#;
    let vm = Vm::new(compile(src), vec![], GasSchedule::default());
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("double_balance", vec![test_coin([1u8; 32], 50)], &mut gas)
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(100)));
    assert_eq!(r.final_args[0], Some(test_coin([1u8; 32], 100)));
}

//
// ─── Utility functions ───
//

fn test_counter(value: u64) -> Value {
    Value::Struct {
        type_name: "Counter".to_string(),
        fields: vec![("value".to_string(), Value::U64(value))],
    }
}

fn test_coin(id: [u8; 32], balance: u64) -> Value {
    Value::Object {
        type_name: "Coin".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("balance".to_string(), Value::U64(balance)),
        ],
    }
}
