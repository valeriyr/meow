//! A committed block containing a header, transactions, and their execution results.

use meow_types::{
    digest::Digest,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
};
use serde::{Deserialize, Serialize};

use crate::block_header::BlockHeader;

/// A committed block: header + transactions + execution results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
    pub results: Vec<ExecutionResult>,
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
        }
    }
}
