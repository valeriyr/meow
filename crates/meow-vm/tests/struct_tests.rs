mod utils;

use meow_vm::gas_meter::GasMeter;
use meow_vm_types::{
    address::Address,
    module_ref,
    natives::{NativeFnEntry, NativeParam, NativeResult},
    types::{Type, Value},
};

//
// ─── Struct ───
//

#[test]
fn struct_construction_and_field_access() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn make(x: u64, y: u64) -> Point { Point { x: x, y: y } }
        pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
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
            type_name: qualify("Point"),
            fields: vec![
                ("x".to_string(), Value::U64(3)),
                ("y".to_string(), Value::U64(7))
            ],
        }
    );

    let r = vm.call("to_x", vec![point], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::U64(3)));
}

#[test]
fn struct_value_semantics() {
    // Structs have move semantics inside the VM, but each vm.call() receives an
    // independent clone — so the same Rust Value can be submitted in multiple calls.
    let src = r#"
        mod test;

        struct Counter { value: u64 }

        pub fn to_value(c: Counter) -> u64 { let Counter { value } = c; value }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();
    let c = test_counter(42);
    assert_eq!(
        vm.call("to_value", vec![c.clone()], &mut gas)
            .unwrap()
            .return_value,
        Some(Value::U64(42))
    );
    assert_eq!(
        vm.call("to_value", vec![c], &mut gas).unwrap().return_value,
        Some(Value::U64(42))
    );
}

#[test]
fn struct_id_field_is_arbitrary() {
    // Structs have no special `id` field rule. A field named `id` can hold any
    // type and be assigned any value.
    let src = r#"
        mod test;

        struct Receipt { id: u64, amount: u64 }

        pub fn make(id: u64, amount: u64) -> Receipt { Receipt { id: id, amount: amount } }
        pub fn to_id(r: Receipt) -> u64 { let Receipt { id, .. } = r; id }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();

    let receipt = vm
        .call("make", vec![Value::U64(99), Value::U64(500)], &mut gas)
        .unwrap()
        .return_value
        .unwrap();

    assert_eq!(
        vm.call("to_id", vec![receipt], &mut gas)
            .unwrap()
            .return_value,
        Some(Value::U64(99))
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
            type_name: qualify("Counter"),
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
        pub fn to_text(m: Msg) -> string { let Msg { text } = m; text }
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
            type_name: qualify("Msg"),
            fields: vec![("text".to_string(), Value::Str("hello".to_string()))],
        }
    );

    let r = vm.call("to_text", vec![msg], &mut gas).unwrap();
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
            let Outer { inner, .. } = o;
            let Inner { value } = inner;
            value
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();

    let outer = vm
        .call("make", vec![Value::U64(42), Value::U64(7)], &mut gas)
        .unwrap()
        .return_value
        .unwrap();

    assert!(matches!(&outer, Value::Struct { type_name, .. } if type_name == &qualify("Outer")));

    let inner_val = vm.call("get_inner_value", vec![outer], &mut gas).unwrap();
    assert_eq!(inner_val.return_value, Some(Value::U64(42)));
}

