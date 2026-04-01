use meow_types::{
    address::Address,
    digest::Digest,
    transaction::execution_result::{ExecutionResult, ExecutionStatus},
};
use serde::Serialize;

use crate::object_brief_info::ObjectBriefInfo;

#[derive(Debug, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum ClientCommandOutput {
    GetObject(ObjectOutput),
    GetTransactionResult(TransactionResultOutput),
    SubmitTransaction(SubmitTransactionOutput),
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ObjectOutput {
    Found(ObjectBriefInfo),
    NotFound { address: Address },
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TransactionResultOutput {
    Found(TransactionResultDetails),
    NotFound { digest: Digest },
}

#[derive(Debug, Serialize)]
pub struct TransactionResultDetails {
    pub digest: Digest,
    pub status: String,
    pub created: Vec<ObjectBriefInfo>,
    pub changed: Vec<ObjectBriefInfo>,
    pub destroyed: Vec<ObjectBriefInfo>,
}

#[derive(Debug, Serialize)]
pub struct SubmitTransactionOutput {
    pub digest: Digest,
}

impl From<ExecutionResult> for TransactionResultDetails {
    fn from(r: ExecutionResult) -> Self {
        let status = match r.status() {
            ExecutionStatus::Success => "success".into(),
            ExecutionStatus::Failure(msg) => format!("failure: {msg}"),
        };
        Self {
            digest: *r.transaction_digest(),
            status,
            created: r
                .created_objects()
                .into_iter()
                .map(|o| ObjectBriefInfo::from(o.clone()))
                .collect(),
            changed: r
                .changed_objects()
                .into_iter()
                .map(|o| ObjectBriefInfo::from(o.clone()))
                .collect(),
            destroyed: r
                .destroyed_objects()
                .into_iter()
                .map(|o| ObjectBriefInfo::from(o.clone()))
                .collect(),
        }
    }
}
