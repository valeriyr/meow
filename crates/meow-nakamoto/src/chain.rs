use std::collections::BTreeMap;

use meow_types::{
    digest::Digest,
    object::Object,
    transaction::{
        self, SignedTransaction, Transaction, execution_result::ExecutionResult, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::executor;

use crate::{block::Block, store::Store};

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
    pub fn get_result(&self, digest: &Digest) -> Option<&ExecutionResult> {
        self.results.get(digest)
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
    pub fn on_block_received(&mut self, block: Block) -> bool {
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

        // Transactions root must match the transaction list in the block.
        let transactions_root = compute_transactions_root(&block.transactions);
        if transactions_root != block.header.transactions_root {
            tracing::warn!(height, "block has invalid transactions root — ignoring");
            return false;
        }

        for tx in &block.transactions {
            if let Err(e) = transaction::validator::validate_signed_transaction(tx) {
                tracing::warn!(height, "block has invalid transaction signature: {e}");
                return false;
            }
        }

        // Build the store snapshot for this block by deterministically re-executing all transactions.
        let mut new_store = self.snapshots[&parent_hash].clone();
        let mut expected_results = Vec::with_capacity(block.transactions.len());

        for signed_tx in &block.transactions {
            let tx = signed_tx.transaction();
            let inputs = resolve_inputs(tx, &new_store);
            let result = match executor::execute(tx, inputs) {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!(height, "block execution failed during verification: {e}");
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
            tracing::info!(height, "chain reorganization: switching to longer chain");
            self.head = block_hash;
            self.prune_old_snapshots();
            return true;
        }

        false
    }
}

/// Collect all objects the transaction needs from the store.
fn resolve_inputs(tx: &Transaction, store: &Store) -> Vec<Object> {
    let mut inputs = Vec::new();

    if let Some(coin) = store.get_object(tx.gas_coin().address()) {
        inputs.push(coin.clone());
    }

    if let TransactionType::MeowCall(call) = tx.type_() {
        if let Some(module) = store.get_object(call.module()) {
            inputs.push(module.clone());
        }
        for arg in call.arguments() {
            if let Input::Object(obj_ref) = arg
                && let Some(obj) = store.get_object(obj_ref.address())
            {
                inputs.push(obj.clone());
            }
        }
    }

    inputs
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
