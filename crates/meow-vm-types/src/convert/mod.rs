//! BCS-based conversion between [`Value`] and Rust types.
//!
//! # `value_to_rust` — Value → Rust
//!
//! Serializes a `Value` with BCS-compatible byte layout, then deserializes as `T`.
//!
//! # `struct_from_rust` / `object_from_rust` — Rust → Value
//!
//! Uses a custom `serde::Serializer` to convert a `T: Serialize` directly into
//! a `Value` without any template or BCS round-trip. Field names are preserved.
//!
//! ```rust
//! use meow_vm_types::types::Value;
//! use meow_vm_types::convert::{value_to_rust, struct_from_rust};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct Point { x: u64, y: u64 }
//!
//! let point = Point { x: 3, y: 7 };
//! let value = struct_from_rust(&point).unwrap();
//! assert_eq!(value_to_rust::<Point>(&value).unwrap(), point);
//! ```

pub mod error;

use serde::{
    Serialize,
    de::DeserializeOwned,
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
};

use crate::{convert::error::ConversionError, types::Value};

/// An error that can occur during VM execution.
pub type Result<T> = std::result::Result<T, ConversionError>;

/// Deserialize a [`Value`] into a Rust type `T`.
///
/// Serializes the value to BCS bytes first, then deserializes as `T`.
/// `T` must derive or implement `serde::Deserialize` with fields in the
/// same order as the `Value`'s struct/object fields.
pub fn value_to_rust<T: DeserializeOwned>(value: &Value) -> Result<T> {
    let bytes = bcs::to_bytes(&ValueToRust(value))?;
    Ok(bcs::from_bytes(&bytes)?)
}

/// Convert a Rust type into a [`Value::Struct`].
///
/// Uses serde's data model to capture the struct name and field names directly.
/// `T` must derive or implement `serde::Serialize`.
///
/// Only structs with named fields are supported. For object values (with `id: [u8; 32]`
/// as the first field), use [`object_from_rust`].
pub fn struct_from_rust<T: Serialize>(val: &T) -> Result<Value> {
    val.serialize(ValueSerializer { is_object: false })
}

/// Convert a Rust type into a [`Value::Object`].
///
/// Same as [`struct_from_rust`] but produces `Value::Object` instead of `Value::Struct`.
pub fn object_from_rust<T: Serialize>(val: &T) -> Result<Value> {
    val.serialize(ValueSerializer { is_object: true })
}

//
// ─── Custom serializers ───
//

/// Newtype wrapper that serializes a [`Value`] as its underlying data,
/// without the enum discriminant. The output is identical to what the
/// equivalent plain Rust type would produce under BCS.
///
/// - `Value::Bool(b)`        → same bytes as `bool`
/// - `Value::U64(n)`         → same bytes as `u64`
/// - `Value::Address(a)`     → same bytes as `[u8; 32]`
/// - `Value::Str(s)`         → same bytes as `String`
/// - `Value::Struct/Object`  → fields serialized in order, same as a Rust struct
struct ValueToRust<'a>(&'a Value);

impl Serialize for ValueToRust<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self.0 {
            Value::Bool(b) => b.serialize(s),
            Value::U64(n) => n.serialize(s),
            Value::Address(a) => a.serialize(s),
            Value::Str(str) => str.serialize(s),
            Value::Void => {
                use serde::ser::Error;
                Err(S::Error::custom("void value cannot be serialized"))
            }
            Value::Tuple(_) => {
                use serde::ser::Error;
                Err(S::Error::custom("tuple value cannot be serialized to BCS"))
            }
            Value::Struct { fields, .. } | Value::Object { fields, .. } => {
                // BCS structs are just their fields concatenated in order.
                // serialize_tuple produces the same layout as serialize_struct in BCS.
                let mut tuple = s.serialize_tuple(fields.len())?;
                for (_, val) in fields {
                    tuple.serialize_element(&ValueToRust(val))?;
                }
                tuple.end()
            }
        }
    }
}

struct ValueSerializer {
    is_object: bool,
}

