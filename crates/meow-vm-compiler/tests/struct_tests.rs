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
// ─── Move semantics: use-after-move, drop, reassign ───
//

#[test]
fn double_move_of_struct_rejected() {
    // Passing the same struct to two consuming calls moves it twice.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn take(t: Token) { let Token { value } = t; }

        pub fn run(t: Token) {
            take(t);
            take(t);
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("after it was moved")
    ));
}

#[test]
fn struct_passed_to_two_params_rejected() {
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn two(a: Token, b: Token) {
            let Token { value: va } = a;
            let Token { value: vb } = b;
        }

        pub fn run(t: Token) {
            two(t, t);
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("after it was moved")
    ));
}

#[test]
fn bare_struct_expression_statement_rejected() {
    // A struct-returning call used as a bare statement would silently drop the struct.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn make() -> Token { Token { value: 1 } }

        pub fn run() {
            make();
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot be discarded")
    ));
}

#[test]
fn struct_literal_expression_statement_rejected() {
    let src = r#"
        mod test;

        struct Token { value: u64 }

        pub fn run() {
            Token { value: 1 };
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot be discarded")
    ));
}

#[test]
fn reassign_over_live_struct_rejected() {
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn make() -> Token { Token { value: 1 } }

        pub fn run() {
            let t = make();
            t = make();
            take(t);
        }

        fn take(t: Token) { let Token { value } = t; }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("still holds a struct value")
    ));
}

#[test]
fn reassign_after_move_then_leak_rejected() {
    // Move t out, then reassign a fresh struct into the slot and never consume it.
    // The fresh struct must be tracked so the leak is caught.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn make() -> Token { Token { value: 1 } }
        fn take(t: Token) { let Token { value } = t; }

        pub fn run() {
            let t = make();
            take(t);
            t = make();
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("must be consumed")
    ));
}

#[test]
fn struct_consumed_in_both_branches_compiles() {
    // Each path consumes the struct exactly once — legal linear code. Move tracking
    // must be per-branch: consumption in the then-branch is not a move on the
    // else-path.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn take(t: Token) { let Token { value } = t; }

        pub fn run(c: bool, t: Token) {
            if c {
                take(t);
            } else {
                take(t);
            }
        }
    "#;
    utils::compile(src).expect("consuming a struct once per branch must compile");
}

#[test]
fn struct_consumed_in_terminating_branch_and_after_compiles() {
    // The then-branch consumes the struct and returns, so it never reaches the
    // join — the fall-through consumption is the only one on that path.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn take(t: Token) { let Token { value } = t; }

        pub fn run(c: bool, t: Token) {
            if c {
                take(t);
                return;
            }
            take(t);
        }
    "#;
    utils::compile(src).expect("early-return branch consumption must compile");
}

#[test]
fn struct_moved_in_one_branch_only_then_used_rejected() {
    // Consumed on the then-path but still live on the fall-through path: the value's
    // liveness is path-dependent, so any later use is rejected.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn take(t: Token) { let Token { value } = t; }

        pub fn run(c: bool, t: Token) {
            if c {
                take(t);
            }
            take(t);
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("after it was moved")
    ));
}

#[test]
fn unconsumed_destructured_tuple_struct_rejected() {
    // Destructuring a tuple whose element is a struct, then leaking that struct.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn pair() -> (Token, u64) { (Token { value: 1 }, 7) }

        pub fn run() -> u64 {
            let (t, v) = pair();
            v
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("must be consumed")
    ));
}

#[test]
fn multiple_unconsumed_structs_report_first_declared() {
    // When several bindings leak at once, the error must deterministically name
    // the first-declared one (smallest slot).
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn make() -> Token { Token { value: 1 } }

        pub fn run() {
            let first = make();
            let second = make();
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("'first'")
    ));
}

#[test]
fn multiple_branch_structs_report_first_declared() {
    // Same determinism rule for the branch-scoped leak check.
    let src = r#"
        mod test;

        struct Token { value: u64 }

        fn make() -> Token { Token { value: 1 } }

        pub fn run(c: bool) {
            if c {
                let first = make();
                let second = make();
            }
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("'first'")
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
