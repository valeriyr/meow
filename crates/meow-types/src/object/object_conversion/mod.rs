pub mod error;

use meow_vm_types::types::Value;

use crate::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_conversion::error::ObjectConversionError, object_decl_ref::ObjectDeclRef,
        object_owner::ObjectOwner, object_type::ObjectType, object_version::ObjectVersion,
    },
};

/// An error that can occur during conversion.
pub type Result<T> = std::result::Result<T, ObjectConversionError>;

/// Convert a meow-types Object to a meow-vm Value::Object.
///
/// The object content is BCS-serialized `Vec<(String, Value)>` without the `id`
/// field (which lives in `Object::address`). This function re-injects `id` as
/// the first field so the VM can work with a complete object value.
pub fn object_to_vm_object_value(obj: &Object) -> Result<Value> {
    let type_name = match obj.type_() {
        ObjectType::Object(decl) => Ok(decl.name().as_ref().to_string()),
        _ => Err(ObjectConversionError::InvalidObjectType),
    }?;
    let mut fields: Vec<(String, Value)> =
        bcs::from_bytes(obj.content()).expect("object content must be valid BCS");
    let id_bytes: [u8; 32] = (*obj.address()).into();
    fields.insert(0, ("id".to_string(), Value::Address(id_bytes)));
    Ok(Value::Object { type_name, fields })
}

/// Convert a meow-vm Value::Object back to a meow-types Object.
///
/// The `id` field is extracted from the VM value and stored in `Object::address`;
/// it is not written into the content to avoid duplication.
pub fn vm_object_value_to_object(
    val: &Value,
    owner: ObjectOwner,
    tx_digest: Digest,
    version: ObjectVersion,
    module_addr: &Address,
) -> Result<Object> {
    let (type_name, fields) = match val {
        Value::Object { type_name, fields } => Ok((type_name.clone(), fields.clone())),
        _ => Err(ObjectConversionError::InvalidVMValueType),
    }?;

    let id: Address = val.object_id().expect("Object must have id field").into();
    let identifier =
        Identifier::new(type_name.clone()).expect("type name must be a valid identifier");

    let object_decl_ref = ObjectDeclRef::new(*module_addr, identifier);

    // Strip `id` — it is stored in Object::address, not in the content.
    let content_fields: Vec<_> = fields.into_iter().filter(|(n, _)| n != "id").collect();
    let content = bcs::to_bytes(&content_fields).expect("fields must be serializable");

    Ok(Object::new(
        id,
        owner,
        tx_digest,
        version,
        ObjectType::Object(object_decl_ref),
        content,
    ))
}
