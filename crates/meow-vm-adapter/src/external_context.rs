//! Block-level data injected before each transaction executes.
//!
//! The random seed is derived from the block hash and the timestamp from the block header,
//! making both values deterministic for all validators replaying the same block.

/// The type for the random seed.
pub type RandSeed = [u8; 32];

/// The zero random seed (all bytes are 0). Used as a safe default in tests and genesis execution,
/// where no block hash is available and randomness is not meaningful.
pub const DEFAULT_RAND_SEED: RandSeed = [0; 32];

/// External context for the VM adapter, containing data that is needed for execution.
pub struct ExternalContext {
    /// Random seed bytes (needed for native functions that require randomness).
    rand_seed: RandSeed,
    /// Block timestamp (Unix milliseconds) at the time the block was mined.
    /// Available to contracts via `meow_vm_timestamp()`.
    timestamp: u64,
}

impl ExternalContext {
    /// Creates a new external context with the given random seed and block timestamp.
    pub fn new(rand_seed: RandSeed, timestamp: u64) -> Self {
        Self {
            rand_seed,
            timestamp,
        }
    }

    /// Returns the seed bytes used for randomness.
    pub fn rand_seed(&self) -> &RandSeed {
        &self.rand_seed
    }

    /// Returns the block timestamp (Unix milliseconds).
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl Default for ExternalContext {
    fn default() -> Self {
        Self {
            rand_seed: DEFAULT_RAND_SEED,
            timestamp: 0,
        }
    }
}
