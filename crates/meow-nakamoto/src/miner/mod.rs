//! Nakamoto PoW miner: builds candidate blocks and grinds for a valid nonce.

pub mod error;
pub mod mining_work;

use std::sync::Arc;

use meow_genesis::Genesis;
use meow_nakamoto_types::{block::Block, block_header::BlockHeader, miner_config::MinerConfig};
use meow_types::{
    address::Address,
    digest::Digest,
    keypair::KeyPair,
    time,
    transaction::{SignedTransaction, Transaction, execution_result::ExecutionResult},
};
use meow_vm_adapter::{executor, external_context::ExternalContext, inputs_resolver};

use crate::{
    chain::ChainState,
    mempool::{self, Mempool},
    miner::{error::MinerError, mining_work::MiningWork},
    roots,
    store::Store,
};

/// The result type related to the miner.
pub type Result<T> = std::result::Result<T, MinerError>;

/// Maximum number of transactions drained from the mempool per mining round.
const BATCH_SIZE: usize = 100;

/// Transaction processor and PoW miner.
///
/// Owns the [`ChainState`] (full block history + store snapshots) and the
/// [`Mempool`]. The outer `Arc<Mutex<Miner>>` in `meow-node` provides
/// shared access for the RPC server and gossip event loop.
pub struct Miner {
    chain: ChainState,
    mempool: Mempool,
    /// Keypair used to sign system transactions.
    keypair: Arc<KeyPair>,
    /// Address derived from `keypair` — used as the system transactions sender.
    miner_address: Address,
    /// Address that receives the minted reward coins.
    reward_address: Address,
}

impl Miner {
    /// Creates a new `Miner` with the given configuration.
    pub fn empty(config: MinerConfig) -> Self {
        let miner_address = Address::from(&config.keypair);
        Self {
            chain: ChainState::new(Store::default(), config.difficulty),
            mempool: Mempool::empty(),
            keypair: Arc::new(config.keypair),
            miner_address,
            reward_address: config.reward_address,
        }
    }

    /// Creates a new `Miner` pre-seeded with the given genesis state.
    pub fn with_genesis(genesis: &Genesis, config: MinerConfig) -> Self {
        let miner_address = Address::from(&config.keypair);
        let store = Store::with_objects(genesis.objects().iter().cloned());
        Self {
            chain: ChainState::new(store, config.difficulty),
            mempool: Mempool::empty(),
            keypair: Arc::new(config.keypair),
            miner_address,
            reward_address: config.reward_address,
        }
    }

    /// Returns the address used to sign system transactions (derived from the miner keypair).
    pub fn miner_address(&self) -> Address {
        self.miner_address
    }

    /// Returns the address that receives the minted reward coins.
    pub fn reward_address(&self) -> Address {
        self.reward_address
    }

    /// Returns a reference to the current head store for transaction validation and execution.
    pub fn head_store(&self) -> &Store {
        self.chain.head_store()
    }

    /// Hash of the current best block.
    pub fn head(&self) -> Digest {
        self.chain.head()
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
        let transactions_root = roots::compute_transactions_root(&batch);

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
            miner_keypair: Arc::clone(&self.keypair),
            miner_address: self.miner_address,
            reward_address: self.reward_address,
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
