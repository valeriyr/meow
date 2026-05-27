//! Serializable summary of an on-chain block for CLI output.

use meow_nakamoto_types::{block::Block, block_header::BlockHeader};
use serde::Serialize;

use crate::outputs::{
    transaction_output::TransactionOutput, transaction_result_output::TransactionResultOutput,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockHeaderOutput {
    pub height: String,
    pub parent_hash: String,
    pub transactions_root: String,
    pub reward_root: Option<String>,
    pub state_root: String,
    pub timestamp: String,
    pub nonce: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockOutput {
    pub hash: String,
    pub header: BlockHeaderOutput,
    pub transactions: Vec<TransactionOutput>,
    pub results: Vec<TransactionResultOutput>,
    pub reward_transaction: Option<TransactionOutput>,
    pub reward_transaction_result: Option<TransactionResultOutput>,
}

impl BlockOutput {
    pub fn new(block: Block, with_object_content: bool) -> Self {
        Self {
            hash: block.hash().to_string(),
            header: block.header.into(),
            transactions: block
                .transactions
                .into_iter()
                .map(TransactionOutput::from)
                .collect(),
            results: block
                .results
                .into_iter()
                .map(|r| TransactionResultOutput::new(r, with_object_content))
                .collect(),
            reward_transaction: block.reward_transaction.map(TransactionOutput::from),
            reward_transaction_result: block
                .reward_transaction_result
                .map(|r| TransactionResultOutput::new(r, with_object_content)),
        }
    }
}

impl From<BlockHeader> for BlockHeaderOutput {
    fn from(block_header: BlockHeader) -> Self {
        Self {
            height: block_header.height.to_string(),
            parent_hash: block_header.parent_hash.to_string(),
            transactions_root: block_header.transactions_root.to_string(),
            reward_root: block_header.reward_root.map(|d| d.to_string()),
            state_root: block_header.state_root.to_string(),
            timestamp: block_header.timestamp.to_string(),
            nonce: block_header.nonce.to_string(),
        }
    }
}
