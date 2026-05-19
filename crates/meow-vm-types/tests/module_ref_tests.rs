use std::str::FromStr;

use meow_vm_types::{
    address::Address,
    module_ref::{is_qualified, parse_module_ref, qualify},
};

//
// ─── Happy paths ───
//

#[test]
fn parses_valid_ref() {
    let addr = Address::from_str("0x01").unwrap();
    assert_eq!(
        parse_module_ref("@0x01::transfer"),
        Some((addr, "transfer"))
    );
}

#[test]
fn parses_full_address() {
    let hex = "0x02";
    let addr = Address::from_str(hex).unwrap();
    assert_eq!(
        parse_module_ref(&format!("{hex}::mint")),
        None,
        "missing @ prefix"
    );
    assert_eq!(
        parse_module_ref(&format!("@{hex}::mint")),
        Some((addr, "mint"))
    );
}

#[test]
fn preserves_function_name_exactly() {
    let (_, name) = parse_module_ref("@0x01::some_fn_name").unwrap();
    assert_eq!(name, "some_fn_name");
}

//
// ─── qualify ───
//

#[test]
fn qualify_produces_expected_format() {
    assert_eq!(
        qualify(&Address::ZERO, "Token"),
        "@0x0000000000000000000000000000000000000000000000000000000000000000::Token"
    );
}

#[test]
fn qualify_roundtrips_with_parse() {
    let addr = Address::from_str("0xabcd").unwrap();
    let qualified = qualify(&addr, "Transfer");

    let (parsed_addr, parsed_name) = parse_module_ref(&qualified).unwrap();

    assert_eq!(parsed_addr, addr);
    assert_eq!(parsed_name, "Transfer");
}

//
// ─── is_qualified ───
//

#[test]
fn is_qualified_returns_true_for_cross_module_ref() {
    assert!(is_qualified("dep::Foo"));
    assert!(is_qualified("my_module::Bar"));
    assert!(is_qualified("@0x01::Foo"));
}

#[test]
fn is_qualified_returns_false_for_plain_name() {
    assert!(!is_qualified("Foo"));
    assert!(!is_qualified(""));
}

//
// ─── Errors ───
//

#[test]
fn returns_none_for_plain_name() {
    assert_eq!(parse_module_ref("transfer"), None);
}

#[test]
fn returns_none_for_missing_at() {
    assert_eq!(parse_module_ref("0x01::transfer"), None);
}

#[test]
fn returns_none_for_missing_separator() {
    assert_eq!(parse_module_ref("@0x01"), None);
}

#[test]
fn returns_none_for_invalid_address() {
    assert_eq!(parse_module_ref("@not_an_address::transfer"), None);
}
