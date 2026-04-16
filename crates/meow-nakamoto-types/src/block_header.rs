use meow_types::digest::Digest;
use serde::{Deserialize, Serialize};

use crate::mining_block_header::MiningBlockHeader;

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
    /// Unix timestamp (milliseconds) at the time of mining.
    pub timestamp: u64,
    /// Nonce that satisfies the PoW difficulty target.
    pub nonce: u64,
}

impl BlockHeader {
    /// Blake2b-256 hash of the BCS-serialized header (all fields).
    ///
    /// Used as the block's identity: stored in `parent_hash` of the next block
    /// and as the key in `ChainState`.
    pub fn hash(&self) -> Digest {
        Digest::compute(self).expect("BlockHeader serialization is infallible")
    }

    /// Blake2b-256 hash over `height`, `parent_hash`, `transactions_root`,
    /// `timestamp`, and `nonce` — everything except `state_root`.
    ///
    /// Used for two purposes:
    /// - **PoW target**: `meets_difficulty` checks this hash. Excluding `state_root`
    ///   breaks the circular dependency (state is only known after execution), so
    ///   the miner can grind the nonce without running transactions first.
    /// - **Randomness seed**: passed to the VM executor so that the random seed is
    ///   fixed the moment the nonce is found — *after* the block is mined but
    ///   *before* any transaction runs. Transaction senders cannot predict it.
    ///   Including `transactions_root` ensures the miner cannot swap transactions
    ///   after finding a valid nonce to fish for a favorable seed.
    pub fn mining_hash(&self) -> Digest {
        let mining_header: MiningBlockHeader = self.into();
        mining_header.hash()
    }

    /// Returns true if the `mining_hash` has at least `difficulty` leading zero bits.
    pub fn meets_difficulty(&self, difficulty: u32) -> bool {
        let hash = self.mining_hash();
        leading_zeros(&hash) >= difficulty
    }
}

/// Counts the number of leading zero bits in the digest.
fn leading_zeros(digest: &Digest) -> u32 {
    let mut leading_zeros = 0u32;

    for byte in digest.as_ref() {
        let lz = byte.leading_zeros();

        leading_zeros += lz;

        if lz < 8 {
            break;
        }
    }

    leading_zeros
}

#[cfg(test)]
mod tests {

    use meow_types::digest::{DIGEST_LENGTH, Digest};

    use super::leading_zeros;

    #[test]
    fn zero_digest_leading_zeros() {
        let digest = Digest::ZERO;
        assert_eq!(leading_zeros(&digest), 256);
    }

    #[test]
    fn digest_with_one_leading_zero() {
        let digest = Digest::from([0x7F; DIGEST_LENGTH]);
        assert_eq!(leading_zeros(&digest), 1);
    }

    #[test]
    fn digest_without_leading_zeros() {
        let digest = Digest::from([0xFF; DIGEST_LENGTH]);
        assert_eq!(leading_zeros(&digest), 0);
    }
}
