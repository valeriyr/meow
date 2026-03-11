/// An error that occurred during compilation (parsing or code generation).
#[derive(Debug, thiserror::Error)]
pub enum CompilerError {
    #[error("{0}")]
    Message(String),
}
