use meow_vm_types::{
    convert,
    types::{StructDef, Type, Value},
};
use serde::{Deserialize, Serialize};

use crate::{address::Address, system_framework::utils};

/// The meow object module address is a reserved address where the meow object module is deployed.
pub const MEOW_OBJECT_MODULE_ADDRESS: Address = utils::builtin_address(0x01);
/// The meow object module name.
pub const MEOW_OBJECT_MODULE_NAME: &str = "meow_object";
/// The meow object id struct name.
pub const MEOW_OBJECT_ID_OBJECT_NAME: &str = "Id";

/// The MEOW object module file path.
pub const MEOW_OBJECT_MODULE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../meow-framework/modules/meow_object.meow"
);

/// The name of the object identity field required as the first field of every object struct.
pub const MEOW_OBJECT_ID_FIELD_NAME: &str = "id";

/// The address-qualified type name for `meow_object::Id` as it appears in compiled bytecode.
///
/// After compilation, dep-qualified names (`meow_object::Id`) are translated to
/// address-qualified form (`@<address>::Id`). This constant is the canonical name
/// used in native signatures and object conversion.
pub const MEOW_OBJECT_ID_BYTECODE_TYPE_NAME: &str =
    "@0x0000000000000000000000000000000000000000000000000000000000000001::Id";

/// The MeowObjectId struct represents a unique object id.
#[derive(Serialize, Deserialize)]
pub struct MeowObjectId {
    /// The unique on-chain object identifier represented as an address.
    inner: Address,
}

impl MeowObjectId {
    /// Creates a new MeowObjectId with the given id.
    pub fn new(id: Address) -> Self {
        Self { inner: id }
    }

    /// Returns the inner address the MeowObjectId.
    pub fn inner(&self) -> &Address {
        &self.inner
    }

    /// Convert to fully qualified VM [`Value::Struct`] representation used in bytecode.
    pub fn to_qualified_vm_value(&self) -> Value {
        Value::Struct {
            type_name: MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string(),
            fields: vec![("inner".to_string(), Value::Address(self.inner.into()))],
        }
    }
}

impl From<Address> for MeowObjectId {
    fn from(address: Address) -> Self {
        Self::new(address)
    }
}

impl From<MeowObjectId> for Address {
    fn from(meow_object_id: MeowObjectId) -> Self {
        meow_object_id.inner
    }
}

/// Returns `true` if `s` is an object struct — i.e. its first field is `id: meow_object::Id`.
pub fn is_object_struct(s: &StructDef) -> bool {
    let id_type = Type::Struct(MEOW_OBJECT_ID_BYTECODE_TYPE_NAME.to_string());
    s.fields
        .first()
        .map(|f| f.name == MEOW_OBJECT_ID_FIELD_NAME && f.ty == id_type)
        .unwrap_or(false)
}

/// Returns the address from the `id` field of an Object.
///
/// Handles one format:
/// - `id: meow_object::Id { inner: address }` — production format introduced with the
///   meow_object module; address extracted from the `inner` field.
pub fn object_id(value: &Value) -> Option<Address> {
    match value {
        Value::Struct { fields, .. } => fields
            .iter()
            .find(|(name, _)| name == MEOW_OBJECT_ID_FIELD_NAME)
            .and_then(|(_, v)| {
                let id = convert::value_to_rust::<MeowObjectId>(v);

                match id {
                    Ok(id) => Some(*id.inner()),
                    Err(_) => None,
                }
            }),
        _ => None,
    }
}
