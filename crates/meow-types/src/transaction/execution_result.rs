//! Execution result recording the outcome of a transaction and the objects it affected.

use serde::{Deserialize, Serialize};

use crate::{digest::Digest, object::Object};

/// The execution status of a transaction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// The transaction executed successfully.
    Success,
    /// The transaction failed with an error message.
    Failure(String),
}

/// The result of a transaction execution.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    /// The execution status.
    status: ExecutionStatus,
    /// The digest of the transaction that produced this execution result.
    transaction_digest: Digest,
    /// The created objects.
    created_objects: Vec<Object>,
    /// The changed objects.
    changed_objects: Vec<Object>,
    /// The destroyed objects.
    destroyed_objects: Vec<Object>,
    /// Gas units consumed by this transaction.
    gas_used: u64,
}

impl ExecutionResult {
    /// Creates a new execution result.
    pub fn new(
        status: ExecutionStatus,
        transaction_digest: Digest,
        created_objects: Vec<Object>,
        changed_objects: Vec<Object>,
        destroyed_objects: Vec<Object>,
    ) -> Self {
        Self {
            status,
            transaction_digest,
            created_objects,
            changed_objects,
            destroyed_objects,
            gas_used: 0,
        }
    }

    /// Creates a failure execution result with the given error message and transaction digest.
    pub fn failure(message: impl Into<String>, transaction_digest: Digest) -> Self {
        Self::new(
            ExecutionStatus::Failure(message.into()),
            transaction_digest,
            vec![],
            vec![],
            vec![],
        )
    }

    /// Sets the gas consumed and returns `self` for chaining.
    pub fn with_gas_used(mut self, gas_used: u64) -> Self {
        self.gas_used = gas_used;
        self
    }

    /// Returns the execution status.
    pub fn status(&self) -> &ExecutionStatus {
        &self.status
    }

    /// Returns the transaction digest.
    pub fn transaction_digest(&self) -> &Digest {
        &self.transaction_digest
    }

    /// Returns the created objects.
    pub fn created_objects(&self) -> &[Object] {
        &self.created_objects
    }

    /// Returns the changed objects.
    pub fn changed_objects(&self) -> &[Object] {
        &self.changed_objects
    }

    /// Returns the destroyed objects.
    pub fn destroyed_objects(&self) -> &[Object] {
        &self.destroyed_objects
    }

    /// Returns the gas units consumed by this transaction.
    pub fn gas_used(&self) -> u64 {
        self.gas_used
    }
}
