//! Chain state tracking blocks, object-store snapshots, and fork resolution.

pub mod error;

use std::collections::{BTreeMap, BTreeSet};

use meow_nakamoto_types::{block::Block, state_snapshot::SNAPSHOT_DEPTH};
use meow_types::{
    digest::Digest,
    time,
    transaction::{SignedTransaction, execution_result::ExecutionResult, validator},
};
use meow_vm_adapter::{executor, external_context::ExternalContext, inputs_resolver};

use crate::{chain::error::ChainError, roots, store::Store, system_transactions};

/// The result type related to the miner.
pub type Result<T> = std::result::Result<T, ChainError>;

/// Tracks the full chain of blocks and the object store snapshot at each tip.
///
/// Fork resolution: when a peer's block extends a chain longer than the local
/// head, `ChainState` switches to that chain and updates the head store.
/// Reorg is safe because the snapshot at every block is stored — reverting
/// to a fork point is a simple snapshot restore, no re-execution needed.
pub struct ChainState {
    /// All known blocks indexed by header hash.
    blocks: BTreeMap<Digest, Block>,
    /// Object store state after applying each block (snapshot per block).
    snapshots: BTreeMap<Digest, Store>,
    /// Fast lookup: transaction digest → execution result across all committed blocks.
    results: BTreeMap<Digest, ExecutionResult>,
    /// Current best tip (block with the most accumulated PoW).
    head: Digest,
    /// PoW difficulty: minimum number of leading zero bits required in a block hash.
    difficulty: u32,
}

/// Maximum number of milliseconds a block's timestamp may be ahead of local clock.
/// Blocks further in the future than this are rejected to prevent timestamp manipulation.
const MAX_BLOCK_FUTURE_DRIFT_MS: u64 = 120_000; // 2 minutes

impl ChainState {
    /// Creates a chain rooted at genesis, with the given initial store as the
    /// state at block 0. The genesis block itself requires no PoW.
    pub fn new(initial_store: Store, difficulty: u32) -> Self {
        let genesis = Block::genesis();
        let genesis_hash = genesis.hash();

        let mut blocks = BTreeMap::new();
        let mut snapshots = BTreeMap::new();

        blocks.insert(genesis_hash, genesis);
        snapshots.insert(genesis_hash, initial_store);

        Self {
            blocks,
            snapshots,
            results: BTreeMap::new(),
            head: genesis_hash,
            difficulty,
        }
    }

    /// Validates a peer's state snapshot and anchors the chain at it.
    ///
    /// Validates in order:
    /// 1. Snapshot height is strictly greater than `current_head_height`.
    /// 2. All structural checks via [`validate_block_structure`]: non-empty, no duplicate
    ///    transactions, timestamp not in future, results count, reward consistency, PoW
    ///    difficulty, transactions root, reward root.
    /// 3. State root matches the supplied objects.
    pub fn from_snapshot(
        current_head_height: u64,
        head_block: Block,
        store: Store,
        difficulty: u32,
    ) -> Result<Self> {
        let snap_height = head_block.header.height;
        if snap_height <= current_head_height {
            tracing::warn!(
                snap_height,
                current_head_height,
                "snapshot does not advance the chain — ignoring"
            );
            return Err(ChainError::SnapshotNotAdvancing {
                snap_height,
                head_height: current_head_height,
            });
        }

        validate_block_structure(&head_block, difficulty)?;

        let block_hash = head_block.hash();

        let computed_state_root = roots::compute_state_root(&store);
        if computed_state_root != head_block.header.state_root {
            tracing::warn!(
                snap_height,
                %block_hash,
                expected = %head_block.header.state_root,
                computed = %computed_state_root,
                "snapshot state root mismatch — ignoring"
            );
            return Err(ChainError::StateRootMismatch);
        }
        let mut blocks = BTreeMap::new();
        let mut snapshots = BTreeMap::new();

        blocks.insert(block_hash, head_block);
        snapshots.insert(block_hash, store);

        Ok(Self {
            blocks,
            snapshots,
            results: BTreeMap::new(),
            head: block_hash,
            difficulty,
        })
    }

    /// PoW difficulty configured for this chain.
    pub fn difficulty(&self) -> u32 {
        self.difficulty
    }

    /// Hash of the current best block.
    pub fn head(&self) -> Digest {
        self.head
    }

    /// The current best block.
    pub fn head_block(&self) -> &Block {
        self.blocks.get(&self.head).expect("head always exists")
    }

    /// Height of the current best block.
    pub fn head_height(&self) -> u64 {
        self.head_block().header.height
    }

