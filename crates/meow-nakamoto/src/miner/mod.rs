pub mod error;

use meow_genesis::Genesis;
use meow_nakamoto_types::{block::Block, block_header::BlockHeader, miner_config::MinerConfig};
use meow_types::{
    digest::Digest,
    object::Object,
    transaction::{
        SignedTransaction, Transaction, execution_result::ExecutionResult, input::Input,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::{executor, external_context::ExternalContext};

use crate::{
    chain::{ChainState, compute_state_root, compute_transactions_root},
    mempool::Mempool,
    miner::error::MinerError,
    store::Store,
    utils,
};

/// The result type related to the miner.
pub type Result<T> = std::result::Result<T, MinerError>;

const BATCH_SIZE: usize = 100;

/// Transaction processor and PoW miner.
///
/// Owns the [`ChainState`] (full block history + store snapshots) and the
/// [`Mempool`]. The outer `Arc<Mutex<Miner>>` in `meow-node` provides
/// shared access for the RPC server and gossip event loop.
pub struct Miner {
    chain: ChainState,
    mempool: Mempool,
}

impl Miner {
    /// Creates a new `Miner` with the given configuration.
    pub fn empty(config: MinerConfig) -> Self {
        Self {
            chain: ChainState::new(Store::default(), config.difficulty),
            mempool: Mempool::empty(),
        }
    }

    /// Creates a new `Miner` pre-seeded with the given genesis state.
    pub fn with_genesis(genesis: &Genesis, config: MinerConfig) -> Self {
        let store = Store::with_objects(genesis.objects().iter().cloned());
        Self {
            chain: ChainState::new(store, config.difficulty),
            mempool: Mempool::empty(),
        }
    }

    /// Returns a reference to the current head store for transaction validation and execution.
    pub fn head_store(&self) -> &Store {
        self.chain.head_store()
    }

    /// Returns the height of the current best block.
    pub fn head_height(&self) -> u64 {
        self.chain.head_height()
    }

    /// Look up an execution result by transaction digest.
    pub fn get_transaction_result(&self, digest: &Digest) -> Option<&ExecutionResult> {
        self.chain.get_transaction_result(digest)
    }

    /// Look up a committed transaction by digest.
    pub fn get_transaction(&self, digest: &Digest) -> Option<&SignedTransaction> {
        self.chain.get_transaction(digest)
    }

    /// Get all blocks from the given height onwards.
    pub fn get_blocks_since(&self, height: u64) -> Vec<Block> {
        self.chain.get_blocks_since(height)
    }

    /// Validate and enqueue a transaction. Internally clones the head store
    /// so that the immutable borrow on `chain` and the mutable borrow on
    /// `mempool` do not overlap.
    pub fn submit_tx(&mut self, tx: SignedTransaction) -> Result<()> {
        let store = self.chain.head_store().clone();
        Ok(self.mempool.submit(tx, &store)?)
    }

    /// Apply a block received from a peer. Returns `true` if the head changed.
    ///
    /// On a chain reorg, transactions still valid against the new head
    /// store are kept; only those referencing stale object versions are dropped.
    pub fn apply_block(&mut self, block: Block) -> bool {
        let reorged = self.chain.apply_block(block);
        if reorged {
            let store = self.chain.head_store();
            self.mempool.retain_valid(store);
        }
        reorged
    }

    /// Drain the mempool and execute a batch of transactions against the current
    /// head, returning the work needed to grind a valid nonce.
    ///
    /// Returns `None` if the mempool is empty.
    pub fn prepare_round(&mut self) -> Option<MiningWork> {
        let batch = self.mempool.drain_batch(BATCH_SIZE);
        if batch.is_empty() {
            return None;
        }

        let parent_hash = self.chain.head();
        let height = self.chain.head_height() + 1;
        let difficulty = self.chain.difficulty();
        let parent_store = self.chain.head_store().clone();

        // transactions_root is computed upfront so it can be included in the
        // mining hash — committing the miner to this exact transaction set before
        // grinding begins. state_root is still unknown until after execution.
        let transactions_root = compute_transactions_root(&batch);

        Some(MiningWork {
            header: BlockHeader {
                height,
                parent_hash,
                transactions_root,
                // State root is unknown until after execution, so set to ZERO for now.
                state_root: meow_types::digest::Digest::ZERO,
                timestamp: utils::current_timestamp(),
                nonce: 0,
            },
            batch,
            parent_store,
            difficulty,
        })
    }

    /// Commit a mined block if the chain head has not changed since `prepare_round`.
    ///
    /// Returns `false` (and discards the block) if another block was committed
    /// or received while the nonce was being ground.
    pub fn commit_mined(&mut self, block: Block, new_store: Store) -> bool {
        if block.header.parent_hash != self.chain.head() {
            tracing::debug!(
                height = block.header.height,
                "mined block is stale (chain advanced) — discarding"
            );
            return false;
        }
        self.chain.commit(block, new_store);
        true
    }
}

/// Work produced by [`Miner::prepare_round`]. Grind the nonce outside the
/// `Miner` lock so that RPC and gossip handlers can run concurrently.
pub struct MiningWork {
    /// Header with `transactions_root` already set and `nonce = 0`.
    /// `state_root` is zeroed — `grind()` fills it in after execution.
    /// `grind()` finds the valid nonce, executes transactions using
    /// `mining_hash()` as the randomness seed, then sets `state_root`.
    pub header: BlockHeader,
    /// Transactions drained from the mempool, pending execution.
    pub batch: Vec<SignedTransaction>,
    /// Object store snapshot at the parent block tip.
    pub parent_store: Store,
    pub difficulty: u32,
}

impl MiningWork {
    /// 1. Grind `nonce` until `mining_hash()` meets `difficulty`.
    /// 2. Execute `batch` using `mining_hash()` as the randomness seed.
    /// 3. Fill in `state_root` (transactions_root was set in `prepare_round`).
    /// 4. Return the completed block and the resulting store state.
    pub fn grind(mut self) -> (Block, Store) {
        while !self.header.meets_difficulty(self.difficulty) {
            self.header.nonce += 1;
        }

        tracing::debug!(
            height = self.header.height,
            nonce = self.header.nonce,
            "PoW solved"
        );

        // Nonce is now final — mining_hash() is the committed randomness seed.
        let execution_context =
            ExternalContext::new(self.header.mining_hash().into(), self.header.timestamp);

        let mut new_store = self.parent_store;
        let mut executed_txs: Vec<SignedTransaction> = Vec::new();
        let mut results = Vec::new();

        for signed_tx in self.batch {
            let tx = signed_tx.transaction();
            let inputs = resolve_inputs(tx, &new_store);
            match executor::execute(tx, inputs, &execution_context) {
                Ok(result) => {
                    new_store.apply_execution_result(&result);
                    results.push(result);
                    executed_txs.push(signed_tx);
                }
                Err(e) => {
                    tracing::warn!(digest = ?signed_tx.transaction().digest(), error = %e, "transaction dropped during execution");
                }
            }
        }

        // transactions_root was already set in prepare_round; only state_root
        // is unknown until execution completes.
        self.header.state_root = compute_state_root(&new_store);

        let block = Block {
            header: self.header,
            transactions: executed_txs,
            results,
        };
        (block, new_store)
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
