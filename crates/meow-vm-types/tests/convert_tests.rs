use meow_vm_types::{
    convert::{error::ConversionError, object_from_rust, struct_from_rust, value_to_rust},
    types::Value,
};
use serde::{Deserialize, Serialize};

//
// ─── Primitives ───
//

#[test]
fn bool_to_rust() {
    let val = Value::Bool(true);
    assert!(value_to_rust::<bool>(&val).unwrap());
}

#[test]
fn u64_to_rust() {
    let val = Value::U64(42);
    assert_eq!(value_to_rust::<u64>(&val).unwrap(), 42u64);
}

#[test]
fn address_to_rust() {
    let addr = [0xABu8; 32];
    let val = Value::Address(addr);
    assert_eq!(value_to_rust::<[u8; 32]>(&val).unwrap(), addr);
}

#[test]
fn string_to_rust() {
    let val = Value::Str("meow".to_string());
    assert_eq!(value_to_rust::<String>(&val).unwrap(), "meow");
}

//
// ─── Struct ───
//

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Point {
    x: u64,
    y: u64,
}

#[test]
fn struct_from_value() {
    let val = Value::Struct {
        type_name: "Point".to_string(),
        fields: vec![
            ("x".to_string(), Value::U64(3)),
            ("y".to_string(), Value::U64(7)),
        ],
    };
    assert_eq!(value_to_rust::<Point>(&val).unwrap(), Point { x: 3, y: 7 });
}

//
// ─── Object ───
//

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct MeowCoin {
    id: [u8; 32],
    balance: u64,
}

#[test]
fn object_from_value() {
    let id = [0x01u8; 32];
    let val = Value::Object {
        type_name: "MeowCoin".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("balance".to_string(), Value::U64(100)),
        ],
    };
    assert_eq!(
        value_to_rust::<MeowCoin>(&val).unwrap(),
        MeowCoin { id, balance: 100 }
    );
}

//
// ─── Newtype address wrapper (mirrors meow_types::Address) ───
//

/// Mimics `meow_types::Address` — a newtype over `[u8; 32]`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Address([u8; 32]);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Coin {
    id: Address,
    balance: u64,
}

#[test]
fn object_from_rust_with_address_newtype() {
    let id = [0x42u8; 32];
    let coin = Coin {
        id: Address(id),
        balance: 100,
    };
    assert_eq!(
        object_from_rust(&coin).unwrap(),
        Value::Object {
            type_name: "Coin".to_string(),
            fields: vec![
                ("id".to_string(), Value::Address(id)),
                ("balance".to_string(), Value::U64(100)),
            ],
        }
    );
}

#[test]
fn round_trip_with_address_newtype() {
    let id = [0x42u8; 32];
    let original = Coin {
        id: Address(id),
        balance: 77,
    };
    let value = object_from_rust(&original).unwrap();
    assert_eq!(value_to_rust::<Coin>(&value).unwrap(), original);
}

//
// ─── struct_from_rust — Rust → Value ───
//

#[test]
fn struct_from_rust_produces_struct_value() {
    let point = Point { x: 3, y: 7 };
    assert_eq!(
        struct_from_rust(&point).unwrap(),
        Value::Struct {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Value::U64(3)),
                ("y".to_string(), Value::U64(7)),
            ],
        }
    );
}

#[test]
fn object_from_rust_produces_object_value() {
    let id = [0x02u8; 32];
    let coin = MeowCoin { id, balance: 50 };
    assert_eq!(
        object_from_rust(&coin).unwrap(),
        Value::Object {
            type_name: "MeowCoin".to_string(),
            fields: vec![
                ("id".to_string(), Value::Address(id)),
                ("balance".to_string(), Value::U64(50)),
            ],
        }
    );
}

#[test]
fn round_trip_struct() {
    let original = Point { x: 10, y: 20 };
    let value = struct_from_rust(&original).unwrap();
    assert_eq!(value_to_rust::<Point>(&value).unwrap(), original);
}

#[test]
fn round_trip_object() {
    let id = [0xFFu8; 32];
    let original = MeowCoin { id, balance: 999 };
    let value = object_from_rust(&original).unwrap();
    assert_eq!(value_to_rust::<MeowCoin>(&value).unwrap(), original);
}

#[test]
fn unsupported_type_returns_error() {
    #[derive(Serialize)]
    struct WithSeq {
        items: Vec<u64>,
    }
    let v = WithSeq {
        items: vec![1, 2, 3],
    };
    assert!(matches!(
        struct_from_rust(&v).unwrap_err(),
        ConversionError::UnsupportedType(ref msg) if msg == "seq"
    ));
}

#[test]
fn object_from_rust_rejects_non_address_first_field() {
    #[derive(Serialize)]
    struct BadObject {
        balance: u64,
    }
    let v = BadObject { balance: 10 };
    assert!(matches!(
        object_from_rust(&v).unwrap_err(),
        ConversionError::UnsupportedType(msg) if msg.contains("first field") && msg.contains("[u8; 32]")
    ));
}

#[test]
fn object_from_rust_rejects_empty_struct() {
    #[derive(Serialize)]
    struct Empty {}
    let v = Empty {};
    assert!(matches!(
        object_from_rust(&v).unwrap_err(),
        ConversionError::UnsupportedType(msg) if msg.contains("at least one field")
    ));
}

//
// ─── struct_from_rust — rejection of unsupported field types ───
//

#[test]
fn struct_from_rust_rejects_f32() {
    #[derive(Serialize)]
    struct WithFloat {
        value: f32,
    }
    let v = WithFloat { value: 1.0 };
    assert!(matches!(
        struct_from_rust(&v).unwrap_err(),
        ConversionError::UnsupportedType(msg) if msg == "f32"
    ));
}

#[test]
fn struct_from_rust_rejects_option() {
    #[derive(Serialize)]
    struct WithOption {
        maybe: Option<u64>,
    }
    // Option::None serializes via serialize_none.
    let v_none = WithOption { maybe: None };
    assert!(matches!(
        struct_from_rust(&v_none).unwrap_err(),
        ConversionError::UnsupportedType(msg) if msg.contains("Option")
    ));
    // Option::Some serializes via serialize_some.
    let v_some = WithOption { maybe: Some(42) };
    assert!(matches!(
        struct_from_rust(&v_some).unwrap_err(),
        ConversionError::UnsupportedType(msg) if msg.contains("Option")
    ));
}

#[test]
fn tuple_of_wrong_length_rejected() {
    // A [u8; 4] array serializes as a 4-element tuple of u8, which is not the
    // supported 32-element address tuple.
    #[derive(Serialize)]
    struct WithSmallArray {
        tag: [u8; 4],
    }
    let v = WithSmallArray { tag: [1, 2, 3, 4] };
    assert!(matches!(
        struct_from_rust(&v).unwrap_err(),
        ConversionError::UnsupportedType(msg) if msg.contains("tuple of length 4")
    ));
}

//
// ─── Errors ───
//

#[test]
fn void_returns_error() {
    assert!(matches!(
        value_to_rust::<u64>(&Value::Void).unwrap_err(),
        ConversionError::BcsError(bcs::Error::Custom(msg)) if msg.contains("void value cannot be serialized")
    ));
}

#[test]
fn wrong_type_returns_deserialize_error() {
    // Bool cannot be deserialized as u64
    let val = Value::Bool(true);
    assert!(matches!(
        value_to_rust::<u64>(&val).unwrap_err(),
        ConversionError::BcsError(bcs::Error::Eof)
    ));
}
