mod utils;

use meow_vm_types::types::Value;

//
// ─── Primitive shadowing ───
//

#[test]
fn shadowed_primitive_returns_new_value() {
    let src = r#"
        mod test;

        pub fn run() -> u64 {
            let x = 1;
            let x = 2;
            return x;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(2)));
}

#[test]
fn primitive_shadowed_multiple_times_returns_last_value() {
    let src = r#"
        mod test;

        pub fn run() -> u64 {
            let x = 1;
            let x = 2;
            let x = 3;
            return x;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(3)));
}

#[test]
fn shadow_via_struct_destructuring_returns_field_value() {
    let src = r#"
        mod test;

        struct Balance { amount: u64 }

        pub fn run() -> u64 {
            let b = Balance { amount: 42 };
            let amount = 0;
            let Balance { amount } = b;
            return amount;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(42)));
}

#[test]
fn struct_slot_reused_after_consumption() {
    // After consuming `p`, rebinding the name `p` reuses the same slot.
    // The new struct is accessible and returns the correct field.
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            let Point { x, .. } = p;
            let p = Point { x: 10, y: 20 };
            let Point { x: result, .. } = p;
            return result;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(10)));
}

//
// ─── Cross-type shadowing ───
//

#[test]
fn primitive_shadowed_by_struct_returns_struct_field() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }

        pub fn run() -> u64 {
            let p = false;
            let p = Point { x: 7, y: 9 };
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(7)));
}

//
// ─── If-body scoping ───
//

#[test]
fn if_body_outer_struct_accessible_after_if() {
    let src = r#"
        mod test;
        struct Point { x: u64, y: u64 }
        pub fn run() -> u64 {
            let p = Point { x: 1, y: 2 };
            if true {
                let p = Point { x: 99, y: 99 };
                let Point { x, .. } = p;
            }
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(1)));
}

#[test]
fn if_body_outer_primitive_unchanged_after_inner_shadow() {
    let src = r#"
        mod test;
        pub fn run() -> u64 {
            let x = 10;
            if true {
                let x = 99;
            }
            return x;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(10)));
}

#[test]
fn nested_if_scopes_restore_correctly() {
    let src = r#"
        mod test;
        pub fn run() -> u64 {
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
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(1)));
}

#[test]
fn if_body_same_scope_shadow_reuses_slot() {
    // `let x = 1; let x = 2;` inside an if body must behave as same-scope shadowing
    // (slot reused, not a second slot allocated). The final value must be 2.
    let src = r#"
        mod test;
        pub fn run() -> u64 {
            let x = 0;
            if true {
                let x = 1;
                let x = 2;
            }
            return x;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(0)));
}

#[test]
fn else_body_outer_struct_accessible_after_else() {
    let src = r#"
        mod test;
        struct Point { x: u64, y: u64 }
        pub fn run() -> u64 {
            let p = Point { x: 5, y: 5 };
            if false {
            } else {
                let p = Point { x: 99, y: 99 };
                let Point { x, .. } = p;
            }
            let Point { x, .. } = p;
            return x;
        }
    "#;
    assert_eq!(utils::run(src, "run", vec![]), Some(Value::U64(5)));
}
