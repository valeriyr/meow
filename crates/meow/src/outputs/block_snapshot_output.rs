//! Serializable summary of an on-chain block snapshot for CLI output.

use meow_nakamoto_types::state_snapshot::StateSnapshot;
use serde::Serialize;

use crate::outputs::{block_output::BlockOutput, object_output::ObjectOutput};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSnapshotOutput {
    pub head: BlockOutput,
    pub objects: Vec<ObjectOutput>,
}

impl BlockSnapshotOutput {
    pub fn new(snapshot: StateSnapshot, with_object_content: bool) -> Self {
        Self {
            head: BlockOutput::new(snapshot.head, with_object_content),
            objects: snapshot
                .objects
                .into_iter()
                .map(|o| ObjectOutput::new(o, with_object_content))
                .collect(),
        }
    }
}
