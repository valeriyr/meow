use meow_types::object::identifier::{Identifier, error::IdentifierError};

//
// Identifier creation tests.
//

#[test]
fn valid_identifier() {
    let id = Identifier::new("hello").unwrap();
    assert_eq!(id.as_ref(), "hello");
}

#[test]
fn valid_identifier_try_from() {
    let id = Identifier::try_from("hello").unwrap();
    assert_eq!(id.as_ref(), "hello");
}

#[test]
fn valid_identifier_single_char() {
    assert!(Identifier::new("a").is_ok());
}

#[test]
fn valid_identifier_uppercase() {
    assert!(Identifier::new("Hello").is_ok());
}

#[test]
fn valid_identifier_all_uppercase() {
    assert!(Identifier::new("HELLO").is_ok());
}

#[test]
fn valid_identifier_with_underscore() {
    let id = Identifier::new("hello_world").unwrap();
    assert_eq!(id.as_ref(), "hello_world");
}

#[test]
fn valid_identifier_with_digits() {
    assert!(Identifier::new("hello1").is_ok());
    assert!(Identifier::new("hello1_world").is_ok());
    assert!(Identifier::new("hello1_world2").is_ok());
}

//
// Identifier validation tests.
//

#[test]
fn invalid_identifier_empty() {
    assert!(matches!(
        Identifier::new("").unwrap_err(),
        IdentifierError::InvalidIdentifier(s) if s.is_empty()
    ));
}

#[test]
fn invalid_identifier_starting_with_digit() {
    assert!(matches!(
        Identifier::new("1hello").unwrap_err(),
        IdentifierError::InvalidIdentifier(s) if s == "1hello"
    ));
}

#[test]
fn invalid_identifier_with_spaces() {
    assert!(matches!(
        Identifier::new("hello world").unwrap_err(),
        IdentifierError::InvalidIdentifier(s) if s == "hello world"
    ));
}

#[test]
fn invalid_identifier_with_non_ascii() {
    assert!(matches!(
        Identifier::new("🦀").unwrap_err(),
        IdentifierError::InvalidIdentifier(s) if s == "🦀"
    ));
}

#[test]
fn invalid_identifier_try_from_non_ascii() {
    assert!(matches!(
        Identifier::try_from("🦀").unwrap_err(),
        IdentifierError::InvalidIdentifier(s) if s == "🦀"
    ));
}

//
// Identifier serialization tests.
//

#[test]
fn identifier_display() {
    let id = Identifier::new("hello").unwrap();
    assert_eq!(id.to_string(), "hello");
}

#[test]
fn identifier_serialization_round_trip() {
    let id = Identifier::new("hello").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    let parsed: Identifier = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn identifier_serialized_value() {
    let id = Identifier::new("hello").unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"hello\"");
}

//
// Identifier equality and ordering tests.
//

#[test]
fn identifier_equality() {
    let id1 = Identifier::new("hello").unwrap();
    let id2 = Identifier::new("hello").unwrap();
    assert_eq!(id1, id2);
}

#[test]
fn identifier_inequality() {
    let id1 = Identifier::new("hello").unwrap();
    let id2 = Identifier::new("world").unwrap();
    assert_ne!(id1, id2);
}

#[test]
fn identifier_ordering() {
    let id1 = Identifier::new("apple").unwrap();
    let id2 = Identifier::new("banana").unwrap();
    assert!(id1 < id2);
    assert!(id2 > id1);
}

#[test]
fn identifier_clone() {
    let id = Identifier::new("hello").unwrap();
    assert_eq!(id.clone(), id);
}
