/// Configuration for the miner.
#[derive(Clone)]
pub struct MinerConfig {
    /// The mining difficulty for the node.
    pub difficulty: u32,
}

impl MinerConfig {
    /// Creates a new `MinerConfig` with the given parameters.
    pub fn new(difficulty: u32) -> Self {
        Self { difficulty }
    }
}
