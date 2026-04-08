# Example: hero game

> A minimal on-chain role-playing game in `.meow`.

A `Hero` is a uniquely-owned object that accumulates experience and can level up. This example covers the full lifecycle: writing the module, testing locally, publishing on-chain, and calling each function.

See [Contracts](contracts.md) for the language reference and call argument format.

## The module

Create `hero.meow`:

```meow
// hero.meow
// A simple on-chain hero that accumulates experience and levels up.

object Hero { id: address, level: u64, experience: u64 }

// Spawn a new level-1 hero and transfer it to `owner`.
fn spawn(owner: address) {
    let hero = Hero { id: meow_vm_fresh_id(), level: 1, experience: 0 };
    meow_vm_transfer(hero, owner);
}

// Award experience points to the hero.
// The hero survives in-place — the executor writes it back to the original owner automatically.
fn award_xp(hero: Hero, xp: u64) {
    hero.experience = hero.experience + xp;
}

// Level up the hero. Requires at least 100 XP; deducts 100 and increments level.
// No explicit transfer needed — mutated input objects are written back to their
// original owner automatically when the function returns.
fn level_up(hero: Hero) {
    meow_vm_abort(hero.experience >= 100, 1, "Not enough experience to level up");
    hero.experience = hero.experience - 100;
    hero.level = hero.level + 1;
}

// Transfer the hero to another player.
fn transfer(hero: Hero, to: address) {
    meow_vm_transfer(hero, to);
}

// Permanently remove the hero from the chain.
fn retire(hero: Hero) {
    meow_vm_destroy(hero);
}
```

## Test locally

`meow smart-contract run` compiles and runs a function in a local VM without submitting a transaction. It still connects to the node to resolve any `0x<hex>` object arguments, so:

- **Primitive arguments** (`bool`, `u64`, `address`, `string`) — work without a running node.
- **Object arguments** (`0x<hex>`) — require a running node and the object to already exist on-chain.

`spawn` only takes an `address`, so it can be run offline:

```bash
# Check that the module compiles cleanly
meow smart-contract build hero.meow

# Run spawn locally — owner is a raw address (@0x prefix), no node needed
meow smart-contract run hero.meow spawn @0xaa
```

To test `award_xp`, `level_up`, `transfer`, or `retire` locally, the `Hero` object must already be on-chain. Use the `0x<hex>` form (no `@`) so the CLI fetches it from the node:

```bash
meow smart-contract run hero.meow award_xp <HERO_ADDRESS> 50
```

## Publish on-chain

Publishing creates a module object on-chain. You need a running node and a funded address.

```bash
# 1. Build and create the publish transaction
meow transaction publish hero.meow --sender <YOUR_ADDRESS> --gas-coin <GAS_COIN_ADDRESS>

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

## Spawn a hero

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function spawn \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  @<YOUR_ADDRESS>
```

Fetch the result to find the `Hero` object address:

```bash
meow client get-transaction-result <TRANSACTION_DIGEST>
```

## Award experience

The hero object address (without `@`) is resolved from the node and passed as a move argument. Plain integers are recognized as `u64` automatically.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function award_xp \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <HERO_ADDRESS> 75
```

## Level up

Fails with abort code `1` if the hero has fewer than 100 XP.

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function level_up \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <HERO_ADDRESS>
```

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
