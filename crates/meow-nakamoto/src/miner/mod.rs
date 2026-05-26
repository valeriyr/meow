//! Nakamoto PoW miner: builds candidate blocks and grinds for a valid nonce.

pub mod error;
pub mod mining_work;

use std::sync::Arc;

use meow_genesis::Genesis;
use meow_nakamoto_types::{
    block::Block, block_header::BlockHeader, miner_config::MinerConfig,
    state_snapshot::StateSnapshot,
};
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
    /// Mining starts once this many transactions are queued; a block may still
    /// end up with fewer if some are dropped during execution.
    batch_size: usize,
}

impl Miner {
    /// Creates a new `Miner` with the given configuration.
    pub fn empty(config: MinerConfig) -> Self {
        let miner_address = Address::from(&config.keypair);
        Self {
            chain: ChainState::new(Store::default(), config.difficulty, config.snapshot_depth),
            mempool: Mempool::empty(),
            keypair: Arc::new(config.keypair),
            miner_address,
            reward_address: config.reward_address,
            batch_size: config.batch_size,
        }
    }

    /// Creates a new `Miner` pre-seeded with the given genesis state.
    pub fn with_genesis(genesis: &Genesis, config: MinerConfig) -> Self {
        let miner_address = Address::from(&config.keypair);
        let store = Store::with_objects(genesis.objects().iter().cloned());
        Self {
            chain: ChainState::new(store, config.difficulty, config.snapshot_depth),
            mempool: Mempool::empty(),
            keypair: Arc::new(config.keypair),
            miner_address,
            reward_address: config.reward_address,
            batch_size: config.batch_size,
        }
    }

    /// Returns the address used to sign system transactions (derived from the miner keypair).
    pub fn miner_address(&self) -> Address {
        self.miner_address
    }

    /// Earliest block height from which a sync should start to cover all resolvable reorgs.
    /// See [`ChainState::sync_from_height`] for details.
    pub fn sync_from_height(&self) -> u64 {
        self.chain.sync_from_height()
    }

    /// Number of block snapshots retained behind the head — the maximum safe reorg depth.
    pub fn snapshot_depth(&self) -> u64 {
        self.chain.snapshot_depth()
    }

    /// Number of transactions to accumulate in the mempool before starting a mining round.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Returns the address that receives the minted reward coins.
    pub fn reward_address(&self) -> Address {
        self.reward_address
    }

    /// Returns a reference to the current head store for transaction validation and execution.
    pub fn head_store(&self) -> &Store {
        self.chain.head_store()
    }

    /// The current best block.
    pub fn head_block(&self) -> &Block {
        self.chain.head_block()
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

    /// Returns a full state snapshot at the current head.
    pub fn get_state_snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            head: self.chain.head_block().clone(),
            objects: self.chain.head_store().objects().cloned().collect(),
        }
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

    /// Replace the entire chain state from a peer-supplied snapshot.
    ///
    /// Delegates all validation to [`ChainState::from_snapshot`]. On success the
    /// chain is anchored at the snapshot block and the mempool is cleared — all
    /// pending transactions reference object versions from the old chain.
    pub fn replace_from_snapshot(&mut self, snapshot: StateSnapshot) -> Result<()> {
        self.chain = ChainState::from_snapshot(
            self.chain.head_height(),
            snapshot.head,
            Store::with_objects(snapshot.objects),
            self.chain.difficulty(),
            self.chain.snapshot_depth(),
        )?;
        self.mempool = Mempool::empty();
        Ok(())
    }

    /// Apply a block received from a peer.
    ///
    /// Returns `Ok(true)` when the head changed (reorg), `Ok(false)` when the
    /// block was valid but did not extend the longest chain, or `Err` when the
    /// block was rejected.
    ///
    /// On a chain reorg, transactions still valid against the new head
    /// store are kept; only those referencing stale object versions are dropped.
    pub fn apply_block(&mut self, block: Block) -> Result<bool> {
        let reorged = self.chain.apply_block(block)?;
        if reorged {
            let store = self.chain.head_store();
            self.mempool.retain_valid(store);
        }
        Ok(reorged)
    }

    /// Drain the mempool and execute a batch of transactions against the current
    /// head, returning the work needed to grind a valid nonce.
    ///
    /// Returns `None` if the mempool has fewer than `batch_size` transactions.
    pub fn prepare_round(&mut self) -> Option<MiningWork> {
        if self.mempool.len() < self.batch_size {
            return None;
        }
        let batch = self.mempool.drain_batch(self.batch_size);

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
                // Reward root is unknown until after execution.
                reward_root: None,
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
                block_hash = %block.hash(),
                chain_head = %self.chain.head(),
                "mined block is stale (chain advanced) — discarding"
            );
            return false;
        }
        self.chain.commit(block, new_store);
        true
    }
}
