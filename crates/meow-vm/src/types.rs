use serde::{Deserialize, Serialize};

/// All types supported by the VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    Bool,
    U64,
    /// An address — a 32-byte array, freely copyable.
    Address,
    /// A user-defined struct referenced by name.
    Struct(String),
    /// A user-defined object referenced by name.
    /// Objects must have `id: address` as their first field.
    Object(String),
}

impl Type {
    pub fn from_name(s: &str) -> Self {
        match s {
            "bool" => Self::Bool,
            "u64" => Self::U64,
            "address" => Self::Address,
            name => Self::Struct(name.to_string()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Bool => "bool",
            Self::U64 => "u64",
            Self::Address => "address",
            Self::Struct(n) | Self::Object(n) => n,
        }
    }

    /// Returns true if this type is a primitive (freely copyable).
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Bool | Self::U64 | Self::Address)
    }

    /// Returns true if this type can be used as a struct/object field type.
    /// Only primitives are allowed as field types.
    pub fn is_valid_field_type(&self) -> bool {
        self.is_primitive()
    }
}

/// A runtime value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    U64(u64),
    /// An address (32-byte array), freely copyable.
    Address([u8; 32]),
    /// A string literal — only used as arguments to native functions (e.g. meow_vm_abort).
    Str(String),
    /// Returned by void functions to keep the stack balanced.
    Void,
    /// A user-defined struct value. Freely copyable (value semantics).
    Struct {
        type_name: String,
        /// Fields in struct-definition order.
        fields: Vec<(String, Value)>,
    },
    /// A user-defined object value. Move semantics — Load consumes the slot.
    /// The first field must always be `id: Address`.
    Object {
        type_name: String,
        /// Fields in struct-definition order (first field is always `id: address`).
        fields: Vec<(String, Value)>,
    },
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Self::Bool(_) => "bool",
            Self::U64(_) => "u64",
            Self::Address(_) => "address",
            Self::Str(_) => "str",
            Self::Void => "void",
            Self::Struct { type_name, .. } | Self::Object { type_name, .. } => type_name,
        }
    }

    /// Returns the `u64` representation for arithmetic.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::U64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_address(&self) -> Option<[u8; 32]> {
        match self {
            Self::Address(a) => Some(*a),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns the `id` field value from an Object.
    pub fn object_id(&self) -> Option<[u8; 32]> {
        match self {
            Self::Object { fields, .. } => fields
                .iter()
                .find(|(name, _)| name == "id")
                .and_then(|(_, v)| v.as_address()),
            _ => None,
        }
    }

    /// Returns true if this is an Object value.
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object { .. })
    }

    /// Returns true if this value uses move semantics (only Object).
    pub fn uses_move_semantics(&self) -> bool {
        self.is_object()
    }
}

/// Schema of a user-defined struct or object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    /// True if declared with the `object` keyword.
    /// Object structs must have `id: address` as their first field.
    pub is_object: bool,
}

impl StructDef {
    /// Return the index of `field_name` in this struct, if it exists.
    pub fn field_index(&self, field_name: &str) -> Option<usize> {
        self.fields.iter().position(|(n, _)| n == field_name)
    }
}