    /// Earliest block height from which a sync should start to cover all resolvable reorgs.
    ///
    /// Pulling blocks from this height onwards guarantees that the common ancestor of any
    /// fork at most `SNAPSHOT_DEPTH` blocks deep is included, so the pulled chain can be
    /// applied end-to-end without hitting a missing-parent rejection.
    pub fn sync_from_height(&self) -> u64 {
        self.head_height().saturating_sub(SNAPSHOT_DEPTH)
    }

    /// Object store state at the current best tip.
    pub fn head_store(&self) -> &Store {
        self.snapshots
            .get(&self.head)
            .expect("head snapshot always exists")
    }

    /// Look up an execution result by transaction digest.
    pub fn get_transaction_result(&self, digest: &Digest) -> Option<&ExecutionResult> {
        self.results.get(digest)
    }

    /// Look up a committed transaction by digest, searching across all known blocks.
    pub fn get_transaction(&self, digest: &Digest) -> Option<&SignedTransaction> {
        self.blocks
            .values()
            .flat_map(|block| block.transactions.iter())
            .find(|tx| tx.transaction().digest() == *digest)
    }

    /// Commit a locally-mined block and its resulting store state.
    /// The caller is responsible for executing the transactions and computing
    /// the new store; this method records the block, indexes its results, and
    /// advances the head.
    pub fn commit(&mut self, block: Block, new_store: Store) {
        validate_block_structure(&block, self.difficulty)
            .expect("commit called with structurally invalid block");

        let hash = block.hash();

        for result in &block.results {
            self.results
                .insert(*result.transaction_digest(), result.clone());
        }

        self.blocks.insert(hash, block);
        self.snapshots.insert(hash, new_store);
        self.head = hash;

        self.prune_finalized_blocks();
    }

    /// Drop all blocks and store snapshots more than `SNAPSHOT_DEPTH` blocks behind the head.
    /// Headers and snapshots are always removed together, keeping the invariant that a block
    /// in `self.blocks` always has a corresponding entry in `self.snapshots`.
    fn prune_finalized_blocks(&mut self) {
        let head_height = self.head_block().header.height;
        if head_height <= SNAPSHOT_DEPTH {
            return;
        }
        let cutoff = head_height - SNAPSHOT_DEPTH;
        let to_remove: Vec<Digest> = self
            .blocks
            .iter()
            .filter(|(_, b)| b.header.height < cutoff)
            .map(|(hash, _)| *hash)
            .collect();
        for hash in to_remove {
            self.snapshots.remove(&hash);
            self.blocks.remove(&hash);
        }
    }

