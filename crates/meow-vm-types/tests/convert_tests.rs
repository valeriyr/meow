use meow_vm_types::{
    convert::{VmTypeNames, error::ConversionError, struct_from_rust, value_to_rust},
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
    let raw = [0xABu8; 32];
    let val = Value::Address(raw.into());
    assert_eq!(value_to_rust::<[u8; 32]>(&val).unwrap(), raw);
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
impl VmTypeNames for Point {}

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
// ─── Newtype address wrapper (mirrors meow_types::Address) ───
//

/// Mimics `meow_types::Address` — a newtype over `[u8; 32]`, used to test
/// `serialize_newtype_struct` transparent unwrapping.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct AddressWrapper([u8; 32]);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Coin {
    id: AddressWrapper,
    balance: u64,
}
impl VmTypeNames for Coin {}

#[test]
fn struct_from_rust_with_address_newtype() {
    let id = [0x42u8; 32];
    let coin = Coin {
        id: AddressWrapper(id),
        balance: 100,
    };
    assert_eq!(
        struct_from_rust(&coin).unwrap(),
        Value::Struct {
            type_name: "Coin".to_string(),
            fields: vec![
                ("id".to_string(), Value::Address(id.into())),
                ("balance".to_string(), Value::U64(100)),
            ],
        }
    );
}

#[test]
fn round_trip_with_address_newtype() {
    let id = [0x42u8; 32];
    let original = Coin {
        id: AddressWrapper(id),
        balance: 77,
    };
    let value = struct_from_rust(&original).unwrap();
    assert_eq!(value_to_rust::<Coin>(&value).unwrap(), original);
}

//
// ─── VmTypeNames — type-name translation ───
//

// Simulates a struct from another module with a qualified type name.
// Its Serialize impl uses the short local name "ObjectId".
#[derive(Serialize)]
struct ObjectId {
    inner: [u8; 32],
}

// Simulates a struct whose field type comes from another module.
// VmTypeNames maps "ObjectId" → the address-qualified bytecode name.
#[derive(Serialize)]
struct Token {
    id: ObjectId,
    amount: u64,
}
impl VmTypeNames for Token {
    fn type_names() -> &'static [(&'static str, &'static str)] {
        &[(
            "ObjectId",
            "@0x0000000000000000000000000000000000000000000000000000000000000001::ObjectId",
        )]
    }
}

#[test]
fn struct_from_rust_translates_nested_type_name() {
    let addr = [0xAAu8; 32];
    let token = Token {
        id: ObjectId { inner: addr },
        amount: 42,
    };
    let value = struct_from_rust(&token).unwrap();

    assert_eq!(
        value,
        Value::Struct {
            type_name: "Token".to_string(),
            fields: vec![
                (
                    "id".to_string(),
                    Value::Struct {
                        type_name: "@0x0000000000000000000000000000000000000000000000000000000000000001::ObjectId".to_string(),
                        fields: vec![("inner".to_string(), Value::Address(addr.into()))],
                    },
                ),
                ("amount".to_string(), Value::U64(42)),
            ],
        }
    );
}

#[test]
fn struct_from_rust_without_mapping_keeps_local_name() {
    // Same Token struct but accessed via a type with no translation — verifies
    // that the mapping only applies when VmTypeNames is implemented.
    #[derive(Serialize)]
    struct PlainToken {
        id: ObjectId,
        amount: u64,
    }
    impl VmTypeNames for PlainToken {}
    let addr = [0xBBu8; 32];
    let token = PlainToken {
        id: ObjectId { inner: addr },
        amount: 7,
    };
    let value = struct_from_rust(&token).unwrap();

    // id field type name is the local "ObjectId", not translated
    let Value::Struct { ref fields, .. } = value else {
        panic!("expected Object")
    };
    let Value::Struct { ref type_name, .. } = fields[0].1 else {
        panic!("expected Struct for id")
    };
    assert_eq!(type_name, "ObjectId");
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
fn round_trip_struct() {
    let original = Point { x: 10, y: 20 };
    let value = struct_from_rust(&original).unwrap();
    assert_eq!(value_to_rust::<Point>(&value).unwrap(), original);
}

#[test]
fn struct_from_rust_accepts_any_first_field() {
    // Layout constraints (id: address first) are adapter-level, not enforced here.
    #[derive(Serialize)]
    struct AnyObject {
        balance: u64,
    }
    impl VmTypeNames for AnyObject {}
    let v = AnyObject { balance: 10 };
    assert!(matches!(
        struct_from_rust(&v).unwrap(),
        meow_vm_types::types::Value::Struct { .. }
    ));
}

#[test]
fn struct_from_rust_accepts_empty_struct() {
    // No structural constraints enforced at this level.
    #[derive(Serialize)]
    struct Empty {}
    impl VmTypeNames for Empty {}
    let v = Empty {};
    assert!(matches!(
        struct_from_rust(&v).unwrap(),
        meow_vm_types::types::Value::Struct { .. }
    ));
}

#[test]
fn unsupported_type_returns_error() {
    #[derive(Serialize)]
    struct WithSeq {
        items: Vec<u64>,
    }
    impl VmTypeNames for WithSeq {}
    let v = WithSeq {
        items: vec![1, 2, 3],
    };
    assert!(matches!(
        struct_from_rust(&v).unwrap_err(),
        ConversionError::UnsupportedType(ref msg) if msg == "seq"
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
    impl VmTypeNames for WithFloat {}
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
    impl VmTypeNames for WithOption {}
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
    impl VmTypeNames for WithSmallArray {}
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
