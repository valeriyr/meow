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
