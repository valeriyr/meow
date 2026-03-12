use std::str::FromStr;

use meow_types::keypair::{error::KeyPairError, signature_scheme::SignatureScheme};

//
// ─── Signature scheme flag tests ───
//

#[test]
fn signature_scheme_flag() {
    assert_eq!(SignatureScheme::Ed25519.flag(), 0x00);
}

//
// ─── Signature scheme conversion tests ───
//

#[test]
fn signature_scheme_serialization() {
    assert_eq!(SignatureScheme::Ed25519.to_string(), "ed25519");
}

#[test]
fn signature_scheme_deserialization() {
    assert_eq!(
        SignatureScheme::from_str("ed25519"),
        Ok(SignatureScheme::Ed25519)
    );
}

#[test]
fn signature_scheme_conversion() {
    // To `u8` conversions.
    assert_eq!(Into::<u8>::into(SignatureScheme::Ed25519), 0x00);

    // From `u8` conversions.
    assert_eq!(
        SignatureScheme::try_from(0x00).unwrap(),
        SignatureScheme::Ed25519
    );

    // Invalid conversion.
    assert!(matches!(
        SignatureScheme::try_from(0x11),
        Err(KeyPairError::InvalidSignatureSchemeFlag { flag: 0x11 })
    ));
}
