use base64::{Engine, engine::general_purpose};
use serde::Serialize;
use strum_macros::EnumString;

/// The output encoder.
#[derive(Clone, Copy, Debug, EnumString, strum_macros::Display, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum OutputEncoder {
    /// Encodes the output in Base64.
    Base64,
    /// Encodes the output in a debug format.
    Debug,
    /// Encodes the output in a pretty-printed format.
    Pretty,
}

impl OutputEncoder {
    /// Encodes the output.
    pub fn encode<T: Serialize + std::fmt::Debug>(
        &self,
        output: &T,
    ) -> Result<String, anyhow::Error> {
        Ok(match self {
            OutputEncoder::Base64 => {
                let base64 = general_purpose::STANDARD.encode(&bcs::to_bytes(output)?);
                format!("{base64}")
            }
            OutputEncoder::Debug => {
                format!("{output:?}")
            }
            OutputEncoder::Pretty => {
                format!("{output:#?}")
            }
        })
    }
}
