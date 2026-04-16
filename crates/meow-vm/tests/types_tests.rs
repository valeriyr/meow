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
                module test;
                pub fn get_addr(): address { return @0x01; }
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
                module test;
                pub fn same(): bool { let a = @0x01; let b = @0x01; return a == b; }
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
                module test;
                pub fn different(): bool { let a = @0x01; let b = @0x02; return a == b; }
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
                module test;
                pub fn is_zero(a: address): bool { return a == @0x00; }
                pub fn check(): bool { return is_zero(@0x00); }
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
                module test;
                pub fn identity(a: address): address { return a; }
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
        module test;
        pub fn same(a: address, b: address): bool { return a == b; }
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
// ─── Struct ───
//

#[test]
fn struct_construction_and_field_access() {
    let src = r#"
        module test;

        struct Point { x: u64, y: u64 }

        pub fn make(x: u64, y: u64): Point { return Point { x: x, y: y }; }
        pub fn get_x(p: Point): u64 { return p.x; }
    "#;
    let vm = utils::vm(utils::compile(src));
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
        module test;

        struct Counter { value: u64 }

        pub fn get_value(c: Counter): u64 { return c.value; }
    "#;
    let vm = utils::vm(utils::compile(src));
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
        module test;

        struct Counter { value: u64 }

        pub fn increment(c: Counter): Counter { c.value = c.value + 1; return c; }
    "#;
    assert_eq!(
        utils::run(src, "increment", vec![test_counter(5)]),
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
    let src = r#"
        module test;
        pub fn greeting(): string { return "hello"; }
    "#;
    assert_eq!(
        utils::run(src, "greeting", vec![]),
        Some(Value::Str("hello".to_string()))
    );
}

#[test]
fn string_parameter_round_trip() {
    let src = r#"
        module test;
        pub fn identity(s: string): string { return s; }
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
        module test;
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

#[test]
fn string_struct_field_round_trip() {
    let src = r#"
        module test;

        struct Msg { text: string }

        pub fn make(text: string): Msg { return Msg { text: text }; }
        pub fn get_text(m: Msg): string { return m.text; }
    "#;
    let vm = utils::vm(utils::compile(src));
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
// ─── Nested structs ───
//

#[test]
fn nested_struct_construction_and_field_access() {
    let src = r#"
        module test;

        struct Inner { value: u64 }
        struct Outer { inner: Inner, label: u64 }

        pub fn make(v: u64, label: u64): Outer {
            let i = Inner { value: v };
            return Outer { inner: i, label: label };
        }

        pub fn get_inner_value(o: Outer): u64 {
            let i = o.inner;
            return i.value;
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();

    let outer = vm
        .call("make", vec![Value::U64(42), Value::U64(7)], &mut gas)
        .unwrap()
        .return_value
        .unwrap();

    // Verify structure
    assert!(matches!(&outer, Value::Struct { type_name, .. } if type_name == "Outer"));

    let inner_val = vm.call("get_inner_value", vec![outer], &mut gas).unwrap();
    assert_eq!(inner_val.return_value, Some(Value::U64(42)));
}

#[test]
fn nested_struct_field_mutation() {
    let src = r#"
        module test;

        struct Inner { x: u64 }
        struct Outer { inner: Inner }

        pub fn double_inner(o: Outer): Outer {
            o.inner = Inner { x: o.inner.x * 2 };
            return o;
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();

    let inner = Value::Struct {
        type_name: "Inner".to_string(),
        fields: vec![("x".to_string(), Value::U64(5))],
    };
    let outer = Value::Struct {
        type_name: "Outer".to_string(),
        fields: vec![("inner".to_string(), inner)],
    };

    let result = vm.call("double_inner", vec![outer], &mut gas).unwrap();
    let expected_inner = Value::Struct {
        type_name: "Inner".to_string(),
        fields: vec![("x".to_string(), Value::U64(10))],
    };
    assert_eq!(
        result.return_value,
        Some(Value::Struct {
            type_name: "Outer".to_string(),
            fields: vec![("inner".to_string(), expected_inner)],
        })
    );
}

#[test]
fn nested_struct_passed_as_argument() {
    let src = r#"
        module test;

        struct Span { start: u64, end: u64 }
        struct Range { span: Span }

        pub fn length(r: Range): u64 {
            return r.span.end - r.span.start;
        }
    "#;
    let span = Value::Struct {
        type_name: "Span".to_string(),
        fields: vec![
            ("start".to_string(), Value::U64(3)),
            ("end".to_string(), Value::U64(10)),
        ],
    };
    let range = Value::Struct {
        type_name: "Range".to_string(),
        fields: vec![("span".to_string(), span)],
    };
    assert_eq!(utils::run(src, "length", vec![range]), Some(Value::U64(7)));
}

#[test]
fn nested_struct_forward_reference() {
    // B is used as a field type in A even though B is defined after A.
    let src = r#"
        module test;

        struct A { b: B }
        struct B { value: u64 }

        pub fn get_value(a: A): u64 {
            let b = a.b;
            return b.value;
        }
    "#;
    let b = Value::Struct {
        type_name: "B".to_string(),
        fields: vec![("value".to_string(), Value::U64(99))],
    };
    let a = Value::Struct {
        type_name: "A".to_string(),
        fields: vec![("b".to_string(), b)],
    };
    assert_eq!(utils::run(src, "get_value", vec![a]), Some(Value::U64(99)));
}

#[test]
fn nested_struct_value_semantics() {
    // Structs are freely copyable — reading a nested struct field copies it.
    let src = r#"
        module test;

        struct Inner { n: u64 }
        struct Outer { inner: Inner }

        pub fn read_twice(o: Outer): u64 {
            let a = o.inner;
            let b = o.inner;
            return a.n + b.n;
        }
    "#;
    let inner = Value::Struct {
        type_name: "Inner".to_string(),
        fields: vec![("n".to_string(), Value::U64(21))],
    };
    let outer = Value::Struct {
        type_name: "Outer".to_string(),
        fields: vec![("inner".to_string(), inner)],
    };
    assert_eq!(
        utils::run(src, "read_twice", vec![outer]),
        Some(Value::U64(42))
    );
}

//
// ─── Object ───
//

#[test]
fn object_construction_and_field_access() {
    let src = r#"
        module test;

        object Coin { id: address, balance: u64 }

        pub fn make_coin(balance: u64): u64 {
            let c = Coin { id: meow_vm_fresh_id(), balance: balance };
            return c.balance;
        }
    "#;
    let vm = utils::vm_with_natives(src, vec![utils::fresh_id_native()]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("make_coin", vec![Value::U64(100)], &mut gas)
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(100)));
}

#[test]
fn object_field_mutation_reflected_in_final_args() {
    let src = r#"
        module test;

        object Coin { id: address, balance: u64 }

        pub fn double_balance(coin: Coin): u64 {
            coin.balance = coin.balance * 2;
            return coin.balance;
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call(
            "double_balance",
            vec![test_coin(Address::fill(1), 50)],
            &mut gas,
        )
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(100)));
    assert_eq!(r.final_args[0], Some(test_coin(Address::fill(1), 100)));
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

fn test_coin(id: Address, balance: u64) -> Value {
    Value::Object {
        type_name: "Coin".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("balance".to_string(), Value::U64(balance)),
        ],
    }
}
