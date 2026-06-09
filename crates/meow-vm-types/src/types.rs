//! Shared type and value definitions used across the VM, compiler, verifier, and adapter.
//!
//! Keeping a single definition avoids translation layers between crates and ensures that
//! values produced by the compiler can be consumed by the VM without conversion.

use serde::{Deserialize, Serialize};

use crate::{address::Address, module_ref};

/// All types supported by the VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    Bool,
    U64,
    /// An address — a 32-byte array, freely copyable.
    Address,
    /// A UTF-8 string value. Freely copyable.
    Str,
    /// A user-defined struct referenced by name.
    Struct(String),
    /// A tuple of types — used for multi-value function returns.
    Tuple(Vec<Type>),
}

impl Type {
    /// Converts a type name string to a [`Type`].
    pub fn from_name(s: &str) -> Self {
        match s {
            "bool" => Self::Bool,
            "u64" => Self::U64,
            "address" => Self::Address,
            "string" => Self::Str,
            name => Self::Struct(name.to_string()),
        }
    }

    /// Returns the canonical name string for this type.
    pub fn name(&self) -> String {
        match self {
            Self::Bool => "bool".to_string(),
            Self::U64 => "u64".to_string(),
            Self::Address => "address".to_string(),
            Self::Str => "string".to_string(),
            Self::Struct(n) => n.clone(),
            Self::Tuple(types) => {
                let inner: Vec<String> = types.iter().map(|t| t.name()).collect();
                format!("({})", inner.join(", "))
            }
        }
    }

    /// Returns true if this type is a primitive (freely copyable).
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Bool | Self::U64 | Self::Address | Self::Str)
    }

    /// Returns true if this type can be used as a struct field type.
    /// Primitives and structs are allowed; tuples are not.
    pub fn is_valid_field_type(&self) -> bool {
        self.is_primitive() || matches!(self, Self::Struct(_))
    }

    /// Returns `true` if this type has move semantics — structs always do; tuples do if any
    /// element does. Primitives are always `false`.
    pub fn is_linear(&self) -> bool {
        match self {
            Self::Struct(_) => true,
            Self::Tuple(types) => types.iter().any(|t| t.is_linear()),
            _ => false,
        }
    }

    /// Returns `true` if this is a struct type declared in a different module.
    pub fn is_cross_module(&self) -> bool {
        matches!(self, Self::Struct(name) if module_ref::is_qualified(name))
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A runtime value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    U64(u64),
    /// An address (32-byte value), freely copyable.
    Address(Address),
    /// A UTF-8 string value.
    Str(String),
    /// Returned by void functions to keep the stack balanced.
    Void,
    /// A user-defined struct value. Move semantics — Load consumes the slot.
    Struct {
        type_name: String,
        /// Fields in struct-definition order.
        fields: Vec<(String, Value)>,
    },
    /// A tuple of values — produced by `MakeTuple`, consumed by `UnpackTuple`.
    /// Uses move semantics if any element is a Struct.
    Tuple(Vec<Value>),
}

impl Value {
    /// Returns the type name of this value.
    pub fn type_name(&self) -> &str {
        match self {
            Self::Bool(_) => "bool",
            Self::U64(_) => "u64",
            Self::Address(_) => "address",
            Self::Str(_) => "str",
            Self::Void => "void",
            Self::Struct { type_name, .. } => type_name,
            Self::Tuple(_) => "tuple",
        }
    }

    /// Returns the `u64` representation for arithmetic.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the `bool` if this is a `Bool` value, or `None` otherwise.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the address if this is an `Address` value, or `None` otherwise.
    pub fn as_address(&self) -> Option<Address> {
        match self {
            Self::Address(a) => Some(*a),
            _ => None,
        }
    }

    /// Returns the string slice if this is a `Str` value, or `None` otherwise.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Consumes this value and returns the inner `String` if it is a `Str` value, or `None` otherwise.
    pub fn into_str(self) -> Option<String> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Returns a slice of the inner elements if this is a `Tuple`, or `None` otherwise.
    pub fn as_tuple(&self) -> Option<&[Value]> {
        match self {
            Self::Tuple(elements) => Some(elements),
            _ => None,
        }
    }

    /// Returns `true` if this is a `Void` value.
    pub fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    /// Returns a reference to the named field if this is a `Struct`, or `None` otherwise.
    pub fn field(&self, name: &str) -> Option<&Value> {
        match self {
            Self::Struct { fields, .. } => fields.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Returns the `u64` value of the named struct field, or `None` if the field is absent
    /// or is not a `U64`.
    pub fn field_u64(&self, name: &str) -> Option<u64> {
        self.field(name)?.as_u64()
    }

    /// Returns the string slice of the named struct field, or `None` if the field is absent
    /// or is not a `Str`.
    pub fn field_str(&self, name: &str) -> Option<&str> {
        self.field(name)?.as_str()
    }

    /// Returns the `bool` of the named struct field, or `None` if the field is absent
    /// or is not a `Bool`.
    pub fn field_bool(&self, name: &str) -> Option<bool> {
        self.field(name)?.as_bool()
    }

    /// Returns the address of the named struct field, or `None` if the field is absent
    /// or is not an `Address`.
    pub fn field_address(&self, name: &str) -> Option<Address> {
        self.field(name)?.as_address()
    }

    /// Returns true if this value is linear (has move semantics).
    /// Structs are always linear. Tuples are linear if any element is.
    pub fn is_linear(&self) -> bool {
        match self {
            Self::Struct { .. } => true,
            Self::Tuple(values) => values.iter().any(|v| v.is_linear()),
            _ => false,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "{}", v),
            Self::U64(v) => write!(f, "{}", v),
            Self::Address(a) => write!(f, "{}", a),
            Self::Str(s) => write!(f, "\"{}\"", s),
            Self::Void => write!(f, "void"),
            Self::Struct { type_name, fields } => {
                let fields_str = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, value))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{} {{ {} }}", type_name, fields_str)
            }
            Self::Tuple(values) => {
                let inner = values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "({})", inner)
            }
        }
    }
}

/// A field in a struct definition.
/// All fields are private — only accessible within the module that declares the type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    /// The field name.
    pub name: String,
    /// The field type.
    pub ty: Type,
}

/// Schema of a user-defined struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    /// The struct name.
    pub name: String,
    /// Fields in declaration order.
    pub fields: Vec<FieldDef>,
    /// True if this type is accessible from modules other than the one that declared it.
    pub is_public: bool,
}

impl StructDef {
    /// Returns the index of `field_name` in this struct, or `None` if not found.
    pub fn field_index(&self, field_name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == field_name)
    }
}
