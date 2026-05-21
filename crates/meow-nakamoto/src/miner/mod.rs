//! Nakamoto PoW miner: builds candidate blocks and grinds for a valid nonce.

pub mod error;

use meow_genesis::Genesis;
use meow_nakamoto_types::{block::Block, block_header::BlockHeader, miner_config::MinerConfig};
use meow_types::{
    digest::Digest,
    time,
    transaction::{SignedTransaction, Transaction, execution_result::ExecutionResult},
};
use meow_vm_adapter::{executor, external_context::ExternalContext, inputs_resolver};

use crate::{
    chain::{ChainState, compute_state_root, compute_transactions_root},
    mempool::{self, Mempool},
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
    pub fn submit_transaction(&mut self, signed_transaction: SignedTransaction) -> Result<()> {
        let store = self.chain.head_store().clone();
        Ok(self.mempool.submit(signed_transaction, &store)?)
    }

    /// Validate object refs and execute a transaction without committing it.
    /// Accepts an unsigned transaction — no signature required for simulation.
    pub fn simulate_transaction(&mut self, transaction: Transaction) -> Result<ExecutionResult> {
        let store = self.chain.head_store().clone();
        let header = self.chain.head_block().header.clone();

        mempool::validate_against_store(&transaction, &store)?;

        let execution_context = ExternalContext::new(header.mining_hash().into(), header.timestamp);

        let inputs = inputs_resolver::collect_inputs(&transaction, |address| {
            store.get_object(address).cloned()
        });

        let result = executor::execute(&transaction, inputs, &execution_context)?;

        Ok(result)
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
                state_root: Digest::ZERO,
                timestamp: time::current_timestamp(),
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
    /// 3. If any transactions fail (e.g. conflicting object versions within the batch),
    ///    update `transactions_root`, reset the nonce, and re-grind — because
    ///    `transactions_root` is part of the mining hash and the previously found nonce
    ///    is no longer valid for the reduced transaction set.
    /// 4. Fill in `state_root` and return the completed block and resulting store state.
    pub fn grind(mut self) -> (Block, Store) {
        let mut surviving_batch = self.batch;

        loop {
            // Grind nonce for the current transactions_root.
            self.header.nonce = 0;
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

            let mut new_store = self.parent_store.clone();
            let mut executed_txs: Vec<SignedTransaction> = Vec::new();
            let mut results = Vec::new();

            for signed_transaction in &surviving_batch {
                let transaction = signed_transaction.transaction();
                let inputs = inputs_resolver::collect_inputs(transaction, |addr| {
                    new_store.get_object(addr).cloned()
                });
                match executor::execute(transaction, inputs, &execution_context) {
                    Ok(result) => {
                        new_store.apply_execution_result(&result);
                        results.push(result);
                        executed_txs.push(signed_transaction.clone());
                    }
                    Err(e) => {
                        tracing::warn!(digest = ?signed_transaction.transaction().digest(), error = %e, "transaction dropped during execution");
                    }
                }
            }

            // Check whether any transactions were dropped. If so, transactions_root
            // is now stale — the nonce found above commits to the wrong transaction
            // set and the block would be rejected by peers. Update the root and
            // re-grind so the header commits to the actual executed transaction set.
            let actual_transactions_root = compute_transactions_root(&executed_txs);
            if actual_transactions_root == self.header.transactions_root {
                self.header.state_root = compute_state_root(&new_store);
                let block = Block {
                    header: self.header,
                    transactions: executed_txs,
                    results,
                };
                return (block, new_store);
            }

            tracing::debug!(
                height = self.header.height,
                dropped = surviving_batch.len() - executed_txs.len(),
                "transactions dropped; updating transactions_root and re-grinding"
            );
            self.header.transactions_root = actual_transactions_root;
            surviving_batch = executed_txs;
        }
    }
}
