use meow_types::{
    digest::Digest,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
};
use serde::{Deserialize, Serialize};

/// The header that is hashed for PoW and chain linking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Block number (0 = genesis).
    pub height: u64,
    /// Hash of the parent block header.
    pub parent_hash: Digest,
    /// Hash over all transaction digests in this block.
    pub transactions_root: Digest,
    /// Hash of the object store state after applying this block.
    pub state_root: Digest,
    /// Unix timestamp (seconds) at the time of mining.
    pub timestamp: u64,
    /// Nonce that satisfies the PoW difficulty target.
    pub nonce: u64,
}

impl BlockHeader {
    /// Blake2b-256 hash of the BCS-serialized header.
    pub fn hash(&self) -> Digest {
        Digest::compute(self).expect("BlockHeader serialization is infallible")
    }

    /// Returns true if the header hash has at least `difficulty` leading zero bits.
    pub fn meets_difficulty(&self, difficulty: u32) -> bool {
        let hash = self.hash();
        let mut leading_zeros = 0u32;
        for byte in hash.as_ref() {
            let lz = byte.leading_zeros();
            leading_zeros += lz;
            if lz < 8 {
                break;
            }
        }
        leading_zeros >= difficulty
    }
}

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
