//! Chain-level size and count limits.
//!
//! Centralising these constants ensures all pipeline stages enforce the same maximums,
//! so a transaction accepted by one stage cannot be rejected by another.

pub mod error;

use std::{fs, path::PathBuf};

use error::ConfigError;
use meow_vm_types::config::{CompilerConfig, VmConfig};

/// The result type related to configurations.
pub type Result<T> = std::result::Result<T, ConfigError>;

/// The default configuration directory name.
const MEOW_DIR: &str = ".meow";
/// The default keystore file name.
const MEOW_KEYSTORE_FILE: &str = "keystore.json";

/// Maximum BCS-serialized byte size of a module.
pub const MAX_BCS_SERIALIZED_MODULE_SIZE: usize = 512 * 1024; // 512 KiB

/// Maximum BCS-serialized byte size of a `MeowCall` transaction.
pub const MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE: usize = 32 * 1024; // 32 KiB
/// Maximum serialized byte size of a `MeowModulePublish` transaction.
///
/// For module publish transactions, the size validation needs to account for the fact
/// that the transaction includes the module bytecode, which can be large.
/// We set the transaction size limit to be the maximum module size plus some overhead
/// for the other transaction fields.
pub const MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE: usize =
    MAX_BCS_SERIALIZED_MODULE_SIZE + 1024; // 513 KiB (module size + overhead)

/// The names of the native functions registered by this adapter.
/// Used to populate [`meow_vm_types::config::CompilerConfig::reserved_function_names`]
/// so the compiler rejects user-defined functions that would shadow them.
pub const NATIVE_FUNCTION_NAMES: &[&str] = &[
    "meow_vm_fresh_id",
    "meow_vm_transfer",
    "meow_vm_destroy",
    "meow_vm_sender",
    "meow_vm_rand",
    "meow_vm_timestamp",
];

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

/// Returns the compiler configuration.
pub fn compiler_config() -> CompilerConfig {
    CompilerConfig::default().with_reserved_function_names(NATIVE_FUNCTION_NAMES)
}

/// Returns the VM configuration.
pub fn vm_config() -> VmConfig {
    VmConfig::default()
}

/// Returns a VM configuration suitable for privileged execution,
/// which enables calling private functions.
pub fn vm_config_privileged() -> VmConfig {
    vm_config().with_enable_call_private_functions(true)
}
