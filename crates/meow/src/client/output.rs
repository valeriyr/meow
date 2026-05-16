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
    pub fn get_object(object: Option<Object>) -> Self {
        ClientCommandOutput::GetObject(object.map(|o| o.into()))
    }

    pub fn get_objects(objects: Vec<Option<Object>>) -> Self {
        ClientCommandOutput::GetObjects(objects.into_iter().map(|o| o.map(|o| o.into())).collect())
    }

    pub fn get_objects_owned(objects: Vec<Object>) -> Self {
        ClientCommandOutput::GetObjectsOwned(objects.into_iter().map(|o| o.into()).collect())
    }

    pub fn get_transaction(transaction: Option<SignedTransaction>) -> Self {
        ClientCommandOutput::GetTransaction(transaction.map(|t| t.into()))
    }

    pub fn get_transaction_result(result: Option<ExecutionResult>) -> Self {
        ClientCommandOutput::GetTransactionResult(result.map(|r| r.into()))
    }

    pub fn submit_transaction(digest: Digest) -> Self {
        ClientCommandOutput::SubmitTransaction(SubmitTransactionOutput {
            digest: digest.to_string(),
        })
    }
}
