#![allow(dead_code)]

use meow_vm_compiler::{Compiler, Result};
use meow_vm_types::{address::Address, config::CompilerConfig, module::Module};

/// Compile a source snippet.
pub fn compile(source: &str) -> Result<Module> {
    Compiler::compile(source, &[], &[], CompilerConfig::default())
}

/// Compile a source snippet with dependency modules.
pub fn compile_with_deps(source: &str, deps: &[(Address, &Module)]) -> Result<Module> {
    Compiler::compile(source, deps, &[], CompilerConfig::default())
}
