use meow_vm_types::{
    config::{self, Config},
    identifier,
    types::Type,
};

use crate::{Result, ast::AstStruct, error::CompilerError};

pub fn validate_identifier(name: &str, context: &str) -> Result<()> {
    if !identifier::is_valid_identifier(name) {
        Err(CompilerError::Message(format!(
            "{context}: '{}' is not a valid identifier \
             (must start with a letter or underscore, followed by letters, digits, or underscores; \
             max {} characters)",
            name,
            config::MAX_IDENTIFIER_LEN,
        )))
    } else {
        Ok(())
    }
}

pub fn validate_struct_def(def: &AstStruct, config: &Config) -> Result<()> {
    let kind = if def.is_object { "object" } else { "struct" };

    validate_identifier(&def.name, &format!("{kind} name"))?;

    let max_fields = config.max_fields();
    if def.fields.len() > max_fields {
        return Err(CompilerError::Message(format!(
            "{kind} '{}': too many fields ({} > limit of {})",
            def.name,
            def.fields.len(),
            max_fields,
        )));
    }

    // Validate field types: only primitives allowed.
    for (field_name, ty) in &def.fields {
        validate_identifier(field_name, &format!("field in {kind} '{}'", def.name))?;
        if !ty.is_valid_field_type() {
            return Err(CompilerError::Message(format!(
                "{kind} '{}': field '{field_name}' has non-primitive type '{}' — only bool, u64, address are allowed",
                def.name,
                ty.name()
            )));
        }
    }

    // Objects must have `id: address` as their first field.
    if def.is_object {
        match def.fields.first() {
            Some((name, Type::Address)) if name == "id" => {}
            _ => {
                return Err(CompilerError::Message(format!(
                    "object '{}': first field must be 'id: address'",
                    def.name
                )));
            }
        }
    }

    Ok(())
}