#[test]
fn nested_struct_field_mutation() {
    // Structs have move semantics — access nested struct by destructuring, then reconstruct.
    let src = r#"
        mod test;

        struct Inner { x: u64 }
        struct Outer { inner: Inner }

        pub fn double_inner(o: Outer) -> Outer {
            let Outer { inner } = o;
            let Inner { x } = inner;
            Outer { inner: Inner { x: x * 2 } }
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();

    let inner = Value::Struct {
        type_name: qualify("Inner"),
        fields: vec![("x".to_string(), Value::U64(5))],
    };
    let outer = Value::Struct {
        type_name: qualify("Outer"),
        fields: vec![("inner".to_string(), inner)],
    };

    let result = vm.call("double_inner", vec![outer], &mut gas).unwrap();
    let expected_inner = Value::Struct {
        type_name: qualify("Inner"),
        fields: vec![("x".to_string(), Value::U64(10))],
    };
    assert_eq!(
        result.return_value,
        Some(Value::Struct {
            type_name: qualify("Outer"),
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
            let Range { span } = r;
            let Span { start, end } = span;
            end - start
        }
    "#;
    let span = Value::Struct {
        type_name: qualify("Span"),
        fields: vec![
            ("start".to_string(), Value::U64(3)),
            ("end".to_string(), Value::U64(10)),
        ],
    };
    let range = Value::Struct {
        type_name: qualify("Range"),
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
            let A { b } = a;
            let B { value } = b;
            value
        }
    "#;
    let b = Value::Struct {
        type_name: qualify("B"),
        fields: vec![("value".to_string(), Value::U64(99))],
    };
    let a = Value::Struct {
        type_name: qualify("A"),
        fields: vec![("b".to_string(), b)],
    };
    assert_eq!(utils::run(src, "get_value", vec![a]), Some(Value::U64(99)));
}

#[test]
fn nested_struct_value_semantics() {
    // Nested struct fields are moved by destructuring — they cannot be read twice.
    let src = r#"
        mod test;

        struct Inner { n: u64 }
        struct Outer { inner: Inner }

        pub fn read_value(o: Outer) -> u64 {
            let Outer { inner } = o;
            let Inner { n } = inner;
            n * 2
        }
    "#;
    let inner = Value::Struct {
        type_name: qualify("Inner"),
        fields: vec![("n".to_string(), Value::U64(21))],
    };
    let outer = Value::Struct {
        type_name: qualify("Outer"),
        fields: vec![("inner".to_string(), inner)],
    };
    assert_eq!(
        utils::run(src, "read_value", vec![outer]),
        Some(Value::U64(42))
    );
}

//
// ─── Struct destructuring ───
//

#[test]
fn struct_destructuring_binds_fields() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn sum(p: Point) -> u64 {
            let Point { x, y } = p;
            x + y
        }
    "#;
    let point = Value::Struct {
        type_name: qualify("Point"),
        fields: vec![
            ("x".to_string(), Value::U64(3)),
            ("y".to_string(), Value::U64(7)),
        ],
    };
    assert_eq!(utils::run(src, "sum", vec![point]), Some(Value::U64(10)));
}

#[test]
fn struct_destructuring_single_field() {
    let src = r#"
        mod test;

        struct Wrapper { value: u64 }

        pub fn unwrap(w: Wrapper) -> u64 {
            let Wrapper { value } = w;
            value
        }
    "#;
    let w = Value::Struct {
        type_name: qualify("Wrapper"),
        fields: vec![("value".to_string(), Value::U64(42))],
    };
    assert_eq!(utils::run(src, "unwrap", vec![w]), Some(Value::U64(42)));
}

#[test]
fn struct_destructuring_rest_discards_unbound_fields() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn get_x(p: Point) -> u64 {
            let Point { x, .. } = p;
            x
        }
    "#;
    let point = Value::Struct {
        type_name: qualify("Point"),
        fields: vec![
            ("x".to_string(), Value::U64(5)),
            ("y".to_string(), Value::U64(99)),
        ],
    };
    assert_eq!(utils::run(src, "get_x", vec![point]), Some(Value::U64(5)));
}

#[test]
fn struct_destructuring_all_discarded() {
    // `{ .. }` consumes the struct without binding any fields
    let src = r#"
        mod test;

        struct Event { code: u64, value: u64 }

        pub fn consume(e: Event) {
            let Event { .. } = e;
            return;
        }
    "#;
    let event = Value::Struct {
        type_name: qualify("Event"),
        fields: vec![
            ("code".to_string(), Value::U64(1)),
            ("value".to_string(), Value::U64(42)),
        ],
    };
    assert_eq!(utils::run(src, "consume", vec![event]), None);
}

#[test]
fn struct_destructuring_binds_only_non_first_field() {
    let src = r#"
        mod test;

        struct Triple { a: u64, b: u64, c: u64 }

        pub fn get_c(t: Triple) -> u64 {
            let Triple { c, .. } = t;
            c
        }
    "#;
    let triple = Value::Struct {
        type_name: qualify("Triple"),
        fields: vec![
            ("a".to_string(), Value::U64(1)),
            ("b".to_string(), Value::U64(2)),
            ("c".to_string(), Value::U64(3)),
        ],
    };
    assert_eq!(utils::run(src, "get_c", vec![triple]), Some(Value::U64(3)));
}

#[test]
fn struct_destructuring_then_reconstruct() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn swap(p: Point) -> Point {
            let Point { x, y } = p;
            Point { x: y, y: x }
        }
    "#;
    let point = Value::Struct {
        type_name: qualify("Point"),
        fields: vec![
            ("x".to_string(), Value::U64(3)),
            ("y".to_string(), Value::U64(7)),
        ],
    };
    assert_eq!(
        utils::run(src, "swap", vec![point]),
        Some(Value::Struct {
            type_name: qualify("Point"),
            fields: vec![
                ("x".to_string(), Value::U64(7)),
                ("y".to_string(), Value::U64(3))
            ],
        })
    );
}

