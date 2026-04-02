use meow::output_formatter::OutputFormatter;
use serde::Serialize;

#[derive(Serialize)]
struct Point {
    x: u64,
    y: u64,
}

//
// ─── JSON tests ───
//

#[test]
fn json_output_is_valid_json() {
    let output = OutputFormatter::Json.format(&Point { x: 1, y: 2 }).unwrap();

    let v: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["x"], 1);
    assert_eq!(v["y"], 2);
}

#[test]
fn json_output_is_pretty_printed() {
    let output = OutputFormatter::Json.format(&Point { x: 1, y: 2 }).unwrap();

    // serde_json::to_string_pretty always includes newlines.
    assert!(output.contains('\n'), "expected pretty JSON, got: {output}");
}

//
// ─── Table tests ───
//

#[test]
fn table_output_contains_field_names() {
    let output = OutputFormatter::Table
        .format(&Point { x: 1, y: 2 })
        .unwrap();

    assert!(output.contains("| x |"), "unexpected output: {output}");
    assert!(output.contains("| y |"), "unexpected output: {output}");
}

#[test]
fn table_output_contains_field_values() {
    let output = OutputFormatter::Table
        .format(&Point { x: 1, y: 2 })
        .unwrap();

    assert!(output.contains("| x | 1 |"), "unexpected output: {output}");
    assert!(output.contains("| y | 2 |"), "unexpected output: {output}");
}

//
// ─── FromStr tests ───
//

#[test]
fn from_str_parses_all_lowercase_variants() {
    assert_eq!(
        "json".parse::<OutputFormatter>().unwrap(),
        OutputFormatter::Json
    );
    assert_eq!(
        "table".parse::<OutputFormatter>().unwrap(),
        OutputFormatter::Table
    );
}

#[test]
fn from_str_unknown_variant_returns_error() {
    assert!("csv".parse::<OutputFormatter>().is_err());
}

#[test]
fn from_str_is_case_sensitive() {
    assert!("JSON".parse::<OutputFormatter>().is_err());
    assert!("Table".parse::<OutputFormatter>().is_err());
}
