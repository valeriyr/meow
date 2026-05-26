//! Full chain state snapshot.

use meow_types::object::Object;
use serde::{Deserialize, Serialize};

use crate::block::Block;

/// How many block snapshots to keep behind the head. Limits reorg depth.
/// Forks deeper than this cannot be resolved because the store snapshots needed
/// to re-execute them have been discarded.
pub const SNAPSHOT_DEPTH: u64 = 64;

/// A full state snapshot at the current chain head.
///
/// The snapshot is trustworthy when:
/// 1. Block header meets difficulty — the block has valid PoW.
/// 2. State root matches the objects in the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// The current best block.
    pub head: Block,
    /// All live objects in the store at the head block.
    pub objects: Vec<Object>,
}
