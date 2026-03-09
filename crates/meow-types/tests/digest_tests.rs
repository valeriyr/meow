use std::str::FromStr;

use meow_types::digest::{DIGEST_LENGTH, Digest, error::DigestError};

//
// Digest creation tests.
//

#[test]
fn zero_digest() {
    let digest = Digest::ZERO;
    assert_eq!(digest.to_string(), "11111111111111111111111111111111");
}

#[test]
fn custom_digest() {
    let digest = Digest::new([1; DIGEST_LENGTH]);
    let string = digest.to_string();
    let parsed = Digest::from_str(&string).unwrap();
    assert_eq!(digest, parsed);
}

//
// Digest computation tests.
//

#[test]
fn compute_produces_deterministic_digest() {
    let d1 = Digest::compute(b"hello").unwrap();
    let d2 = Digest::compute(b"hello").unwrap();
    assert_eq!(d1, d2);
}

#[test]
fn compute_different_inputs_produce_different_digests() {
    let d1 = Digest::compute(b"hello").unwrap();
    let d2 = Digest::compute(b"world").unwrap();
    assert_ne!(d1, d2);
}

#[test]
fn compute_not_zero() {
    let digest = Digest::compute(b"hello").unwrap();
    assert_eq!(
        digest.to_string(),
        "4PNCZERNLKAqwSYHhZpb7B4GE34eiYDPXGgeNKWNNaBp"
    );
}

//
// Digest conversion tests.
//

#[test]
fn digest_from_string() {
    let digest = Digest::compute(b"hello").unwrap();
    let string = digest.to_string();
    let parsed = Digest::from_str(&string).unwrap();
    assert_eq!(digest, parsed);
}

#[test]
fn digest_from_bytes() {
    let digest = Digest::compute(b"hello").unwrap();
    let parsed = Digest::try_from(digest.as_ref()).unwrap();
    assert_eq!(digest, parsed);
}

#[test]
fn digest_from_invalid_bytes_length() {
    let result = Digest::try_from([0u8; 16].as_ref());
    assert!(matches!(
        result.unwrap_err(),
        DigestError::InvalidDigestBytesLength {
            actual: 16,
            expected: DIGEST_LENGTH
        }
    ));
}

#[test]
fn digest_from_invalid_base58() {
    let result = Digest::from_str("not_valid_base58!");
    assert!(matches!(result.unwrap_err(), DigestError::Base58Error(_)));
}
