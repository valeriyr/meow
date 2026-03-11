/// An error related to the builder.
#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    #[error("compile error: {0}")]
    CompileError(#[from] meow_vm::compiler::error::CompilerError),
}
