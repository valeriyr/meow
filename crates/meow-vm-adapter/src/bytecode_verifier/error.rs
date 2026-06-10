//! Error type for adapter-level bytecode verification.

#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum BytecodeVerifierError {
    // Object layout
    #[error(
        "struct '{struct_name}' field '{field_name}' has type `meow_object::Id` but is not the first field — an `id: meow_object::Id` field is only allowed as the first field (which makes the struct an object)"
    )]
    IdFieldNotFirst {
        struct_name: String,
        field_name: String,
    },
    #[error(
        "struct '{struct_name}' field '{field_name}' has type '{object_type}' which is an object type — objects cannot be nested as struct fields"
    )]
    ObjectAsFieldType {
        struct_name: String,
        field_name: String,
        object_type: String,
    },

    // ID freshness
    #[error(
        "in '{function}' at pc {pc}: object '{object}' constructed with non-fresh id (id must originate directly from meow_vm_fresh_id)"
    )]
    ObjectIdNotFresh {
        function: String,
        pc: usize,
        object: String,
    },

    // Transfer type
    #[error(
        "in '{function}' at pc {pc}: meow_vm_transfer argument '{struct_name}' is not an on-chain object type — first field must be `id: meow_object::Id`"
    )]
    TransferNonObjectStruct {
        function: String,
        pc: usize,
        struct_name: String,
    },
}
