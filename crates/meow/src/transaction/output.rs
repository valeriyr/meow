use serde::Serialize;

use crate::output_encoder::OutputEncoder;

/// The transaction output.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionCommandOutput {
    /// The transaction as a string encoded in base64.
    pub transaction: String,
}

impl TransactionCommandOutput {
    pub fn new<T: Serialize + std::fmt::Debug>(
        transaction: T,
        encoder: OutputEncoder,
    ) -> anyhow::Result<Self> {
        let transaction = encoder.encode(&transaction)?;

        Ok(TransactionCommandOutput { transaction })
    }
}
