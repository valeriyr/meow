# Consensus

> Nakamoto proof-of-work consensus: block structure, validation rules, fork choice, and on-chain entropy.

## Overview

MEOW uses Nakamoto-style proof-of-work. Miners continuously grind a nonce until the block's mining hash satisfies a difficulty target, then broadcast the solved block to peers. Every node independently re-executes all transactions and verifies the result — a block is only accepted when it passes every validation rule.

The fork choice rule is **longest chain by height**. When two valid chains exist, the one with the higher block height wins. The maximum safe reorg depth is **64 blocks** (snapshot retention limit).

---

## Block structure

A `BlockHeader` contains six fields:

| Field | Type | Description |
|-------|------|-------------|
| `height` | `u64` | Block number; genesis = 0 |
| `parent_hash` | `Digest` | Blake2b-256 hash of the full parent `BlockHeader` |
| `transactions_root` | `Digest` | Blake2b-256 hash over all transaction digests in order |
| `state_root` | `Digest` | Blake2b-256 hash of the object store after applying every transaction |
| `timestamp` | `u64` | Unix milliseconds at the time of mining |
| `nonce` | `u64` | Grinding value that makes the mining hash satisfy the PoW target |

Two hashes are derived from a block header:

- **Block hash** — Blake2b-256 of the BCS-serialized full header (all six fields). Used as the block's identity and stored in the next block's `parent_hash`.
- **Mining hash** — Blake2b-256 of a reduced projection containing only `height`, `parent_hash`, `transactions_root`, `timestamp`, and `nonce` — **excluding `state_root`**. This is the value checked against the PoW target and used as the VM randomness seed.

`state_root` is excluded from the mining hash because it is only known after re-executing every transaction — which requires the nonce to already be fixed. `transactions_root` is included so the miner commits to the exact transaction set before grinding begins and cannot swap transactions after finding a favorable nonce.

---

## Proof of work

The PoW target is expressed as a **minimum number of leading zero bits** in the mining hash. Difficulty `d` means the first `d` bits of the mining hash must all be zero. A difficulty of 0 accepts any hash.

Mining proceeds as follows:

1. Assemble the transaction batch (up to 100 transactions from the mempool).
2. Compute `transactions_root` from the batch — it is fixed before grinding starts.
3. Set `timestamp` and `nonce = 0`.
4. Increment `nonce` until `mining_hash` has at least `difficulty` leading zero bits.
5. Re-execute all transactions using the solved `mining_hash` as the randomness seed.
6. Compute `state_root` from the resulting object store.
7. Broadcast the completed block.

If the chain advances while grinding (another block arrives), the work-in-progress block is **discarded as stale** and a new round starts from the new head.

---

## Block validation

`apply_block` enforces the following rules in order. Any failure causes the block to be silently rejected.

1. **No duplicate** — if the block hash is already known, skip (idempotent).
2. **Parent known** — `parent_hash` must refer to a block already in the chain.
3. **PoW** — for `height > 0`, `mining_hash` must have at least `difficulty` leading zero bits. Genesis (`height == 0`) is exempt.
4. **Height continuity** — `height` must equal `parent_height + 1`.
5. **Timestamp monotonicity** — `timestamp > parent_timestamp`.
6. **Timestamp future drift** — `timestamp <= local_clock + 120_000` ms (2-minute limit).
7. **Transactions root** — `transactions_root` must match the hash computed from the block's transaction list.
8. **Signature validity** — every transaction must carry a valid signature.
9. **Re-execution match** — transactions are re-executed against the parent store using `mining_hash` as the randomness seed and `timestamp` as the block time; the resulting `Vec<ExecutionResult>` must equal the results included in the block exactly.
10. **State root** — the `state_root` in the header must match the hash of the object store produced by re-execution.

---

## Timestamps

The block timestamp is set by the miner and recorded in the header as Unix milliseconds.

**Validation rules:**

- Must be strictly greater than the parent block's timestamp.
- Must not exceed the node's local clock by more than **two minutes**.

Because the miner has discretion within these bounds, contracts that depend on `meow_vm_timestamp()` should build in tolerance of minutes rather than seconds. See [Contracts — Time](contracts.md#time) for usage patterns.

---

## Randomness

`meow_vm_rand()` draws from a per-transaction pseudo-random sequence seeded from the **mining hash** combined with the transaction digest. Because the mining hash commits to `height`, `parent_hash`, `transactions_root`, `timestamp`, and the solved `nonce`, the seed is:

- **Deterministic across re-executions** — every node running the same block gets the same sequence.
- **Unknown at submission time** — the nonce is only determined after your transaction is already in the mempool.
- **Per-transaction** — different transaction digests yield independent sequences even within the same block.

**Miner-bias attack:** a miner can inspect the random outcome after solving PoW and, if unfavorable, discard the block and re-mine. Each attempt costs one block reward, so manipulation is only rational for high-value outcomes.

`meow_vm_rand()` is suitable for low-stakes game mechanics or cosmetic variation. Avoid it where a rational miner would profit from biasing the result. See [Adapter — Native functions](adapter.md#native-functions) for the call signature.

---

## Fork choice and reorgs

**Fork choice rule:** the chain with the greatest `height` wins.

When a new block arrives that extends a side chain to a height greater than the current head, the node switches to that chain. Re-execution is not needed during the reorg — every block keeps a pre-computed snapshot of the object store, so a fork switch is a pointer update.

**Snapshot pruning:** store snapshots for blocks more than **64 heights** behind the current head are discarded. Block headers and transaction data are kept indefinitely. The practical maximum safe reorg depth is 64 blocks.

---

## Mempool

### Submission

A transaction is accepted into the mempool only if:

1. The signature is valid.
2. The transaction digest has not been seen before (deduplication).
3. Every input object reference (including the gas coin) matches an object in the current head store by both version and digest.

### Ordering

Transactions are queued FIFO. There is no fee-based priority ordering.

### Reorg retention

When the chain head changes, the mempool is filtered against the new head store. Transactions whose object references are still valid are kept; those referencing stale or non-existent objects are evicted. Signature validity is not re-checked.

---

## Genesis

The genesis block has `height = 0` and is bootstrap-loaded from a genesis file. It is exempt from PoW — no difficulty check is applied to `height == 0`.