type SerResult = std::result::Result<Value, ConversionError>;

impl serde::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = ConversionError;

    type SerializeSeq = Impossible;
    type SerializeTuple = TupleSerializer;
    type SerializeTupleStruct = Impossible;
    type SerializeTupleVariant = Impossible;
    type SerializeMap = Impossible;
    type SerializeStruct = StructSerializer;
    type SerializeStructVariant = Impossible;

    fn serialize_bool(self, v: bool) -> SerResult {
        Ok(Value::Bool(v))
    }
    fn serialize_u8(self, v: u8) -> SerResult {
        Ok(Value::U64(v as u64))
    }
    fn serialize_u16(self, v: u16) -> SerResult {
        Ok(Value::U64(v as u64))
    }
    fn serialize_u32(self, v: u32) -> SerResult {
        Ok(Value::U64(v as u64))
    }
    fn serialize_u64(self, v: u64) -> SerResult {
        Ok(Value::U64(v))
    }
    fn serialize_str(self, v: &str) -> SerResult {
        Ok(Value::Str(v.to_string()))
    }
    fn serialize_bytes(self, v: &[u8]) -> SerResult {
        Err(ConversionError::UnsupportedType(format!(
            "byte slice of length {}; use [u8; 32] for addresses",
            v.len()
        )))
    }
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<StructSerializer> {
        Ok(StructSerializer {
            type_name: name.to_string(),
            fields: Vec::with_capacity(len),
            is_object: self.is_object,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<TupleSerializer> {
        Ok(TupleSerializer {
            elements: Vec::with_capacity(len),
        })
    }

    // Remaining required methods — all unsupported.
    fn serialize_i8(self, _: i8) -> SerResult {
        Err(ConversionError::UnsupportedType("i8".into()))
    }
    fn serialize_i16(self, _: i16) -> SerResult {
        Err(ConversionError::UnsupportedType("i16".into()))
    }
    fn serialize_i32(self, _: i32) -> SerResult {
        Err(ConversionError::UnsupportedType("i32".into()))
    }
    fn serialize_i64(self, _: i64) -> SerResult {
        Err(ConversionError::UnsupportedType("i64".into()))
    }
    fn serialize_f32(self, _: f32) -> SerResult {
        Err(ConversionError::UnsupportedType("f32".into()))
    }
    fn serialize_f64(self, _: f64) -> SerResult {
        Err(ConversionError::UnsupportedType("f64".into()))
    }
    fn serialize_char(self, _: char) -> SerResult {
        Err(ConversionError::UnsupportedType("char".into()))
    }
    fn serialize_none(self) -> SerResult {
        Err(ConversionError::UnsupportedType("Option::None".into()))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> SerResult {
        Err(ConversionError::UnsupportedType("Option::Some".into()))
    }
    fn serialize_unit(self) -> SerResult {
        Err(ConversionError::UnsupportedType("unit".into()))
    }
    fn serialize_unit_struct(self, _: &'static str) -> SerResult {
        Err(ConversionError::UnsupportedType("unit struct".into()))
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> SerResult {
        Err(ConversionError::UnsupportedType("unit variant".into()))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> SerResult {
        // Transparently unwrap newtypes (e.g. `Address([u8; 32])` → `Value::Address`).
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> SerResult {
        Err(ConversionError::UnsupportedType("newtype variant".into()))
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Impossible> {
        Err(ConversionError::UnsupportedType("seq".into()))
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Impossible> {
        Err(ConversionError::UnsupportedType("tuple struct".into()))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Impossible> {
        Err(ConversionError::UnsupportedType("tuple variant".into()))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Impossible> {
        Err(ConversionError::UnsupportedType("map".into()))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Impossible> {
        Err(ConversionError::UnsupportedType("struct variant".into()))
    }
}

//
// ─── StructSerializer ───
//

struct StructSerializer {
    type_name: String,
    fields: Vec<(String, Value)>,
    is_object: bool,
}

impl SerializeStruct for StructSerializer {
    type Ok = Value;
    type Error = ConversionError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        let v = value.serialize(ValueSerializer { is_object: false })?;
        self.fields.push((key.to_string(), v));
        Ok(())
    }

    fn end(self) -> SerResult {
        if self.is_object {
            match self.fields.first() {
                Some((_, Value::Address(_))) => {}
                Some((name, _)) => {
                    return Err(ConversionError::UnsupportedType(format!(
                        "object first field '{}' must be Address ([u8; 32])",
                        name
                    )));
                }
                None => {
                    return Err(ConversionError::UnsupportedType(
                        "object must have at least one field (id: [u8; 32])".into(),
                    ));
                }
            }
            Ok(Value::Object {
                type_name: self.type_name,
                fields: self.fields,
            })
        } else {
            Ok(Value::Struct {
                type_name: self.type_name,
                fields: self.fields,
            })
        }
    }
}

//
// ─── TupleSerializer ───
//
// Used for [u8; N] arrays: serde serializes them as N-element tuples of u8.
// We detect the 32-element case and produce Value::Address.

struct TupleSerializer {
    elements: Vec<Value>,
}

impl SerializeTuple for TupleSerializer {
    type Ok = Value;
    type Error = ConversionError;

    fn serialize_element<T: ?Sized + Serialize>(
        &mut self,
        value: &T,
    ) -> std::result::Result<(), Self::Error> {
        let v = value.serialize(ValueSerializer { is_object: false })?;
        self.elements.push(v);
        Ok(())
    }

    fn end(self) -> SerResult {
        if self.elements.len() == 32 {
            let mut addr = [0u8; 32];
            for (i, v) in self.elements.into_iter().enumerate() {
                match v {
                    Value::U64(b) if b <= 0xFF => addr[i] = b as u8,
                    other => {
                        return Err(ConversionError::UnsupportedType(format!(
                            "expected u8 in address tuple, got {:?}",
                            other
                        )));
                    }
                }
            }
            Ok(Value::Address(addr.into()))
        } else {
            Err(ConversionError::UnsupportedType(format!(
                "tuple of length {}; only [u8; 32] tuples are supported",
                self.elements.len()
            )))
        }
    }
}

//
// ─── Impossible ───
//
// Placeholder for serializer associated types that are never constructed.

struct Impossible;

impl SerializeSeq for Impossible {
    type Ok = Value;
    type Error = ConversionError;
    fn serialize_element<T: ?Sized + Serialize>(
        &mut self,
        _: &T,
    ) -> std::result::Result<(), Self::Error> {
        unreachable!()
    }
    fn end(self) -> SerResult {
        unreachable!()
    }
}
impl SerializeTupleStruct for Impossible {
    type Ok = Value;
    type Error = ConversionError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _: &T,
    ) -> std::result::Result<(), Self::Error> {
        unreachable!()
    }
    fn end(self) -> SerResult {
        unreachable!()
    }
}
impl SerializeTupleVariant for Impossible {
    type Ok = Value;
    type Error = ConversionError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _: &T,
    ) -> std::result::Result<(), Self::Error> {
        unreachable!()
    }
    fn end(self) -> SerResult {
        unreachable!()
    }
}
impl SerializeMap for Impossible {
    type Ok = Value;
    type Error = ConversionError;
    fn serialize_key<T: ?Sized + Serialize>(
        &mut self,
        _: &T,
    ) -> std::result::Result<(), Self::Error> {
        unreachable!()
    }
    fn serialize_value<T: ?Sized + Serialize>(
        &mut self,
        _: &T,
    ) -> std::result::Result<(), Self::Error> {
        unreachable!()
    }
    fn end(self) -> SerResult {
        unreachable!()
    }
}
impl SerializeStructVariant for Impossible {
    type Ok = Value;
    type Error = ConversionError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _: &'static str,
        _: &T,
    ) -> std::result::Result<(), Self::Error> {
        unreachable!()
    }
    fn end(self) -> SerResult {
        unreachable!()
    }
}
