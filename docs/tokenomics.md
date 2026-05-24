# Tokenomics

> How Meow Coin is created, spent, and redistributed.

## Overview

[Meow Coin](meow-coin.md) is the single native asset of MEOW. The total supply is fixed at genesis — block rewards do not create new coins, they redistribute existing value. When a transaction pays gas, the sender's coin balance decreases by `gas_used`; the miner receives a new coin object with that same amount. The net change in total supply is zero.

Supply decreases only when users explicitly call `meow_coin::burn`.

---

## Initial supply

The genesis supply is defined by `allocations.csv`. Each row is an `(address, amount)` pair. The genesis builder calls the private `meow_coin::mint` once per row, creating one `MeowCoin` object per entry and transferring it to the specified address. A single address may appear on multiple rows; each row produces a separate coin object.

```
0xabc...123,1000000
0xdef...456,2000000
```

There are no vesting schedules or lock-ups in the protocol. All genesis coins are immediately spendable.

---

## Gas model

Every user transaction must reference a `MeowCoin` object as its gas coin. The gas coin's entire balance acts as the gas budget.

Gas is consumed in two parts:

| Part | Cost |
|------|------|
| Base transaction cost | 1 000 gas |
| VM instruction gas | varies by operation |

The base cost covers the overhead of deserializing and dispatching the transaction. VM instruction gas is charged per opcode and per native function call.

If the budget is exhausted before execution completes, the transaction fails. Gas is always charged regardless of outcome — a failed transaction still deducts from the gas coin.

The gas coin's balance is reduced by exactly `gas_used` and the updated coin appears in the transaction's `changed_objects`. For a full breakdown of per-instruction and per-native costs see [Adapter & Natives — Gas metering](adapter.md#gas-metering).

System and genesis transactions bypass gas entirely: they run under an unlimited meter and report `gas_used = 0`.

---

## Block rewards

After all user transactions in a block are executed, the miner collects their total gas cost as a reward:

```
reward = sum(gas_used) across all transactions in the block
```

If `reward > 0`, the miner builds a `meow_coin::mint(reward, reward_address)` transaction, signs it with the miner keypair, and executes it via the privileged system path (which can call the private `mint` function). The resulting `MeowCoin` object — owned by `reward_address` — is committed to the block alongside the user transactions.

The new coin object carries exactly the value that senders lost to gas in that block. Total supply is unchanged.

Blocks where all transactions cost zero gas carry no reward transaction.

### Reward verification

Every validating node re-executes the reward transaction and checks:

1. The signature is valid.
2. The call is `meow_coin::mint` with the correct amount (sum of re-executed user gas costs) and a well-formed recipient address.
3. The re-executed result matches the result included in the block exactly.
4. The state root computed after applying the reward matches the block header.

A mismatch at any step causes the entire block to be rejected.

---

## Signer and reward address

The miner has two distinct addresses:

| Address | Role |
|---------|------|
| **Miner address** | Derived from the signing keypair; signs reward transactions. |
| **Reward address** | Receives the minted coin; may be any address, e.g. a cold wallet. |

These are configured separately in `MinerConfig`. If `--miner-reward-address` is omitted when starting a node, the reward address defaults to the miner's own address.

Directing rewards to a cold wallet lets operators keep the hot signing key separate from accumulated funds.

---

## Supply dynamics

| Event | Supply change |
|-------|--------------|
| Genesis allocation | +N per row in `allocations.csv` |
| Block with fees | 0 — transaction sender loses `gas_used`, miner gains the same amount |
| Block with no fees | 0 |
| User calls `burn` | −burned coin's balance; permanently removed from circulation |

Total supply is fixed after genesis. The only operation that permanently removes coins from circulation is an explicit [`burn`](meow-coin.md#burn-a-coin).
