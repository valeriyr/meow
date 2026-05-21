//! Error type for configuration loading.

/// An error related to configurations.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot obtain the home directory path")]
    CannotObtainHomeDirectoryPath,
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}
