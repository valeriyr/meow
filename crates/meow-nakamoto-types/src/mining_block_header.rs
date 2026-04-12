use meow_types::digest::Digest;
use serde::Serialize;

use crate::block_header::BlockHeader;

/// A [`BlockHeader`] data subset used for PoW grinding and as the randomness seed for execution.
///
/// Includes `transactions_root` so the miner cannot swap transactions after
/// finding a valid nonce. Excludes only `state_root`, which is unknowable until
/// execution completes, breaking the circular dependency.
#[derive(Serialize)]
pub struct MiningBlockHeader<'a> {
    /// Block number (0 = genesis).
    height: u64,
    /// Hash of the parent block header.
    parent_hash: &'a Digest,
    /// Hash over all transaction digests in this block.
    transactions_root: &'a Digest,
    /// Unix timestamp (milliseconds) at the time of mining.
    timestamp: u64,
    /// Nonce that satisfies the PoW difficulty target.
    nonce: u64,
}

impl MiningBlockHeader<'_> {
    /// Blake2b-256 hash of the BCS-serialized mining header.
    pub fn hash(&self) -> Digest {
        Digest::compute(self).expect("MiningBlockHeader serialization is infallible")
    }
}

impl<'a> From<&'a BlockHeader> for MiningBlockHeader<'a> {
    fn from(header: &'a BlockHeader) -> Self {
        Self {
            height: header.height,
            parent_hash: &header.parent_hash,
            transactions_root: &header.transactions_root,
            timestamp: header.timestamp,
            nonce: header.nonce,
        }
    }
}
