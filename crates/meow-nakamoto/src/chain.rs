use std::collections::BTreeMap;

use meow_nakamoto_types::block::Block;
use meow_types::{
    digest::Digest,
    object::Object,
    time,
    transaction::{SignedTransaction, execution_result::ExecutionResult, validator},
};
use meow_vm_adapter::{executor, external_context::ExternalContext, inputs_resolver};

use crate::store::Store;

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

/// How many block snapshots to keep behind the head. Limits reorg depth.
const SNAPSHOT_DEPTH: u64 = 64;
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
    /// the new store; this method simply records them and advances the head.
    pub fn commit(&mut self, block: Block, new_store: Store) {
        let hash = block.hash();
        for result in &block.results {
            self.results
                .insert(*result.transaction_digest(), result.clone());
        }
        self.blocks.insert(hash, block);
        self.snapshots.insert(hash, new_store);
        self.head = hash;
        self.prune_old_snapshots();
    }

    /// Drop store snapshots more than `SNAPSHOT_DEPTH` blocks behind the head.
    /// Block headers are retained for chain validation; only the (large) Store
    /// clones are freed.
    fn prune_old_snapshots(&mut self) {
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
        }
    }

    /// Process a block received from a peer.
    ///
    /// Validates:
    /// - parent block is known (we have it)
    /// - PoW hash meets difficulty
    /// - height is exactly parent height + 1
    /// - transactions root matches block transactions
    /// - all transaction signatures are valid
    /// - local deterministic execution results match block results
    /// - resulting state root matches block header
    ///
    /// If the block extends a chain longer than the current head, the head
    /// is updated (chain reorganization). Returns `true` when the head changed.
    pub fn apply_block(&mut self, block: Block) -> bool {
        let block_hash = block.hash();

        // Already known — skip.
        if self.blocks.contains_key(&block_hash) {
            return false;
        }

        let parent_hash = block.header.parent_hash;
        let height = block.header.height;

        // Parent must be known; otherwise we'd need to request the missing ancestors.
        if !self.blocks.contains_key(&parent_hash) {
            tracing::debug!(height, "block with unknown parent — ignoring");
            return false;
        }

        // Genesis has no PoW; all subsequent blocks must satisfy difficulty.
        if height > 0 && !block.header.meets_difficulty(self.difficulty) {
            tracing::warn!(height, "block fails PoW difficulty check — ignoring");
            return false;
        }

        // Height must be exactly parent + 1.
        let parent_height = self.blocks[&parent_hash].header.height;
        if height != parent_height + 1 {
            tracing::warn!(
                height,
                expected = parent_height + 1,
                "block has wrong height — ignoring"
            );
            return false;
        }

        let timestamp = block.header.timestamp;

        // Timestamp must be strictly greater than the parent's to ensure time
        // only moves forward — important for contracts that read the block time.
        let parent_timestamp = self.blocks[&parent_hash].header.timestamp;
        if timestamp <= parent_timestamp {
            tracing::warn!(
                height,
                timestamp,
                parent_timestamp,
                "block timestamp is not greater than parent — ignoring"
            );
            return false;
        }

        // Reject blocks stamped too far in the future to prevent miners from
        // manipulating the clock to unlock time-sensitive contract logic early.
        let now = time::current_timestamp();
        if timestamp > now + MAX_BLOCK_FUTURE_DRIFT_MS {
            tracing::warn!(
                height,
                timestamp,
                now,
                "block timestamp is too far in the future — ignoring"
            );
            return false;
        }

        // Transactions root must match the transaction list in the block.
        let transactions_root = compute_transactions_root(&block.transactions);
        if transactions_root != block.header.transactions_root {
            tracing::warn!(height, "block has invalid transactions root — ignoring");
            return false;
        }

        for signed_transaction in &block.transactions {
            if let Err(e) = validator::validate_signed_transaction(signed_transaction) {
                tracing::warn!(height, error = %e, "block has invalid transaction signature");
                return false;
            }
        }

        // Build the store snapshot for this block by deterministically re-executing all transactions.
        let mut new_store = self.snapshots[&parent_hash].clone();
        let mut expected_results = Vec::with_capacity(block.transactions.len());

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
                    tracing::warn!(height, error = %e, "block execution failed during verification");
                    return false;
                }
            };

            new_store.apply_execution_result(&result);
            expected_results.push(result);
        }

        if expected_results != block.results {
            tracing::warn!(height, "block results mismatch local execution — ignoring");
            return false;
        }

        let expected_state_root = compute_state_root(&new_store);
        if expected_state_root != block.header.state_root {
            tracing::warn!(height, "block has invalid state root — ignoring");
            return false;
        }

        for result in &expected_results {
            self.results
                .insert(*result.transaction_digest(), result.clone());
        }

        self.blocks.insert(block_hash, block);
        self.snapshots.insert(block_hash, new_store);

        // Switch head to this block if it's on a longer chain.
        if height > self.head_height() {
            tracing::info!(height, "chain reorg: switching to longer chain");
            self.head = block_hash;
            self.prune_old_snapshots();
            return true;
        }

        false
    }

    /// Returns all blocks from the given height onwards (in height order).
    pub fn get_blocks_since(&self, height: u64) -> Vec<Block> {
        self.blocks
            .values()
            .filter(|b| b.header.height >= height)
            .cloned()
            .collect()
    }
}

/// Deterministic hash of the object store's current state.
/// Used as `state_root` in block headers.
pub fn compute_state_root(store: &Store) -> Digest {
    let objects: Vec<&Object> = store.objects().collect();
    Digest::compute(&objects).expect("state root serialization is infallible")
}

/// Hash over all transaction digests in order.
/// Used as `transactions_root` in block headers.
pub fn compute_transactions_root(txs: &[SignedTransaction]) -> Digest {
    let digests: Vec<Digest> = txs.iter().map(|tx| tx.transaction().digest()).collect();
    Digest::compute(&digests).expect("transactions root serialization is infallible")
}
