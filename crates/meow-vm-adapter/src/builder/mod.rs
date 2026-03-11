pub mod error;

use meow_vm::{compiler::Compiler, module::Module};

use crate::builder::error::BuilderError;

/// The result type related to the builder.
pub type Result<T> = std::result::Result<T, BuilderError>;

pub fn build(module_name: &str, source: &str) -> Result<Module> {
    Ok(Compiler::compile(module_name, source)?)
}
