//! Error type for adapter-level bytecode verification.

#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum BytecodeVerifierError {
    #[error(
        "in '{function}' at pc {pc}: object '{object}' constructed with non-fresh id (id must originate directly from meow_vm_fresh_id)"
    )]
    ObjectIdNotFresh {
        function: String,
        pc: usize,
        object: String,
    },

    #[error(
        "struct '{struct_name}' field '{field_name}' has type '{object_type}' which is an object type — objects cannot be nested as struct fields"
    )]
    ObjectAsFieldType {
        struct_name: String,
        field_name: String,
        object_type: String,
    },
}
