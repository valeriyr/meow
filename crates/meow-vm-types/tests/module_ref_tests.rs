use std::str::FromStr;

use meow_vm_types::{address::Address, module_ref::parse_module_ref};

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
