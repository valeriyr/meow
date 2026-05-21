# Example: hero game

> An on-chain role-playing game in `.meow`.

A `Hero` is a named, uniquely-owned object that earns experience, levels up, and can duel other heroes. This example covers the full lifecycle: writing the module, testing locally, publishing on-chain, and calling each function.

See [Contracts](contracts.md) for the CLI commands and call argument format.

## The module

Source: [`crates/meow-vm-examples/modules/hero_game.meow`](../crates/meow-vm-examples/modules/hero_game.meow)

## Test locally

`meow contract run` compiles and runs a function in a local VM without submitting a transaction. A running node is always required — dependency modules (`meow_object@0x10`) are fetched from it. The distinction is in the function arguments:

- **Primitive arguments** (`bool`, `u64`, `address`, `string`) — no user-created objects needed beyond the module dependencies.
- **Object arguments** (`0x<hex>`) — the referenced objects must already exist on-chain.

`spawn` only takes a `string`, so no on-chain objects need to exist before running it:

```bash
# Check that the module compiles cleanly
meow contract build hero_game.meow

# Run spawn — requires a node, but no pre-existing object arguments
meow contract run hero_game.meow spawn Thorin
```

To test `duel` or `retire` locally, the `Hero` objects must already be on-chain. Use the `0x<hex>` form (no `@`) so the CLI fetches them from the node:

```bash
meow contract run hero_game.meow duel <ATTACKER_HERO_ADDRESS> <DEFENDER_HERO_ADDRESS>
```

## Publish on-chain

Publishing creates a module object on-chain. You need a running node and a funded address.

```bash
# 1. Build and create the publish transaction
meow transaction publish hero_game.meow --sender <YOUR_ADDRESS> --gas-coin <GAS_COIN_ADDRESS>

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

## Spawn a hero

`spawn` takes a name string and sends the new hero to the transaction sender via `meow_vm_sender()`.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function spawn \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  Thorin
```

Fetch the result to find the `Hero` object address:

```bash
meow client get-transaction-result <TRANSACTION_DIGEST>
```

## Rename the hero

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function rename \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <HERO_ADDRESS> "Thorin Oakenshield"
```

## Duel

Both heroes must be owned by the transaction sender. Spawn a second hero first, then pass both addresses as object arguments.

Each hero draws a random number from `meow_vm_rand()`. Higher roll wins; ties go to the attacker — the outcome is never guaranteed regardless of level. The winner gains `loser.level × 25` XP and **levels up automatically** when their XP reaches `level × 100` (level 1 → 2 at 100 XP, level 2 → 3 at 200 XP, and so on). The randomness is seeded from the block's mining hash (which commits to the transaction set and nonce), so it is deterministic across all nodes but cannot be predicted by a transaction sender before the block is mined. See [Contracts → Randomness](contracts.md#randomness) for the full security model.

```bash
# Spawn a second hero to be the defender
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function spawn \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  Goblin

# Duel — attacker is the first argument, defender is the second
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function duel \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <ATTACKER_HERO_ADDRESS> <DEFENDER_HERO_ADDRESS>
```

After the duel both heroes are explicitly transferred back to the transaction sender with their updated stats.

## Inspect the hero

```bash
meow client get-object <HERO_ADDRESS>
```

## Transfer to another player

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function transfer \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <HERO_ADDRESS> @<RECIPIENT_ADDRESS>
```

## Retire the hero

Destroys the `Hero` object permanently.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function retire \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <HERO_ADDRESS>
```
