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
fn implicit_return_without_return_type_compiles() {
    utils::compile(
        r#"
            mod test;

            pub fn noop() { }
        "#,
    )
    .expect("void function with empty body must compile");
}

//
// ─── Explicit return mixed with trailing expression ───
//
// A function can use `return` for early exits and still fall through
// to a trailing expression as the default path.
//

#[test]
fn explicit_early_return_with_trailing_expression() {
    utils::compile(
        r#"
            mod test;

            pub fn bounded(x: u64) -> u64 {
                if x > 100 { return 100; }
                x
            }
        "#,
    )
    .expect("early return in branch + trailing expression must compile");
}

#[test]
fn explicit_return_in_both_branches_with_no_trailing() {
    utils::compile(
        r#"
            mod test;

            pub fn sign(x: u64) -> bool {
                if x > 0 { return true; } else { return false; }
            }
        "#,
    )
    .expect("explicit return in every branch must compile");
}