//
// ─── Equality ───
//

#[test]
fn struct_eq() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn same(a: Point, b: Point) -> bool { a == b }
    "#;
    let make = |x, y| Value::Struct {
        type_name: qualify("Point"),
        fields: vec![
            ("x".to_string(), Value::U64(x)),
            ("y".to_string(), Value::U64(y)),
        ],
    };
    assert_eq!(
        utils::run(src, "same", vec![make(3, 7), make(3, 7)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(src, "same", vec![make(3, 7), make(3, 8)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn struct_ne() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn different(a: Point, b: Point) -> bool { a != b }
    "#;
    let make = |x, y| Value::Struct {
        type_name: qualify("Point"),
        fields: vec![
            ("x".to_string(), Value::U64(x)),
            ("y".to_string(), Value::U64(y)),
        ],
    };
    assert_eq!(
        utils::run(src, "different", vec![make(1, 2), make(1, 3)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(src, "different", vec![make(1, 2), make(1, 2)]),
        Some(Value::Bool(false))
    );
}

#[test]
fn struct_eq_nested_compares_recursively() {
    let src = r#"
        mod test;

        struct Inner { v: u64 }
        struct Outer { inner: Inner }

        pub fn same(a: Outer, b: Outer) -> bool { a == b }
    "#;
    let make = |v| Value::Struct {
        type_name: qualify("Outer"),
        fields: vec![(
            "inner".to_string(),
            Value::Struct {
                type_name: qualify("Inner"),
                fields: vec![("v".to_string(), Value::U64(v))],
            },
        )],
    };
    assert_eq!(
        utils::run(src, "same", vec![make(42), make(42)]),
        Some(Value::Bool(true))
    );
    assert_eq!(
        utils::run(src, "same", vec![make(42), make(99)]),
        Some(Value::Bool(false))
    );
}

//
// ─── Qualification invariant ───
//

#[test]
#[should_panic(expected = "struct argument has unqualified type name")]
fn unqualified_struct_arg_panics() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
    "#;
    let vm = utils::vm(utils::compile(src));
    let unqualified = Value::Struct {
        type_name: "Point".to_string(),
        fields: vec![
            ("x".to_string(), Value::U64(1)),
            ("y".to_string(), Value::U64(2)),
        ],
    };
    let mut gas = GasMeter::unlimited();
    let _ = vm.call("to_x", vec![unqualified], &mut gas);
}

#[test]
#[should_panic(expected = "struct argument has unqualified type name")]
fn unqualified_nested_struct_field_panics() {
    let src = r#"
        mod test;

        struct Inner { v: u64 }
        struct Outer { inner: Inner }

        pub fn get_v(o: Outer) -> u64 {
            let Outer { inner } = o;
            let Inner { v } = inner;
            v
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let outer = Value::Struct {
        type_name: qualify("Outer"),
        fields: vec![(
            "inner".to_string(),
            Value::Struct {
                type_name: "Inner".to_string(), // unqualified nested field
                fields: vec![("v".to_string(), Value::U64(42))],
            },
        )],
    };
    let mut gas = GasMeter::unlimited();
    let _ = vm.call("get_v", vec![outer], &mut gas);
}

#[test]
#[should_panic(
    expected = "native function 'make_token' returned a struct with unqualified type name"
)]
fn native_returning_unqualified_struct_panics() {
    let src = r#"
        mod test;

        struct Token { amount: u64 }

        pub fn run() -> Token { make_token(99) }
    "#;
    let native = NativeFnEntry {
        name: "make_token".to_string(),
        params: vec![NativeParam::Concrete(Type::U64)],
        return_type: Some(Type::Struct("Token".to_string())),
        gas_cost: 0,
        func: Box::new(|args| {
            NativeResult::Return(Some(Value::Struct {
                type_name: "Token".to_string(), // unqualified — should panic
                fields: vec![("amount".to_string(), args[0].clone())],
            }))
        }),
    };
    let vm = utils::vm_with_natives(src, vec![native]);
    let mut gas = GasMeter::unlimited();
    let _ = vm.call("run", vec![], &mut gas);
}

//
// ─── Utility functions ───
//

fn test_counter(value: u64) -> Value {
    Value::Struct {
        type_name: qualify("Counter"),
        fields: vec![("value".to_string(), Value::U64(value))],
    }
}

fn qualify(name: &str) -> String {
    module_ref::qualify(&Address::ZERO, name)
}
