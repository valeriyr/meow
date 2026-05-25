//! A committed block containing a header, transactions, and their execution results.

use meow_types::{
    digest::Digest,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
};
use serde::{Deserialize, Serialize};

use crate::block_header::BlockHeader;

/// A fully validated, committed block that advances the chain by one height.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    /// Signed user transactions included in this block.
    pub transactions: Vec<SignedTransaction>,
    /// Execution results for each transaction, in the same order.
    pub results: Vec<ExecutionResult>,
    /// Miner-signed reward transaction for the gas fees collected in this block.
    /// `None` when no gas was collected.
    pub reward_transaction: Option<SignedTransaction>,
    /// Execution result of the reward transaction. `None` when `reward_transaction` is `None`.
    pub reward_transaction_result: Option<ExecutionResult>,
}

impl Block {
    /// Returns the hash of the block header, which is the block's unique identifier.
    pub fn hash(&self) -> Digest {
        self.header.hash()
    }

    /// The genesis block: height 0, no transactions, no PoW required.
    pub fn genesis() -> Self {
        Self {
            header: BlockHeader {
                height: 0,
                parent_hash: Digest::ZERO,
                transactions_root: Digest::ZERO,
                state_root: Digest::ZERO,
                timestamp: 0,
                nonce: 0,
            },
            transactions: vec![],
            results: vec![],
            reward_transaction: None,
            reward_transaction_result: None,
        }
    }
}
