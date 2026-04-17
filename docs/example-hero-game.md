# Example: hero game

> An on-chain role-playing game in `.meow`.

A `Hero` is a named, uniquely-owned object that earns experience, levels up, and can duel other heroes. This example covers the full lifecycle: writing the module, testing locally, publishing on-chain, and calling each function.

See [Contracts](contracts.md) for the language reference and call argument format.

## The module

Create `hero.meow`:

```meow
// hero.meow
// An on-chain hero that earns experience, levels up, and can duel others.
//
// Each Hero is a uniquely-owned object. Spawn creates one and sends it to the
// transaction sender. Duel resolves combat using on-chain randomness seeded
// from the block's mining hash — results are deterministic across validators
// but unpredictable at submission time.

module hero;

// An on-chain hero owned by a player.
//   id         — unique on-chain address, set at creation and immutable.
//   name       — display name chosen at spawn; can be changed via rename.
//   level      — starts at 1; increases as experience accumulates.
//   experience — XP earned by winning duels; resets on each level-up.
//   wins       — total number of duels won; never decreases.
pub object Hero {
    id: address,
    name: string,
    level: u64,
    experience: u64,
    wins: u64
}

// Creates a new level-1 Hero with the given name and transfers it to the
// transaction sender.
pub fn spawn(name: string) {
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

// Changes the hero's display name. The hero is written back to its owner
// with the updated name (no transfer or destroy occurs).
pub fn rename(hero: Hero, new_name: string) {
    hero.name = new_name;
}

// Resolves a duel between two heroes. Both must be owned by the transaction sender.
//
// Each hero draws a random u64 from the block's RNG sequence. Higher roll wins.
// The winner gains loser.level × 25 XP and levels up when XP reaches level × 100
// (resetting the XP to the remainder). Ties go to the attacker.
//
// Aborts with code 1 if attacker and defender are the same hero.
pub fn duel(attacker: Hero, defender: Hero) {
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

// Transfers the hero to another player.
pub fn transfer(hero: Hero, to: address) {
    meow_vm_transfer(hero, to);
}

// Permanently destroys the hero and removes it from chain state.
pub fn retire(hero: Hero) {
    meow_vm_destroy(hero);
}
```

## Test locally

`meow contract run` compiles and runs a function in a local VM without submitting a transaction. It still connects to the node to resolve any `0x<hex>` object arguments, so:

- **Primitive arguments** (`bool`, `u64`, `address`, `string`) — work without a running node.
- **Object arguments** (`0x<hex>`) — require a running node and the object to already exist on-chain.

`spawn` only takes a `string`, so it can be run offline:

```bash
# Check that the module compiles cleanly
meow contract build hero.meow

# Run spawn locally — no node needed
meow contract run hero.meow spawn Thorin
```

To test `duel` or `retire` locally, the `Hero` objects must already be on-chain. Use the `0x<hex>` form (no `@`) so the CLI fetches them from the node:

```bash
meow contract run hero.meow duel <ATTACKER_HERO_ADDRESS> <DEFENDER_HERO_ADDRESS>
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