    /// Process a block received from a peer.
    ///
    /// Validates in order:
    /// 1. Block is not already known.
    /// 2. Parent block is known.
    /// 3. Structural checks via [`validate_block_structure`]: non-empty, no duplicate
    ///    transactions, timestamp not in future, results count, reward consistency, PoW
    ///    difficulty, transactions root, reward root.
    /// 4. Height is exactly parent height + 1.
    /// 5. Timestamp is strictly greater than parent timestamp.
    /// 6. All transaction signatures are valid.
    /// 7. Local deterministic re-execution matches the included results.
    /// 8. Reward transaction valid and result matches (if total gas > 0).
    /// 9. State root matches the store produced by re-execution.
    ///
    /// If the block extends a chain longer than the current head, the head
    /// is updated (chain reorganization). Returns `Ok(true)` when the head
    /// changed, `Ok(false)` when the block was valid but did not extend the
    /// longest chain, or `Err` when the block was rejected.
    pub fn apply_block(&mut self, block: Block) -> Result<bool> {
        let block_hash = block.hash();

        // Already known — skip.
        if self.blocks.contains_key(&block_hash) {
            return Err(ChainError::AlreadyKnown);
        }

        let parent_hash = block.header.parent_hash;
        let height = block.header.height;

        // Parent must be known; otherwise we'd need to request the missing ancestors.
        if !self.blocks.contains_key(&parent_hash) {
            tracing::debug!(height, %block_hash, %parent_hash, "block with unknown parent — ignoring");
            return Err(ChainError::UnknownParent);
        }

        // Structural checks: non-empty, timestamp, PoW, transactions root.
        validate_block_structure(&block, self.difficulty)?;

        // Height must be exactly parent + 1.
        let parent_height = self.blocks[&parent_hash].header.height;
        if height != parent_height + 1 {
            let expected = parent_height + 1;
            tracing::warn!(height, %block_hash, expected, "block has wrong height — ignoring");
            return Err(ChainError::InvalidHeight {
                expected,
                got: height,
            });
        }

        let timestamp = block.header.timestamp;

        // Timestamp must be strictly greater than the parent's to ensure time
        // only moves forward — important for contracts that read the block time.
        let parent_timestamp = self.blocks[&parent_hash].header.timestamp;
        if timestamp <= parent_timestamp {
            tracing::warn!(
                height,
                %block_hash,
                timestamp,
                parent_timestamp,
                "block timestamp is not greater than parent — ignoring"
            );
            return Err(ChainError::TimestampNotAdvancing);
        }

        // Validate all transaction signatures before re-executing.
        for signed_transaction in &block.transactions {
            if let Err(e) = validator::validate_signed_transaction(signed_transaction) {
                tracing::warn!(
                    height,
                    %block_hash,
                    tx_digest = %signed_transaction.transaction().digest(),
                    error = %e,
                    "block has invalid transaction signature"
                );
                return Err(ChainError::InvalidSignature);
            }
        }

        // Build the store snapshot for this block by deterministically re-executing all transactions.
        // Parent is in self.blocks (checked above) so its snapshot is guaranteed present —
        // blocks and snapshots are always pruned together.
        let parent_snapshot = self
            .snapshots
            .get(&parent_hash)
            .expect("parent snapshot always exists when parent block is known");

        let mut new_store = parent_snapshot.clone();
        let mut recomputed_results = Vec::with_capacity(block.transactions.len());

        // Use the mining hash as the randomness seed — it commits height,
        // parent_hash, transactions_root, timestamp, and nonce, all of which
        // are fixed before any transaction runs and verifiable by every node.
        let external_executor_context =
            ExternalContext::new(block.header.mining_hash().into(), timestamp);

        for signed_transaction in &block.transactions {
            let transaction = signed_transaction.transaction();
            let inputs = inputs_resolver::collect_inputs(transaction, |addr| {
                new_store.get_object(addr).cloned()
            });
            let result = match executor::execute(transaction, inputs, &external_executor_context) {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!(
                        height,
                        %block_hash,
                        tx_digest = %transaction.digest(),
                        error = %e,
                        "block execution failed during verification"
                    );
                    return Err(ChainError::ExecutionFailed);
                }
            };

            new_store.apply_execution_result(&result);
            recomputed_results.push(result);
        }

        if recomputed_results != block.results {
            tracing::warn!(height, %block_hash, "block results mismatch local execution — ignoring");
            return Err(ChainError::ResultsMismatch);
        }

        let total_gas = recomputed_results.iter().map(|r| r.gas_used()).sum();
        let reward_valid = match (
            total_gas,
            block.reward_transaction.as_ref(),
            block.reward_transaction_result.as_ref(),
        ) {
            (0, None, None) => true,
            (0, _, _) => {
                tracing::warn!(height, %block_hash, "unexpected reward on zero-gas block — ignoring");
                false
            }
            (_, None, _) | (_, _, None) => {
                tracing::warn!(height, %block_hash, total_gas, "missing reward on non-zero-gas block — ignoring");
                false
            }
            (amount, Some(reward_transaction), Some(reward_execution_result)) => {
                apply_block_reward_transaction(
                    reward_transaction,
                    reward_execution_result,
                    amount,
                    &block_hash,
                    block.header.mining_hash(),
                    &mut new_store,
                    height,
                )
            }
        };

        if !reward_valid {
            return Err(ChainError::InvalidReward);
        }

        let recomputed_state_root = roots::compute_state_root(&new_store);
        if recomputed_state_root != block.header.state_root {
            tracing::warn!(
                height,
                %block_hash,
                expected = %block.header.state_root,
                computed = %recomputed_state_root,
                "block has invalid state root — ignoring"
            );
            return Err(ChainError::StateRootMismatch);
        }

        for result in &recomputed_results {
            self.results
                .insert(*result.transaction_digest(), result.clone());
        }

        self.blocks.insert(block_hash, block);
        self.snapshots.insert(block_hash, new_store);

        // Switch head to this block if it's on a longer chain.
        if height > self.head_height() {
            tracing::info!(height, %block_hash, old_height = self.head_height(), old_block_hash = %self.head, "chain reorg: switching to longer chain");
            self.head = block_hash;
            self.prune_finalized_blocks();
            return Ok(true);
        }

        Ok(false)
    }

    /// Returns all blocks at or above the given height.
    /// Order is not guaranteed — callers must sort if height order is required.
    pub fn get_blocks_since(&self, height: u64) -> Vec<Block> {
        self.blocks
            .values()
            .filter(|b| b.header.height >= height)
            .cloned()
            .collect()
    }
}

