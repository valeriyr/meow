# Example: hero game

> An on-chain role-playing game in `.meow`.

A `Hero` is a named, uniquely-owned object that earns experience, levels up, and can duel other heroes. This example covers the full lifecycle: writing the module, testing locally, publishing on-chain, and calling each function.

See [Contracts](contracts.md) for the language reference and call argument format.

## The module

Create `hero.meow`:

```meow
// hero.meow
// An on-chain hero that earns experience, levels up, and can duel others.

object Hero {
    id: address,
    name: string,
    level: u64,
    experience: u64,
    wins: u64
}

// Spawn a new level-1 hero and transfer it to the transaction sender.
fn spawn(name: string) {
    let owner = meow_vm_sender();
    let hero = Hero {
        id: meow_vm_fresh_id(),
        name: name,
        level: 1,
        experience: 0,
        wins: 0
    };
    meow_vm_transfer(hero, owner);
}

// Rename the hero.
fn rename(hero: Hero, new_name: string) {
    hero.name = new_name;
}

// Duel two heroes. Both must be owned by the transaction sender.
// Each hero draws a random number — higher roll wins. The winner gains
// XP equal to the loser's level × 25 and levels up automatically when
// their XP reaches level × 100. The outcome is non-deterministic:
// it is seeded from the block's mining hash (see Contracts → Randomness).
fn duel(attacker: Hero, defender: Hero) {
    meow_vm_abort(attacker.id != defender.id, 1, "A hero cannot duel itself");

    let attacker_roll = meow_vm_rand();
    let defender_roll = meow_vm_rand();

    if attacker_roll >= defender_roll {
        attacker.wins = attacker.wins + 1;
        attacker.experience = attacker.experience + defender.level * 25;
        if attacker.experience >= attacker.level * 100 {
            attacker.experience = attacker.experience - attacker.level * 100;
            attacker.level = attacker.level + 1;
        }
    } else {
        defender.wins = defender.wins + 1;
        defender.experience = defender.experience + attacker.level * 25;
        if defender.experience >= defender.level * 100 {
            defender.experience = defender.experience - defender.level * 100;
            defender.level = defender.level + 1;
        }
    }
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

`spawn` only takes a `string`, so it can be run offline:

```bash
# Check that the module compiles cleanly
meow smart-contract build hero.meow

# Run spawn locally — no node needed
meow smart-contract run hero.meow spawn Thorin
```

To test `duel` or `retire` locally, the `Hero` objects must already be on-chain. Use the `0x<hex>` form (no `@`) so the CLI fetches them from the node:

```bash
meow smart-contract run hero.meow duel <ATTACKER_HERO_ADDRESS> <DEFENDER_HERO_ADDRESS>
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

Each hero draws a random number from `meow_vm_rand()`. Higher roll wins — the outcome is never guaranteed regardless of level. The winner gains `loser.level × 25` XP and **levels up automatically** when their XP reaches `level × 100` (level 1 → 2 at 100 XP, level 2 → 3 at 200 XP, and so on). The randomness is seeded from the block's mining hash (which commits to the transaction set and nonce), so it is deterministic across all validators but cannot be predicted by a transaction sender before the block is mined. See [Contracts → Randomness](contracts.md#randomness) for the full security model.

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

After the duel both heroes survive in-place (neither is transferred nor destroyed), so the executor writes them back to the sender automatically with updated `experience` and `wins`.

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
