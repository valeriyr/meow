use meow_types::{
    digest::Digest,
    object::Object,
    transaction::{
        SignedTransaction,
        execution_result::{ExecutionResult, ExecutionStatus},
    },
};
use serde::Serialize;

use crate::object_output::ObjectOutput;
use crate::transaction_output::TransactionOutput;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ClientCommandOutput {
    GetObject(Option<ObjectOutput>),
    GetObjects(Vec<ObjectOutput>),
    GetTransaction(Option<TransactionOutput>),
    GetTransactionResult(Option<TransactionResultOutput>),
    SubmitTransaction(SubmitTransactionOutput),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResultOutput {
    pub digest: String,
    pub status: String,
    pub created: Vec<ObjectOutput>,
    pub changed: Vec<ObjectOutput>,
    pub destroyed: Vec<ObjectOutput>,
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

    pub fn get_objects(objects: Vec<Object>) -> Self {
        ClientCommandOutput::GetObjects(objects.into_iter().map(|o| o.into()).collect())
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

impl From<ExecutionResult> for TransactionResultOutput {
    fn from(r: ExecutionResult) -> Self {
        let status = match r.status() {
            ExecutionStatus::Success => "success".into(),
            ExecutionStatus::Failure(msg) => format!("failure: {msg}"),
        };
        Self {
            digest: r.transaction_digest().to_string(),
            status,
            created: r
                .created_objects()
                .iter()
                .map(|o| ObjectOutput::from(o.clone()))
                .collect(),
            changed: r
                .changed_objects()
                .iter()
                .map(|o| ObjectOutput::from(o.clone()))
                .collect(),
            destroyed: r
                .destroyed_objects()
                .iter()
                .map(|o| ObjectOutput::from(o.clone()))
                .collect(),
        }
    }
}
