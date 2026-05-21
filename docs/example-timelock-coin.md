# Example: timelock coin

> Lock a MeowCoin balance on-chain until a future block timestamp using `meow_vm_timestamp()`.

A `TimelockCoin` holds a MeowCoin balance that can only be claimed once the block timestamp reaches a chosen unlock time. This example shows the core pattern for time-gated value: **record an unlock timestamp at creation, enforce it on release**.

See [Contracts](contracts.md) for the CLI commands and call argument format, and [Meow Coin](meow-coin.md) for the coin module reference.

## The module

Source: [`crates/meow-vm-examples/modules/timelock_coin.meow`](../crates/meow-vm-examples/modules/timelock_coin.meow)

### How the timestamp works

`meow_vm_timestamp()` returns the **block timestamp in Unix milliseconds** — the time recorded by the miner when the block was sealed. It is the same value for every transaction in a block and is fixed before any transaction runs.

When `lock` executes, `meow_vm_timestamp()` is the block time of the block that includes the `lock` transaction. The unlock time is therefore `block_time_of_lock + delay_ms`.

When `claim` executes, `meow_vm_timestamp()` is the block time of the block that includes the `claim` transaction. The check `meow_vm_timestamp() >= lock.unlock_time` passes once a block is mined whose timestamp is at or after the recorded unlock time.

### How lock and claim handle the balance

`lock` calls `meow_coin::to_balance` to convert the input coin into a `MeowCoinBalance` value, which is stored directly as the `balance` field of `TimelockCoin`. The original coin is destroyed in the process.

`claim` reverses this: it unpacks `TimelockCoin` to recover the `MeowCoinBalance`, then calls `meow_coin::from_balance` to mint a fresh `MeowCoin` and transfer it to the sender.

## Build the module

This module depends on `meow_coin@0x20`, so building requires a running node:

```bash
meow contract build timelock_coin.meow
```

All functions take object arguments (`MeowCoin` or `TimelockCoin`), so `meow contract run` also requires a running node with the objects already on-chain.

## Publish on-chain

Publishing creates a module object on-chain. You need a running node and a funded address.

```bash
# 1. Build and create the publish transaction
meow transaction publish timelock_coin.meow --sender <YOUR_ADDRESS> --gas-coin <GAS_COIN_ADDRESS>

# 2. Sign the output transaction
meow transaction sign <BASE64_TRANSACTION>

# 3. Submit
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

Fetch the execution result to find the new module's on-chain address:

```bash
meow client get-transaction-result <TRANSACTION_DIGEST>
```

Look for the created object — that address is your `<MODULE_ADDRESS>`.

## Lock a coin

Pass a `MeowCoin` object and a delay in milliseconds. Common values:

| Delay    | Milliseconds |
|----------|-------------|
| 1 minute | `60000`      |
| 1 hour   | `3600000`    |
| 1 day    | `86400000`   |
| 1 week   | `604800000`  |

```bash
# Lock a coin for 1 day
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function lock \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <COIN_ADDRESS> 86400000
```

Fetch the result to find the `TimelockCoin` object address:

```bash
meow client get-transaction-result <TRANSACTION_DIGEST>
```

Inspect the lock to see its `unlock_time` and current `balance`:

```bash
meow client get-object <TIMELOCK_COIN_ADDRESS>
```

## Claim after unlock

Submitting `claim` before `unlock_time` results in a transaction failure with the message `Coin is still locked`. Once the block timestamp reaches `unlock_time`, the transaction succeeds: the `TimelockCoin` is destroyed and a fresh `MeowCoin` with the stored balance is transferred to the sender.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function claim \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <TIMELOCK_COIN_ADDRESS>
```

## Transfer the lock

Transfers the lock to a new address. The new owner can then claim or transfer it further.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function transfer \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <TIMELOCK_COIN_ADDRESS> @<RECIPIENT_ADDRESS>
```
