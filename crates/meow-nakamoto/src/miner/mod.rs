pub mod error;

use std::time::{SystemTime, UNIX_EPOCH};

use meow_types::{
    digest::Digest,
    object::Object,
    transaction::{
        SignedTransaction, Transaction, call::Input, execution_result::ExecutionResult,
        transaction_type::TransactionType,
    },
};
use meow_vm_adapter::executor;

use crate::{
    block::{Block, BlockHeader},
    chain::{ChainState, compute_state_root, compute_transactions_root},
    mempool::Mempool,
    miner::error::MinerError,
    store::Store,
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
    /// Creates a new `Miner` with the given initial store state and PoW difficulty.
    pub fn new(initial_store: Store, mempool: Mempool, difficulty: u32) -> Self {
        Self {
            chain: ChainState::new(initial_store, difficulty),
            mempool,
        }
    }

    /// Returns a reference to the current head store for transaction validation and execution.
    pub fn head_store(&self) -> &Store {
        self.chain.head_store()
    }

    /// Look up an execution result by transaction digest.
    pub fn get_result(&self, digest: &Digest) -> Option<&ExecutionResult> {
        self.chain.get_result(digest)
    }

    /// Validate and enqueue a transaction. Internally clones the head store
    /// so that the immutable borrow on `chain` and the mutable borrow on
    /// `mempool` do not overlap.
    pub fn submit_tx(&mut self, tx: SignedTransaction) -> Result<()> {
        let store = self.chain.head_store().clone();
        Ok(self.mempool.submit(tx, &store)?)
    }

    /// Apply a block received from a peer. Returns `true` if the head changed.
    pub fn on_block_received(&mut self, block: Block) -> bool {
        self.chain.on_block_received(block)
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

        let mut new_store = self.chain.head_store().clone();
        let mut executed_txs: Vec<SignedTransaction> = Vec::new();
        let mut results = Vec::new();

        for signed_tx in batch {
            let tx = signed_tx.transaction();
            let inputs = resolve_inputs(tx, &new_store);
            match executor::execute(tx, inputs) {
                Ok(result) => {
                    new_store.apply_execution_result(&result);
                    results.push(result);
                    executed_txs.push(signed_tx);
                }
                Err(e) => {
                    tracing::warn!("transaction dropped: {e}");
                }
            }
        }

        let transactions_root = compute_transactions_root(&executed_txs);
        let state_root = compute_state_root(&new_store);

        Some(MiningWork {
            header: BlockHeader {
                height,
                parent_hash,
                transactions_root,
                state_root,
                timestamp: current_timestamp(),
                nonce: 0,
            },
            transactions: executed_txs,
            results,
            new_store,
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
    /// Header with `nonce = 0`; grind until it meets `difficulty`.
    pub header: BlockHeader,
    pub transactions: Vec<SignedTransaction>,
    pub results: Vec<ExecutionResult>,
    pub new_store: Store,
    pub difficulty: u32,
}

impl MiningWork {
    /// Increment `nonce` until the header hash meets `difficulty`, then return
    /// the completed block and the resulting store state.
    pub fn grind(mut self) -> (Block, Store) {
        while !self.header.meets_difficulty(self.difficulty) {
            self.header.nonce += 1;
        }

        tracing::debug!(
            height = self.header.height,
            nonce = self.header.nonce,
            "PoW solved"
        );

        let block = Block {
            header: self.header,
            transactions: self.transactions,
            results: self.results,
        };
        (block, self.new_store)
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

/// Current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
