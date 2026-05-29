//! Mining work unit produced by the miner and ground for a valid nonce off the hot path.

use std::sync::Arc;

use meow_nakamoto_types::{block::Block, block_header::BlockHeader};
use meow_types::{address::Address, keypair::KeyPair, transaction::SignedTransaction};
use meow_vm_adapter::{executor, external_context::ExternalContext, inputs_resolver};

use crate::{roots, store::Store, system_transactions};

/// Yield to the async runtime every this many nonces so the executor stays
/// responsive and an outer `select!` can cancel the grind promptly.
const YIELD_EVERY_N_NONCES: u64 = 1024;

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
    /// PoW difficulty: minimum number of leading zero bits required in a block hash.
    pub difficulty: u32,
    /// Keypair used to sign system transactions.
    pub miner_keypair: Arc<KeyPair>,
    /// Address derived from the signing keypair — used as the system transactions sender.
    pub miner_address: Address,
    /// Address that receives the minted reward coins.
    pub reward_address: Address,
}

impl MiningWork {
    /// 1. Grind `nonce` until `mining_hash()` meets `difficulty`.
    /// 2. Execute `batch` using `mining_hash()` as the randomness seed.
    /// 3. If any transactions fail (e.g. conflicting object versions within the batch),
    ///    update `transactions_root`, reset the nonce, and re-grind — because
    ///    `transactions_root` is part of the mining hash and the previously found nonce
    ///    is no longer valid for the reduced transaction set.
    /// 4. If total gas across surviving transactions is > 0, build and execute the block
    ///    reward transaction (`meow_coin::mint`) and apply it to the store.
    /// 5. Fill in `state_root` and return the completed block and resulting store state.
    ///
    /// Returns `None` when:
    /// - All transactions in the batch are dropped during execution (e.g. every gas coin was
    ///   already spent). The caller should discard the round and wait for the next
    ///   `prepare_round` call.
    /// - The entire u64 nonce space is exhausted without finding a valid nonce (only possible
    ///   with a difficulty setting that requires more than 64 leading zero bits, which is
    ///   never used in practice). The caller should discard the round.
    pub async fn grind(mut self) -> Option<(Block, Store)> {
        let mut surviving_batch = self.batch;

        loop {
            // Grind nonce for the current transactions_root.
            // Yield every YIELD_EVERY_N_NONCES nonces so the async runtime stays responsive
            // and the future can be cancelled promptly by an outer select!.
            self.header.nonce = 0;
            while !self.header.meets_difficulty(self.difficulty) {
                self.header.nonce = match self.header.nonce.checked_add(1) {
                    Some(n) => n,
                    None => {
                        tracing::debug!(
                            height = self.header.height,
                            "nonce space exhausted — discarding round"
                        );
                        return None;
                    }
                };

                if self.header.nonce.is_multiple_of(YIELD_EVERY_N_NONCES) {
                    tokio::task::yield_now().await;
                }
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
                        tracing::warn!(digest = %signed_transaction.transaction().digest(), error = %e, "transaction dropped during execution");
                    }
                }
            }

            if executed_txs.is_empty() {
                tracing::debug!(
                    height = self.header.height,
                    "all transactions dropped — discarding round"
                );
                return None;
            }

            // Check whether any transactions were dropped. If so, transactions_root
            // is now stale — the nonce found above commits to the wrong transaction
            // set and the block would be rejected by peers. Update the root and
            // re-grind so the header commits to the actual executed transaction set.
            let actual_transactions_root = roots::compute_transactions_root(&executed_txs);
            if actual_transactions_root == self.header.transactions_root {
                let total_reward: u64 = results.iter().map(|r| r.gas_used()).sum();
                let (reward_transaction, reward_transaction_result) = if total_reward > 0 {
                    let reward_transaction = system_transactions::make_reward_transaction(
                        self.miner_address,
                        self.reward_address,
                        total_reward,
                        self.header.mining_hash(),
                    );

                    let (signed_transaction, _) = reward_transaction.sign(&self.miner_keypair);

                    let reward_transaction_result =
                        system_transactions::execute_reward_transaction(
                            signed_transaction.transaction(),
                            &new_store,
                        )
                        .expect("block reward mint must not fail");

                    new_store.apply_execution_result(&reward_transaction_result);

                    (Some(signed_transaction), Some(reward_transaction_result))
                } else {
                    (None, None)
                };

                self.header.reward_root = reward_transaction
                    .as_ref()
                    .map(|tx| tx.transaction().digest());
                self.header.state_root = roots::compute_state_root(&new_store);

                let block = Block {
                    header: self.header,
                    transactions: executed_txs,
                    results,
                    reward_transaction,
                    reward_transaction_result,
                };

                return Some((block, new_store));
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
