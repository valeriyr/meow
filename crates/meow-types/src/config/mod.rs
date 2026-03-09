pub mod error;

use std::{fs, path::PathBuf};

use error::ConfigError;

/// The result type related to configurations.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// The default configuration directory name.
const MEOW_DIR: &str = ".meow";
/// The default keystore file name.
const MEOW_KEYSTORE_FILE: &str = "keystore.json";

/// Returns the path to the meow configuration directory.
///
/// Creates the directory if it does not exist.
pub fn meow_config_dir() -> Result<PathBuf> {
    match std::env::var_os("MEOW_CONFIG_DIR") {
        Some(config_env) => Ok(config_env.into()),
        None => match dirs::home_dir() {
            Some(v) => Ok(v.join(MEOW_DIR)),
            None => Err(ConfigError::CannotObtainHomeDirectoryPath),
        },
    }
    .and_then(|dir| {
        if !dir.exists() {
            fs::create_dir_all(dir.clone())?;
        }
        Ok(dir)
    })
}

/// Returns the path to the keystore file.
///
/// Creates the directory if it does not exist.
pub fn meow_keystore_path() -> Result<PathBuf> {
    meow_config_dir().map(|dir| dir.join(MEOW_KEYSTORE_FILE))
}
