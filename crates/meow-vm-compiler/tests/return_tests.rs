mod utils;

//
// ─── Implicit return ───
//
// A function body whose last item is an expression without a trailing semicolon
// returns that expression's value. Explicit `return` is not required.
//

#[test]
fn implicit_return_with_primitive() {
    utils::compile(
        r#"
            mod test;
            pub fn meaning_of_life() -> u64 { 42 }
        "#,
    )
    .expect("implicit return of u64 literal must compile");
}

#[test]
fn implicit_return_with_struct_constructor() {
    utils::compile(
        r#"
            mod test;
            struct Point { x: u64, y: u64 }
            pub fn origin() -> Point { Point { x: 0, y: 0 } }
        "#,
    )
    .expect("implicit return of struct constructor must compile");
}

#[test]
fn implicit_return_with_arithmetic() {
    utils::compile(
        r#"
            mod test;
            pub fn double(x: u64) -> u64 { x * 2 }
        "#,
    )
    .expect("implicit return of arithmetic expression must compile");
}

#[test]
fn implicit_return_with_object_constructor() {
    utils::compile(
        r#"
            mod test;
            object Coin { id: address, balance: u64 }
            pub fn mint(balance: u64) -> Coin {
                Coin { id: meow_vm_fresh_id(), balance: balance }
            }
        "#,
    )
    .expect("implicit return of object constructor must compile");
}

#[test]
fn implicit_return_without_return_type_compiles() {
    utils::compile(
        r#"
            mod test;
            pub fn noop() { }
        "#,
    )
    .expect("void function with empty body must compile");
}
