# Example: time capsule

> Time-locked messages on-chain using `meow_vm_timestamp()`.

A `Capsule` is an object that stores a message and an unlock time. It can only be opened once the block timestamp reaches that time — the contract enforces this with `meow_vm_abort`. This example shows the core pattern for time-gated logic: **record a timestamp at creation, compare against it later**.

See [Contracts](contracts.md) for the language reference and call argument format.

## The module

Create `time-capsule.meow`:

```meow
// time-capsule.meow
// A message sealed on-chain until a future block time.
//
// seal creates a Capsule that stores a message and an unlock_time computed as
// the current block timestamp plus a caller-supplied delay. open destroys the
// capsule once the block time reaches unlock_time — before that the transaction
// fails. reclaim lets the owner cancel early; transfer changes ownership.

mod time_capsule;

// A time-locked message stored on-chain.
//   id          — unique on-chain address, set at creation and immutable.
//   owner       — the address allowed to reclaim, transfer, or open this capsule.
//   unlock_time — Unix milliseconds after which open is permitted.
//   message     — the sealed message; readable in the execution result after open.
pub object Capsule {
    id: address,
    owner: address,
    unlock_time: u64,
    message: string
}

// Creates a Capsule containing message, locked until at least delay_ms milliseconds
// after the current block timestamp. Transfers the capsule to the transaction sender.
pub fn seal(message: string, delay_ms: u64) {
    let owner = meow_vm_sender();
    let capsule = Capsule {
        id: meow_vm_fresh_id(),
        owner: owner,
        unlock_time: meow_vm_timestamp() + delay_ms,
        message: message
    };
    meow_vm_transfer(capsule, owner);
}

// Destroys the capsule and makes its message visible in the execution result.
// Aborts with code 1 if the current block timestamp has not yet reached unlock_time.
pub fn open(capsule: Capsule) {
    meow_vm_abort(meow_vm_timestamp() >= capsule.unlock_time, 1, "Capsule is not ready to open yet");
    meow_vm_destroy(capsule);
}

// Destroys the capsule without waiting for the unlock time.
// Aborts with code 2 if the transaction sender is not the capsule owner.
pub fn reclaim(capsule: Capsule) {
    meow_vm_abort(capsule.owner == meow_vm_sender(), 2, "Only the owner can reclaim");
    meow_vm_destroy(capsule);
}

// Transfers the capsule to a new owner. Updates capsule.owner so the new owner
// can reclaim, transfer, or open it. Aborts with code 3 if the sender is not
// the current owner.
pub fn transfer(capsule: Capsule, to: address) {
    meow_vm_abort(capsule.owner == meow_vm_sender(), 3, "Only the owner can transfer");
    capsule.owner = to;
    meow_vm_transfer(capsule, to);
}
```

### How the timestamp works

`meow_vm_timestamp()` returns the **block timestamp in Unix milliseconds** — the time recorded by the miner when the block was sealed. It is the same value for every transaction in a block and is fixed before any transaction runs.

When `seal` executes, `meow_vm_timestamp()` is the block time of the block that includes the `seal` transaction, not the time you submitted it. The unlock time is therefore `block_time_of_seal + delay_ms`.

When `open` executes, `meow_vm_timestamp()` is the block time of the block that includes the `open` transaction. The check `meow_vm_timestamp() >= capsule.unlock_time` passes once a block is mined whose timestamp is at or after the recorded unlock time.

## Test locally

`meow contract run` compiles and runs a function in a local VM without submitting a transaction. It still connects to the node to resolve any `0x<hex>` object arguments, so:

- **Primitive arguments** (`bool`, `u64`, `address`, `string`) — work without a running node.
- **Object arguments** (`0x<hex>`) — require a running node and the object to already exist on-chain.

`seal` only takes primitives, so it can be run offline:

```bash
# Check that the module compiles cleanly
meow contract build time-capsule.meow

# Run seal locally — no node needed
# Seals a message with a 1-minute delay (60 000 ms)
meow contract run time-capsule.meow seal "Hello from the past" 60000
```

## Publish on-chain

Publishing creates a module object on-chain. You need a running node and a funded address.

```bash
# 1. Build and create the publish transaction
meow transaction publish time-capsule.meow --sender <YOUR_ADDRESS> --gas-coin <GAS_COIN_ADDRESS>

# 2. Sign the output transaction
meow transaction sign --transaction <BASE64_TRANSACTION>

# 3. Submit
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

Fetch the execution result to find the new module's on-chain address:

```bash
meow client get-transaction-result <TRANSACTION_DIGEST>
```

Look for the created object — that address is your `<MODULE_ADDRESS>`.

## Seal a capsule

Pass any string message and a delay in milliseconds. Common values:

| Delay | Milliseconds |
|-------|-------------|
| 1 minute | `60000` |
| 1 hour | `3600000` |
| 1 day | `86400000` |
| 1 week | `604800000` |

```bash
# Seal with a 1-day delay
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function seal \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  "Hello from the past" 86400000
```

Fetch the result to find the `Capsule` object address:

```bash
meow client get-transaction-result <TRANSACTION_DIGEST>
```

Inspect the capsule to see its `unlock_time`:

```bash
meow client get-object <CAPSULE_ADDRESS>
```

## Open a capsule

Submitting `open` before the unlock time results in a transaction failure with the message `Capsule is not ready to open yet`. Once the block timestamp reaches `unlock_time`, the transaction succeeds and the capsule is destroyed. The message field will be visible in the execution result before the object disappears from chain state.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function open \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <CAPSULE_ADDRESS>
```

## Reclaim a capsule

The original owner can destroy the capsule at any time, bypassing the unlock delay. Only the address stored in `capsule.owner` is allowed — anyone else gets `Only the owner can reclaim`.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function reclaim \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <CAPSULE_ADDRESS>
```

## Transfer before opening

Transferring updates `capsule.owner` so that only the new owner can reclaim or open it.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function transfer \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <CAPSULE_ADDRESS> @<RECIPIENT_ADDRESS>
```
