//! Output types for the client subcommands.

use meow_nakamoto_types::{block::Block, state_snapshot::StateSnapshot};
use meow_types::{
    digest::Digest,
    object::Object,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
};
use serde::Serialize;

use crate::outputs::{
    block_output::BlockOutput, block_snapshot_output::BlockSnapshotOutput,
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
    GetBlock(Option<BlockOutput>),
    GetBlockSnapshot(Option<BlockSnapshotOutput>),
    GetChainHead(ChainHeadOutput),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitTransactionOutput {
    pub digest: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainHeadOutput {
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

    pub fn get_block(block: Option<Block>, with_object_content: bool) -> Self {
        ClientCommandOutput::GetBlock(block.map(|b| BlockOutput::new(b, with_object_content)))
    }

    pub fn get_block_snapshot(snapshot: Option<StateSnapshot>, with_object_content: bool) -> Self {
        ClientCommandOutput::GetBlockSnapshot(
            snapshot.map(|s| BlockSnapshotOutput::new(s, with_object_content)),
        )
    }

    pub fn get_chain_head(digest: Digest) -> Self {
        ClientCommandOutput::GetChainHead(ChainHeadOutput {
            digest: digest.to_string(),
        })
    }
}
