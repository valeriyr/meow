mod utils;

use meow_vm::gas_meter::GasMeter;
use meow_vm_types::types::Value;

//
// ─── Struct ───
//

#[test]
fn struct_construction_and_field_access() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn make(x: u64, y: u64) -> Point { Point { x: x, y: y } }
        pub fn get_x(p: Point) -> u64 { p.x }
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
        mod test;

        struct Counter { value: u64 }

        pub fn get_value(c: Counter) -> u64 { c.value }
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
        mod test;

        struct Counter { value: u64 }

        pub fn increment(c: Counter) -> Counter { c.value = c.value + 1; c }
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
// ─── String field ───
//

#[test]
fn string_struct_field_round_trip() {
    let src = r#"
        mod test;

        struct Msg { text: string }

        pub fn make(text: string) -> Msg { Msg { text: text } }
        pub fn get_text(m: Msg) -> string { m.text }
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
        mod test;

        struct Inner { value: u64 }
        struct Outer { inner: Inner, label: u64 }

        pub fn make(v: u64, label: u64) -> Outer {
            let i = Inner { value: v };
            Outer { inner: i, label: label }
        }

        pub fn get_inner_value(o: Outer) -> u64 {
            let i = o.inner;
            i.value
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();

    let outer = vm
        .call("make", vec![Value::U64(42), Value::U64(7)], &mut gas)
        .unwrap()
        .return_value
        .unwrap();

    assert!(matches!(&outer, Value::Struct { type_name, .. } if type_name == "Outer"));

    let inner_val = vm.call("get_inner_value", vec![outer], &mut gas).unwrap();
    assert_eq!(inner_val.return_value, Some(Value::U64(42)));
}

#[test]
fn nested_struct_field_mutation() {
    let src = r#"
        mod test;

        struct Inner { x: u64 }
        struct Outer { inner: Inner }

        pub fn double_inner(o: Outer) -> Outer {
            o.inner = Inner { x: o.inner.x * 2 };
            o
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
        mod test;

        struct Span { start: u64, end: u64 }
        struct Range { span: Span }

        pub fn length(r: Range) -> u64 {
            r.span.end - r.span.start
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
        mod test;

        struct A { b: B }
        struct B { value: u64 }

        pub fn get_value(a: A) -> u64 {
            let b = a.b;
            b.value
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
        mod test;

        struct Inner { n: u64 }
        struct Outer { inner: Inner }

        pub fn read_twice(o: Outer) -> u64 {
            let a = o.inner;
            let b = o.inner;
            a.n + b.n
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
// ─── Utility functions ───
//

fn test_counter(value: u64) -> Value {
    Value::Struct {
        type_name: "Counter".to_string(),
        fields: vec![("value".to_string(), Value::U64(value))],
    }
}
