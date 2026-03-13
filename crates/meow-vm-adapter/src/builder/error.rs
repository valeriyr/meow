/// An error related to the builder.
#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    #[error("compile error: {0}")]
    CompileError(#[from] meow_vm_compiler::error::CompilerError),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("invalid file name: {0}")]
    InvalidFileName(String),
    #[error("source too large: {size} bytes exceeds limit of {limit} bytes")]
    SourceTooLarge { size: usize, limit: usize },
}
