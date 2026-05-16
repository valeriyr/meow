use meow_types::transaction::execution_result::ExecutionResult;
use serde::Serialize;

use crate::{
    output_encoder::OutputEncoder, outputs::transaction_result_output::TransactionResultOutput,
};

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum TransactionCommandOutput {
    Encoded(EncodedTransactionOutput),
    Simulate(TransactionResultOutput),
    ExecuteLocally(TransactionResultOutput),
}

/// Encoded transaction bytes (base64 or similar).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EncodedTransactionOutput {
    pub transaction: String,
}

impl TransactionCommandOutput {
    pub fn encoded<T: Serialize + std::fmt::Debug>(
        transaction: T,
        encoder: OutputEncoder,
    ) -> anyhow::Result<Self> {
        let transaction = encoder.encode(&transaction)?;
        Ok(TransactionCommandOutput::Encoded(
            EncodedTransactionOutput { transaction },
        ))
    }

    pub fn simulate(result: ExecutionResult, with_object_content: bool) -> Self {
        TransactionCommandOutput::Simulate(TransactionResultOutput::new(
            result,
            with_object_content,
        ))
    }

    pub fn execute_locally(result: ExecutionResult, with_object_content: bool) -> Self {
        TransactionCommandOutput::ExecuteLocally(TransactionResultOutput::new(
            result,
            with_object_content,
        ))
    }
}
