//! Conversions between on-chain `Object` and its VM-level field representation.

pub mod error;

use std::collections::BTreeMap;

use meow_vm_types::{module_ref, types::Value};

use crate::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_conversion::error::ObjectConversionError, object_decl_ref::ObjectDeclRef,
        object_owner::ObjectOwner, object_type::ObjectType, object_version::ObjectVersion,
    },
    system_framework::meow_object::{self, MEOW_OBJECT_ID_FIELD_NAME, MeowObjectId},
};

/// An error that can occur during conversion.
pub type Result<T> = std::result::Result<T, ObjectConversionError>;

/// Convert a meow-types Object to a meow-vm Value::Struct.
///
/// The object content is BCS-serialized `Vec<(String, Value)>` without the `id`
/// field (which lives in `Object::address`). This function re-injects `id` as
/// the first field and qualifies the type name as `@<module>::<name>` so that
/// the VM can identify which module the struct belongs to.
pub fn object_to_vm_object_value(obj: &Object) -> Result<Value> {
    let decl = match obj.type_() {
        ObjectType::Object(decl) => Ok(decl),
        _ => Err(ObjectConversionError::InvalidObjectType),
    }?;
    let type_name = module_ref::qualify(&(*decl.module()).into(), decl.name().as_ref());
    let mut fields: Vec<(String, Value)> = bcs::from_bytes(obj.content())
        .map_err(|e| ObjectConversionError::InvalidContent(e.to_string()))?;
    let id = MeowObjectId::from(*obj.address()).into();
    fields.insert(0, (MEOW_OBJECT_ID_FIELD_NAME.to_string(), id));
    Ok(Value::Struct { type_name, fields })
}

/// Convert a meow-vm Value::Struct back to a meow-types Object.
///
/// The `id` field is extracted from the VM value and stored in `Object::address`;
/// it is not written into the content to avoid duplication.
///
/// The `ObjectType` is derived from the qualified type name in the VM value
/// (format: `@<hex_address>::<StructName>`). The VM is responsible for producing
/// qualified type names via the `NewStruct` instruction.
pub fn vm_object_value_to_object(
    val: &Value,
    owner: ObjectOwner,
    tx_digest: Digest,
    version: ObjectVersion,
) -> Result<Object> {
    let (type_name, fields) = match val {
        Value::Struct { type_name, fields } => (type_name.clone(), fields.clone()),
        _ => return Err(ObjectConversionError::InvalidVMValueType),
    };

    let (vm_module_addr, struct_name) =
        module_ref::parse_module_ref(&type_name).ok_or(ObjectConversionError::InvalidTypeName)?;
    let object_type = ObjectType::Object(ObjectDeclRef::new(
        Address::from(vm_module_addr),
        Identifier::new(struct_name)
            .map_err(|e| ObjectConversionError::InvalidIdentifier(e.to_string()))?,
    ));

    let address: Address =
        meow_object::object_address(val).ok_or(ObjectConversionError::MissingIdField)?;

    // Strip `id` — it is stored in Object::address, not in the content.
    let content_fields: Vec<_> = fields
        .into_iter()
        .filter(|(n, _)| n != MEOW_OBJECT_ID_FIELD_NAME)
        .collect();
    let content = bcs::to_bytes(&content_fields).expect("fields must be serializable");

    Ok(Object::new(
        address,
        owner,
        tx_digest,
        version,
        object_type,
        content,
    ))
}

/// Decode any Object's content into a human-readable key-value map for display.
///
/// All on-chain objects store their fields as BCS-serialized `Vec<(String, Value)>`.
/// Returns `None` for module objects (which contain bytecode, not field data).
/// Keys are ordered alphabetically via `BTreeMap` for consistent output.
pub fn extract_human_readable_content(object: &Object) -> Option<BTreeMap<String, String>> {
    if !matches!(object.type_(), ObjectType::Object(_)) {
        return None;
    }
    let fields: Vec<(String, Value)> = bcs::from_bytes(object.content()).ok()?;
    Some(
        fields
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect(),
    )
}
