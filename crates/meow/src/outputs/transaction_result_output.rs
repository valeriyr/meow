use meow_types::transaction::execution_result::{ExecutionResult, ExecutionStatus};
use serde::Serialize;

use crate::outputs::object_output::ObjectOutput;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResultOutput {
    pub digest: String,
    pub status: String,
    pub gas_used: String,
    pub created: Vec<ObjectOutput>,
    pub changed: Vec<ObjectOutput>,
    pub destroyed: Vec<ObjectOutput>,
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
            gas_used: r.gas_used().to_string(),
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
