use base64::{Engine, engine::general_purpose};
use meow_vm_adapter::Module;
use strum_macros::EnumString;

/// The module encoder.
#[derive(Clone, Copy, Debug, EnumString, strum_macros::Display, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum ModuleEncoder {
    /// Encodes the module in Base64.
    Base64,
    /// Encodes the module in a debug format.
    Debug,
    /// Encodes the module in a pretty-printed format.
    Pretty,
}

impl ModuleEncoder {
    /// Encodes the module.
    pub fn encode(&self, module: &Module) -> Result<String, anyhow::Error> {
        Ok(match self {
            ModuleEncoder::Base64 => {
                let base64 = general_purpose::STANDARD.encode(&bcs::to_bytes(module)?);
                format!("{base64}")
            }
            ModuleEncoder::Debug => {
                format!("{module:?}")
            }
            ModuleEncoder::Pretty => {
                format!("{module:#?}")
            }
        })
    }
}
