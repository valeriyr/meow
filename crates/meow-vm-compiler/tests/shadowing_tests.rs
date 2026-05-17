mod utils;

use meow_vm_compiler::error::CompilerError;

//
// ─── Primitive shadowing ───
//

#[test]
fn primitive_shadowing_compiles() {
    let src = r#"
        mod test;

        fn run() -> u64 {
            let x = 1;
            let x = 2;
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn primitive_shadowing_in_let_tuple_compiles() {
    let src = r#"
        mod test;

        fn pair() -> (u64, u64) { (10, 20) }
        fn run() -> u64 {
            let x = 1;
            let (x, y) = pair();
            return x + y;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn primitive_shadowing_in_struct_destructuring_compiles() {
    let src = r#"
        mod test;

        struct Balance { amount: u64 }

        fn run() -> u64 {
            let b = Balance { amount: 42 };
            let amount = 0;
            let Balance { amount } = b;
            return amount;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn multiple_shadows_of_same_name_compile() {
    let src = r#"
        mod test;

        fn run() -> u64 {
            let x = 1;
            let x = 2;
            let x = 3;
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

//
// ─── Struct shadowing ───
//

#[test]
fn struct_shadowing_after_consumption_compiles() {
    // Destructuring p consumes it; rebinding p to a new struct should be allowed.
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            let Point { x, .. } = p;
            let p = Point { x: 3, y: 4 };
            let Point { x: result, .. } = p;
            return result;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_shadowing_after_move_into_function_compiles() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        fn consume(p: Point) -> u64 { let Point { x, .. } = p; return x; }
        fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            consume(p);
            let p = Point { x: 10, y: 20 };
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn live_struct_shadowing_is_rejected() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            let p = Point { x: 3, y: 4 };
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot shadow 'p'")
    ));
}

//
// ─── Cross-type shadowing ───
//

#[test]
fn primitive_to_struct_shadow_compiles() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        fn run() -> u64 {
            let p = false;
            let p = Point { x: 1, y: 2 };
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn struct_to_primitive_shadow_rejected_when_struct_live() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            let p = false;
            return 0;
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("cannot shadow 'p'")
    ));
}

#[test]
fn struct_to_primitive_shadow_allowed_after_consumption() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            let Point { x, .. } = p;
            let p = false;
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

//
// ─── If-body scoping ───
//

#[test]
fn if_body_shadow_compiles() {
    let src = r#"
        mod test;
        struct Point { x: u64, y: u64 }
        fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            if true {
                let p = Point { x: 3, y: 4 };
                let Point { x, .. } = p;
            }
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn if_body_variable_not_visible_after_if() {
    let src = r#"
        mod test;
        fn run() -> u64 {
            if true {
                let inner = 42;
            }
            return inner;
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("undefined variable 'inner'")
    ));
}

#[test]
fn if_body_unconsumed_struct_is_rejected() {
    let src = r#"
        mod test;
        struct Point { x: u64, y: u64 }
        fn run() {
            let p = Point { x: 1, y: 2 };
            if true {
                let p = Point { x: 3, y: 4 };
                // inner p not consumed — error
            }
            let Point { .. } = p;
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("must be consumed before the branch ends")
    ));
}

#[test]
fn else_body_unconsumed_struct_is_rejected() {
    let src = r#"
        mod test;
        struct Point { x: u64, y: u64 }
        fn run() {
            let p = Point { x: 1, y: 2 };
            if true {
                let Point { .. } = p;
            } else {
                let q = Point { x: 9, y: 9 };
                // q not consumed — error
            }
        }
    "#;
    assert!(matches!(
        utils::compile(src).unwrap_err(),
        CompilerError::Message(msg) if msg.contains("must be consumed before the branch ends")
    ));
}

#[test]
fn else_body_shadow_compiles() {
    let src = r#"
        mod test;
        struct Point { x: u64, y: u64 }
        fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            if false {
            } else {
                let p = Point { x: 9, y: 9 };
                let Point { x, .. } = p;
            }
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn nested_if_scopes_compile() {
    let src = r#"
        mod test;
        fn run() -> u64 {
            let x = 1;
            if true {
                let x = 2;
                if true {
                    let x = 3;
                }
            }
            return x;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}

#[test]
fn primitive_shadow_in_if_compiles() {
    let src = r#"
        mod test;
        struct Point { x: u64, y: u64 }
        fn run() -> u64 {
            let p = false;
            if true {
                let p = Point { x: 3, y: 4 };
                let Point { .. } = p;
            }
            return 0;
        }
    "#;
    assert!(utils::compile(src).is_ok());
}
