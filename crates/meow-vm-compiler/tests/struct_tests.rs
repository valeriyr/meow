mod utils;

use meow_vm_compiler::error::CompilerError;

//
// ─── Field definitions ───
//

#[test]
fn struct_field_can_be_string_type() {
    let src = r#"
        mod test;

        struct Msg { text: string }

        fn make(text: string) -> Msg { Msg { text: text } }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_field_can_be_another_struct_type() {
    let src = r#"
        mod test;

        struct Inner { x: u64 }
        struct Outer { inner: Inner, y: bool }

        fn noop() {}
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_id_field_is_arbitrary() {
    // Unlike objects, structs have no special `id` field rule.
    // A struct may have a field named `id` of any type, assigned to any value.
    let src = r#"
        mod test;

        struct Receipt { id: u64, amount: u64 }

        pub fn make(id: u64, amount: u64) -> Receipt { Receipt { id: id, amount: amount } }
        pub fn to_id(r: Receipt) -> u64 { let Receipt { id, .. } = r; id }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn empty_struct_rejected() {
    let src = r#"
        mod test;

        struct Empty {}

        fn noop() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("must have at least one field")
    ));
}

#[test]
fn struct_field_unknown_type_rejected() {
    let src = r#"
        mod test;

        struct Wrapper { item: NonExistent }

        fn noop() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("unknown struct 'NonExistent'")
    ));
}

//
// ─── Cycles ───
//

#[test]
fn struct_cycle_direct_rejected() {
    let src = r#"
        mod test;

        struct A { b: B }
        struct B { a: A }

        fn noop() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cycle")
    ));
}

#[test]
fn struct_cycle_indirect_rejected() {
    // A → B → C → A
    let src = r#"
        mod test;

        struct A { b: B }
        struct B { c: C }
        struct C { a: A }

        fn noop() {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cycle")
    ));
}

//
// ─── Move semantics (linearity) ───
//

#[test]
fn struct_typed_field_read_rejected() {
    // `let b = outer.inner;` where `inner: Inner` — struct-typed field access is forbidden.
    // Structs have move semantics; use destructuring to extract struct-typed fields.
    let src = r#"
        mod test;

        struct Inner { value: u64 }
        struct Outer { inner: Inner, amount: u64 }

        pub fn bad(o: Outer) -> u64 {
            let b = o.inner;
            let Inner { value } = b;
            value
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("struct type") && msg.contains("inner")
    ));
}

#[test]
fn struct_typed_field_write_rejected() {
    // `outer.inner = new_inner;` — writing a struct into a struct-typed field is forbidden.
    // The old field value would be implicitly dropped, violating linearity.
    let src = r#"
        mod test;

        struct Inner { value: u64 }
        struct Outer { inner: Inner, amount: u64 }

        pub fn bad(o: Outer) -> Outer {
            let new_inner = Inner { value: 99 };
            o.inner = new_inner;
            o
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("move semantics") && msg.contains("inner")
    ));
}

#[test]
fn getfield_on_call_result_struct_typed_field_rejected() {
    // make_outer().inner — GetField fallback path (root is a Call, not an Ident).
    // The accessed field 'inner' has struct type — forbidden even via the fallback path.
    let src = r#"
        mod test;

        struct Inner { value: u64 }
        struct Outer { inner: Inner, amount: u64 }

        fn make_outer() -> Outer { Outer { inner: Inner { value: 1 }, amount: 2 } }

        pub fn bad() -> Inner { make_outer().inner }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("struct type") && msg.contains("inner")
    ));
}

#[test]
fn getfield_on_call_result_drops_linear_field_rejected() {
    // make_outer().amount — result is u64 but inner: Inner would be silently dropped.
    let src = r#"
        mod test;

        struct Inner { value: u64 }
        struct Outer { inner: Inner, amount: u64 }

        fn make_outer() -> Outer { Outer { inner: Inner { value: 1 }, amount: 2 } }

        pub fn bad() -> u64 { make_outer().amount }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("inner") && msg.contains("dropped")
    ));
}

#[test]
fn struct_param_not_consumed_rejected() {
    // A struct parameter that is never moved or destroyed must be rejected.
    // Unconsumed struct params violate linearity — the value is silently lost.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        pub fn noop(tok: Token) {}
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("must be consumed") && msg.contains("tok")
    ));
}

//
// ─── Destructuring ───
//

#[test]
fn struct_destructuring_compiles() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn sum(p: Point) -> u64 {
            let Point { x, y } = p;
            x + y
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_destructuring_unknown_field_rejected() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn bad(p: Point) -> u64 {
            let Point { x, z } = p;
            x + z
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("unknown field 'z'")
    ));
}

#[test]
fn struct_destructuring_missing_field_rejected() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn bad(p: Point) -> u64 {
            let Point { x } = p;
            x
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("missing binding for field 'y'") && msg.contains("..")
    ));
}

#[test]
fn struct_destructuring_cannot_discard_struct_field() {
    let src = r#"
        mod test;

        struct Inner { value: u64 }
        struct Outer { name: u64, data: Inner }

        pub fn bad(o: Outer) -> u64 {
            let Outer { name, .. } = o;
            name
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot discard") && msg.contains("'data'")
    ));
}
