use base64::{Engine, engine::general_purpose};
use meow::output_encoder::OutputEncoder;

/// A named struct so that `Debug` / `Pretty` output is unambiguously Rust's
/// `{:?}` / `{:#?}` format rather than any `Display` implementation.
#[derive(Debug, serde::Serialize)]
struct Wrapper {
    value: u64,
}

//
// ─── Base64 tests ───
//

#[test]
fn base64_output_is_valid_base64() {
    let output = OutputEncoder::Base64.encode(&42u64).unwrap();

    assert!(general_purpose::STANDARD.decode(&output).is_ok());
}

#[test]
fn base64_roundtrip_preserves_value() {
    let original = 42u64;
    let encoded = OutputEncoder::Base64.encode(&original).unwrap();
    let bytes = general_purpose::STANDARD.decode(&encoded).unwrap();
    let decoded: u64 = bcs::from_bytes(&bytes).unwrap();

    assert_eq!(decoded, original);
}

//
// ─── Debug tests ───
//

#[test]
fn debug_output_contains_value() {
    let output = OutputEncoder::Debug.encode(&Wrapper { value: 42 }).unwrap();

    assert_eq!(output, "Wrapper { value: 42 }");
}

#[test]
fn debug_output_is_single_line() {
    let output = OutputEncoder::Debug.encode(&Wrapper { value: 42 }).unwrap();

    assert!(!output.contains('\n'), "unexpected newline in: {output}");
}

//
// ─── Pretty tests ───
//

#[test]
fn pretty_output_contains_value() {
    let output = OutputEncoder::Pretty
        .encode(&Wrapper { value: 42 })
        .unwrap();

    assert_eq!(output, "Wrapper {\n    value: 42,\n}");
}

//
// ─── FromStr tests ───
//

#[test]
fn from_str_parses_all_lowercase_variants() {
    assert_eq!(
        "base64".parse::<OutputEncoder>().unwrap(),
        OutputEncoder::Base64
    );
    assert_eq!(
        "debug".parse::<OutputEncoder>().unwrap(),
        OutputEncoder::Debug
    );
    assert_eq!(
        "pretty".parse::<OutputEncoder>().unwrap(),
        OutputEncoder::Pretty
    );
}

#[test]
fn from_str_unknown_variant_returns_error() {
    assert!("hex".parse::<OutputEncoder>().is_err());
}

#[test]
fn from_str_is_case_sensitive() {
    assert!("Base64".parse::<OutputEncoder>().is_err());
    assert!("DEBUG".parse::<OutputEncoder>().is_err());
}
