mod utils;

//
// ─── Comments ───
//

#[test]
fn line_comment_inside_function_body() {
    utils::compile(
        r#"
            mod comments;

            // This is a top-level comment
            pub fn add(a: u64, b: u64) -> u64 {
                // add the two values and return
                a + b // inline comment
            }
        "#,
    )
    .expect("comments must be stripped before parsing");
}

#[test]
fn comment_with_slashes_in_string_not_stripped() {
    utils::compile(
        r#"
            mod comments;

            pub fn url() -> string { "https://example.com" }
        "#,
    )
    .expect("slashes inside string literals must not be treated as comments");
}

//
// ─── Integer literals ───
//

#[test]
fn u64_max_literal_compiles() {
    // u64::MAX must be accepted exactly (boundary).
    utils::compile(
        r#"
            mod ints;

            pub fn f() -> u64 { 18446744073709551615 }
        "#,
    )
    .expect("u64::MAX literal must compile");
}

#[test]
fn integer_literal_overflow_is_rejected() {
    // A literal larger than u64::MAX must be a compile error, not silently wrap to 0.
    let err = utils::compile(
        r#"
            mod ints;

            pub fn f() -> u64 { 99999999999999999999999 }
        "#,
    )
    .expect_err("an out-of-range integer literal must be rejected");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("does not fit in u64"),
        "expected an overflow error, got: {msg}"
    );
}
