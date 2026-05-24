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
}

impl MinerConfig {
    /// Creates a new `MinerConfig` with the given parameters.
    pub fn new(difficulty: u32, keypair: KeyPair, reward_address: Address) -> Self {
        Self {
            difficulty,
            keypair,
            reward_address,
        }
    }
}
