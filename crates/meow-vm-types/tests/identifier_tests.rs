use meow_vm_types::{config::CompilerConfig, identifier};

//
// ─── Identifier validity tests ───
//

#[test]
fn valid_identifier() {
    assert!(is_valid_identifier("hello"));
}

#[test]
fn valid_identifier_single_char() {
    assert!(is_valid_identifier("a"));
}

#[test]
fn valid_identifier_uppercase() {
    assert!(is_valid_identifier("Hello"));
}

#[test]
fn valid_identifier_all_uppercase() {
    assert!(is_valid_identifier("HELLO"));
}

#[test]
fn valid_identifier_with_underscore() {
    assert!(is_valid_identifier("hello_world"));
}

#[test]
fn valid_identifier_with_digits() {
    assert!(is_valid_identifier("hello1"));
    assert!(is_valid_identifier("hello1_world"));
    assert!(is_valid_identifier("hello1_world2"));
}

//
// ─── Identifier rejection tests ───
//

#[test]
fn invalid_identifier_empty() {
    assert!(!is_valid_identifier(""));
}

#[test]
fn invalid_identifier_starting_with_digit() {
    assert!(!is_valid_identifier("1hello"));
}

#[test]
fn invalid_identifier_with_spaces() {
    assert!(!is_valid_identifier("hello world"));
}

#[test]
fn invalid_identifier_with_non_ascii() {
    assert!(!is_valid_identifier("🦀"));
}

#[test]
fn invalid_too_long_identifier() {
    let config = CompilerConfig::default();
    let long_identifier = "a".repeat(config.max_identifier_len() + 1);

    assert!(!identifier::is_valid_identifier(&long_identifier, &config));
}

//
// ─── Utility functions ───
//

fn is_valid_identifier(name: &str) -> bool {
    let config = CompilerConfig::default();
    identifier::is_valid_identifier(name, &config)
}
