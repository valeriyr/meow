//! Compiles Meow source into a bytecode module ready for on-chain publishing.
//!
//! Enforces size limits on both the raw source and the serialized output so that publishing
//! transactions stay within the bounds validators will accept.

pub mod error;

use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

use meow_types::{
    address::Address,
    config::{self, MAX_BCS_SERIALIZED_MODULE_SIZE},
};
use meow_vm_compiler::Compiler;

use crate::{Module, builder::error::BuilderError, natives};

/// The result type related to the builder.
pub type Result<T> = std::result::Result<T, BuilderError>;

/// Maximum byte length of source code passed to the compiler.
pub const MAX_SOURCE_SIZE: usize = 64 * 1024; // 64 KiB

/// Extract declared dependency information from source without full compilation.
///
/// Returns `(name, alias, address)` triples in source order, where `name` is the
/// actual module name and `alias` is the local name used in source (`name` when
/// no `as` clause is present). No dep modules need to be provided — this is
/// intended for callers that need to know which modules to fetch before calling
/// [`build`].
///
/// # Note on double parsing
/// Callers that follow up with [`build`] will cause the source to be parsed
/// twice: once here and once inside `build`. This is a known limitation —
/// dependency declarations are embedded in the source rather than a separate
/// manifest, so there is no way to carry over the parse result between the
/// two calls without exposing internal AST types. The overhead is bounded by
/// [`MAX_SOURCE_SIZE`] and is negligible compared to the network round-trips
/// needed to fetch dep modules.
pub fn extract_module_deps(source: &str) -> Result<Vec<(String, Option<String>, Address)>> {
    if source.len() > MAX_SOURCE_SIZE {
        return Err(BuilderError::SourceTooLarge {
            size: source.len(),
            limit: MAX_SOURCE_SIZE,
        });
    }

    Ok(Compiler::extract_deps(source)?
        .into_iter()
        .map(|(name, alias, addr)| (name, alias, addr.into()))
        .collect())
}

/// Build a module from source with pre-loaded dependency modules.
pub fn build(source: &str, deps: &[(Address, &Module)]) -> Result<Module> {
    if source.len() > MAX_SOURCE_SIZE {
        return Err(BuilderError::SourceTooLarge {
            size: source.len(),
            limit: MAX_SOURCE_SIZE,
        });
    }

    compile(source, deps)
}

/// Build a module from a source file with pre-loaded dependency modules.
pub fn build_from_file<P: AsRef<Path>>(
    file_path: P,
    deps: &[(Address, &Module)],
) -> Result<Module> {
    let source = read_source_file(file_path)?;

    build(&source, deps)
}

/// Read source from a file, validating the size limit.
pub fn read_source_file<P: AsRef<Path>>(file_path: P) -> Result<String> {
    let file_path = file_path.as_ref();

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

    Ok(source)
}

/// Compile source with pre-loaded dependency modules, returning the compiled module.
fn compile(source: &str, deps: &[(Address, &Module)]) -> Result<Module> {
    let deps = deps
        .iter()
        .map(|(addr, m)| ((*addr).into(), *m))
        .collect::<Vec<_>>();

    let module = Compiler::compile(
        source,
        &deps,
        &natives::adapter_native_sigs_for_compiler(),
        config::compiler_config(),
    )?;

    let module_size = bcs::serialized_size(&module).expect("module serialization is infallible");

    let max_module_size = MAX_BCS_SERIALIZED_MODULE_SIZE;
    if module_size > max_module_size {
        return Err(BuilderError::ModuleTooLarge {
            size: module_size,
            limit: max_module_size,
        });
    }

    Ok(module)
}