/// Validate the block's structural properties that can be checked without knowledge of
/// the parent block or the chain state. Used by both `apply_block` and `from_snapshot`.
///
/// Checks in order (cheapest first):
/// 1. Block has at least one transaction.
/// 2. No duplicate transaction digests.
/// 3. Timestamp is not too far in the future.
/// 4. Results count matches transaction count.
/// 5. `reward_transaction` and `reward_transaction_result` are both present or both absent.
/// 6. Block meets PoW difficulty (skipped at height 0).
/// 7. Transactions root matches the transaction list.
/// 8. Reward root matches the reward transaction (or both absent).
fn validate_block_structure(block: &Block, difficulty: u32) -> Result<()> {
    let height = block.header.height;
    if block.transactions.is_empty() {
        tracing::warn!(height, block_hash = %block.hash(), "block has no transactions");
        return Err(ChainError::EmptyBlock);
    }
    let mut seen = BTreeSet::new();
    for tx in &block.transactions {
        let digest = tx.transaction().digest();
        if !seen.insert(digest) {
            tracing::warn!(height, block_hash = %block.hash(), %digest, "block contains duplicate transaction");
            return Err(ChainError::DuplicateTransaction);
        }
    }
    let now = time::current_timestamp();
    if block.header.timestamp > now + MAX_BLOCK_FUTURE_DRIFT_MS {
        tracing::warn!(
            height,
            block_hash = %block.hash(),
            timestamp = block.header.timestamp,
            now,
            "block timestamp is too far in the future"
        );
        return Err(ChainError::TimestampTooFarInFuture);
    }
    if block.results.len() != block.transactions.len() {
        tracing::warn!(
            height,
            block_hash = %block.hash(),
            transactions = block.transactions.len(),
            results = block.results.len(),
            "block results count does not match transaction count"
        );
        return Err(ChainError::ResultsCountMismatch);
    }
    if block.reward_transaction.is_some() != block.reward_transaction_result.is_some() {
        tracing::warn!(
            height,
            block_hash = %block.hash(),
            "block has exactly one of reward_transaction / reward_transaction_result"
        );
        return Err(ChainError::InconsistentReward);
    }
    if block.header.height > 0 && !block.header.meets_difficulty(difficulty) {
        tracing::warn!(
            height,
            block_hash = %block.hash(),
            difficulty,
            "block fails PoW difficulty check"
        );
        return Err(ChainError::PowCheckFailed);
    }
    let transactions_root = roots::compute_transactions_root(&block.transactions);
    if transactions_root != block.header.transactions_root {
        tracing::warn!(
            height,
            block_hash = %block.hash(),
            expected = %block.header.transactions_root,
            computed = %transactions_root,
            "block has invalid transactions root"
        );
        return Err(ChainError::TransactionsRootMismatch);
    }
    let expected_reward_root = block
        .reward_transaction
        .as_ref()
        .map(|tx| tx.transaction().digest());
    if expected_reward_root != block.header.reward_root {
        tracing::warn!(
            height,
            block_hash = %block.hash(),
            in_header = ?block.header.reward_root,
            from_tx = ?expected_reward_root,
            "block has invalid reward root"
        );
        return Err(ChainError::RewardRootMismatch);
    }
    Ok(())
}

/// Validate and re-execute the block reward transaction, then apply it to the store.
/// Returns `false` on any validation or execution mismatch without mutating the store.
fn apply_block_reward_transaction(
    reward_transaction: &SignedTransaction,
    reward_execution_result: &ExecutionResult,
    total_gas: u64,
    block_hash: &Digest,
    mining_block_hash: Digest,
    store: &mut Store,
    height: u64,
) -> bool {
    let reward_tx_digest = reward_transaction.transaction().digest();
    if let Err(e) = validator::validate_signed_transaction(reward_transaction) {
        tracing::warn!(height, %block_hash, %reward_tx_digest, error = %e, "invalid reward transaction signature");
        return false;
    }
    if !system_transactions::is_valid_reward_transaction(
        reward_transaction.transaction(),
        total_gas,
        mining_block_hash,
    ) {
        tracing::warn!(height, %block_hash, %reward_tx_digest, total_gas, "reward amount or target mismatch");
        return false;
    }
    let recomputed_result = match system_transactions::execute_reward_transaction(
        reward_transaction.transaction(),
        store,
    ) {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!(height, %block_hash, %reward_tx_digest, error = %e, "reward transaction re-execution failed");
            return false;
        }
    };
    if &recomputed_result != reward_execution_result {
        tracing::warn!(height, %block_hash, %reward_tx_digest, "reward result mismatch local execution");
        return false;
    }
    store.apply_execution_result(&recomputed_result);
    true
}
