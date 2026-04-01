use crate::{Result, error::VmError};

/// Tracks gas spending during execution.
#[derive(Debug, Clone)]
pub struct GasMeter {
    limit: u64,
    spent: u64,
}

impl GasMeter {
    /// Creates a new gas meter with the given limit.
    pub fn new(limit: u64) -> Self {
        Self { limit, spent: 0 }
    }

    /// Unlimited gas meter (for testing / trusted contexts).
    pub fn unlimited() -> Self {
        Self::new(u64::MAX)
    }

    /// Charge `cost` units of gas. Returns [`VmError::OutOfGas`] if the limit is exceeded.
    pub fn charge(&mut self, cost: u64) -> Result<()> {
        let new = self.spent.saturating_add(cost);

        self.spent = new;

        if new > self.limit {
            return Err(VmError::OutOfGas {
                spent: new,
                limit: self.limit,
            });
        }

        Ok(())
    }

    /// Returns the total gas spent so far.
    pub fn spent(&self) -> u64 {
        self.spent
    }

    /// Returns the remaining gas available.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.spent)
    }

    /// Returns the gas limit.
    pub fn limit(&self) -> u64 {
        self.limit
    }
}
