use meow_vm_types::{
    config::CompilerConfig,
    identifier::{self, RESERVED_FUNCTION_NAMES},
    types::Type,
};

use crate::{Result, ast::AstStruct, error::CompilerError};

pub fn validate_identifier(name: &str, context: &str, config: &CompilerConfig) -> Result<()> {
    if !identifier::is_valid_identifier(name, config) {
        Err(CompilerError::Message(format!(
            "{context}: '{}' is not a valid identifier \
             (must start with a letter or underscore, followed by letters, digits, or underscores; \
             max {} characters)",
            name,
            config.max_identifier_len(),
        )))
    } else {
        Ok(())
    }
}

pub fn validate_function_name(name: &str, config: &CompilerConfig) -> Result<()> {
    validate_identifier(name, "function name", config)?;

    let is_vm_reserved = RESERVED_FUNCTION_NAMES.contains(&name);
    let is_config_reserved = config.reserved_function_names().iter().any(|n| n == name);

    if is_vm_reserved || is_config_reserved {
        return Err(CompilerError::Message(format!(
            "function name '{}' is reserved for a built-in native function",
            name
        )));
    }
    Ok(())
}

pub fn validate_struct_def(def: &AstStruct, config: &CompilerConfig) -> Result<()> {
    let kind = if def.is_object { "object" } else { "struct" };

    validate_identifier(&def.name, &format!("{kind} name"), config)?;

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
        validate_identifier(
            field_name,
            &format!("field in {kind} '{}'", def.name),
            config,
        )?;
        if !ty.is_valid_field_type() {
            return Err(CompilerError::Message(format!(
                "{kind} '{}': field '{field_name}' has non-primitive type '{}' — only bool, u64, address, string are allowed",
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
