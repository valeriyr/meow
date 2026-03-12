pub mod error;

use std::path::Path;

use meow_vm::{compiler::Compiler, module::Module};

use crate::builder::error::BuilderError;

/// The result type related to the builder.
pub type Result<T> = std::result::Result<T, BuilderError>;

pub fn build(module_name: &str, source: &str) -> Result<Module> {
    Ok(Compiler::compile(module_name, source)?)
}

pub fn build_from_file<P: AsRef<Path>>(file_path: P) -> Result<Module> {
    let file_path = file_path.as_ref();

    let module_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| BuilderError::InvalidFileName(file_path.display().to_string()))?;

    let source = std::fs::read_to_string(file_path)?;

    build(module_name, &source)
}
