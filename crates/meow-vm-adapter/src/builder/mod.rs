pub mod error;

use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use meow_vm_compiler::Compiler;
use meow_vm_types::config::CompilerConfig;

use crate::{Module, builder::error::BuilderError};

/// The result type related to the builder.
pub type Result<T> = std::result::Result<T, BuilderError>;

/// Maximum byte length of source code passed to the compiler.
pub const MAX_SOURCE_SIZE: usize = 64 * 1024; // 64 KiB

/// Build a module from source code.
pub fn build(module_name: &str, source: &str) -> Result<Module> {
    if source.len() > MAX_SOURCE_SIZE {
        return Err(BuilderError::SourceTooLarge {
            size: source.len(),
            limit: MAX_SOURCE_SIZE,
        });
    }

    compile(module_name, source)
}

/// Build a module from a source file.
pub fn build_from_file<P: AsRef<Path>>(file_path: P) -> Result<Module> {
    let file_path = file_path.as_ref();

    let module_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| BuilderError::InvalidFileName(file_path.display().to_string()))?;

    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();

    if file_size > MAX_SOURCE_SIZE as u64 {
        return Err(BuilderError::SourceTooLarge {
            size: file_size as usize,
            limit: MAX_SOURCE_SIZE,
        });
    }

    let mut reader = BufReader::new(file);

    let mut source = String::new();
    reader.read_to_string(&mut source)?;

    compile(module_name, &source)
}

fn compile(module_name: &str, source: &str) -> Result<Module> {
    Ok(Compiler::compile(
        module_name,
        source,
        CompilerConfig::default(),
    )?)
}
