use meow_types::{
    digest::Digest,
    object::Object,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
};
use serde::Serialize;

use crate::outputs::{
    object_output::ObjectOutput, transaction_output::TransactionOutput,
    transaction_result_output::TransactionResultOutput,
};

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ClientCommandOutput {
    GetObject(Option<ObjectOutput>),
    GetObjects(Vec<Option<ObjectOutput>>),
    GetObjectsOwned(Vec<ObjectOutput>),
    GetTransaction(Option<TransactionOutput>),
    GetTransactionResult(Option<TransactionResultOutput>),
    SubmitTransaction(SubmitTransactionOutput),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitTransactionOutput {
    pub digest: String,
}

impl ClientCommandOutput {
    pub fn get_object(object: Option<Object>, with_object_content: bool) -> Self {
        ClientCommandOutput::GetObject(object.map(|o| ObjectOutput::new(o, with_object_content)))
    }

    pub fn get_objects(objects: Vec<Option<Object>>, with_object_content: bool) -> Self {
        ClientCommandOutput::GetObjects(
            objects
                .into_iter()
                .map(|o| o.map(|o| ObjectOutput::new(o, with_object_content)))
                .collect(),
        )
    }

    pub fn get_objects_owned(objects: Vec<Object>, with_object_content: bool) -> Self {
        ClientCommandOutput::GetObjectsOwned(
            objects
                .into_iter()
                .map(|o| ObjectOutput::new(o, with_object_content))
                .collect(),
        )
    }

    pub fn get_transaction(transaction: Option<SignedTransaction>) -> Self {
        ClientCommandOutput::GetTransaction(transaction.map(|t| t.into()))
    }

    pub fn get_transaction_result(
        result: Option<ExecutionResult>,
        with_object_content: bool,
    ) -> Self {
        ClientCommandOutput::GetTransactionResult(
            result.map(|r| TransactionResultOutput::new(r, with_object_content)),
        )
    }

    pub fn submit_transaction(digest: Digest) -> Self {
        ClientCommandOutput::SubmitTransaction(SubmitTransactionOutput {
            digest: digest.to_string(),
        })
    }
}
