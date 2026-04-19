use serde::{Deserialize, Serialize};

use crate::address::Address;

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
    /// A user-defined object referenced by name.
    /// Objects must have `id: address` as their first field.
    Object(String),
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
            Self::Struct(n) | Self::Object(n) => n.clone(),
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

    /// Returns true if this type can be used as a struct/object field type.
    /// Primitives and (non-object) structs are allowed; objects and tuples are not.
    pub fn is_valid_field_type(&self) -> bool {
        self.is_primitive() || matches!(self, Self::Struct(_))
    }
}

/// Returns true if `ty` is, or contains, an object type.
///
/// `structs` is used to resolve `Type::Struct(name)` — the compiler emits
/// `Type::Struct` for all named types in function signatures, so checking
/// whether the name refers to an `is_object` definition requires the
/// struct list from the declaring module.
pub fn type_contains_object(ty: &Type, structs: &[StructDef]) -> bool {
    match ty {
        Type::Object(_) => true,
        Type::Struct(name) => structs.iter().any(|s| s.name == *name && s.is_object),
        Type::Tuple(types) => types.iter().any(|t| type_contains_object(t, structs)),
        _ => false,
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
    /// A tuple of values — produced by `MakeTuple`, consumed by `UnpackTuple`.
    /// Uses move semantics if any element is an Object.
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
            Self::Struct { type_name, .. } | Self::Object { type_name, .. } => type_name,
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

    /// Returns the `id` field value from an Object.
    pub fn object_id(&self) -> Option<Address> {
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

    /// Returns true if this value uses move semantics.
    /// Objects always use move semantics. Tuples use move semantics if any element does.
    pub fn uses_move_semantics(&self) -> bool {
        match self {
            Self::Object { .. } => true,
            Self::Tuple(values) => values.iter().any(|v| v.uses_move_semantics()),
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
            Self::Struct { type_name, fields } | Self::Object { type_name, fields } => {
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

/// A field in a struct or object definition.
/// All fields are private — only accessible within the module that declares the type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    /// The field name.
    pub name: String,
    /// The field type.
    pub ty: Type,
}

/// Schema of a user-defined struct or object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    /// The struct or object name.
    pub name: String,
    /// Fields in declaration order.
    pub fields: Vec<FieldDef>,
    /// True if declared with the `object` keyword.
    /// Object structs must have `id: address` as their first field.
    pub is_object: bool,
    /// True if this type is accessible from modules other than the one that declared it.
    pub is_public: bool,
}

impl StructDef {
    /// Returns the index of `field_name` in this struct, or `None` if not found.
    pub fn field_index(&self, field_name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f.name == field_name)
    }
}
