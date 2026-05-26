//! Configuration for the proof-of-work miner.

use meow_types::{address::Address, keypair::KeyPair};

/// Configuration for the miner.
pub struct MinerConfig {
    /// The mining difficulty for the node.
    pub difficulty: u32,
    /// Keypair used to sign system transactions.
    pub keypair: KeyPair,
    /// Address that receives the minted reward coins. May differ from the keypair's
    /// own address so miners can direct earnings to a cold wallet or a separate account.
    pub reward_address: Address,
    /// Number of transactions to drain from the mempool per mining round.
    /// Mining starts once this many transactions are queued; a block may still
    /// end up with fewer if some are dropped during execution.
    pub batch_size: usize,
    /// Number of block snapshots to retain behind the chain head. Determines the
    /// maximum safe reorg depth — forks deeper than this cannot be resolved because
    /// the store snapshots needed for re-execution have been pruned. Also used as the
    /// state-sync threshold: peers whose chain is more than `snapshot_depth` blocks
    /// ahead trigger a full state snapshot download instead of block replay.
    pub snapshot_depth: u64,
}

impl MinerConfig {
    /// Creates a new miner configuration.
    pub fn new(
        difficulty: u32,
        keypair: KeyPair,
        reward_address: Address,
        batch_size: usize,
        snapshot_depth: u64,
    ) -> Self {
        Self {
            difficulty,
            keypair,
            reward_address,
            batch_size,
            snapshot_depth,
        }
    }
}
