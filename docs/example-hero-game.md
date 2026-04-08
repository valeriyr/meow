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
    wins: u64,
    sword_attack: u64,
    shield_defense: u64
}

// Spawn a new level-1 hero with no equipment and transfer it to the transaction sender.
fn spawn(name: string) {
    let owner = meow_vm_sender();
    let hero = Hero {
        id: meow_vm_fresh_id(),
        name: name,
        level: 1,
        experience: 0,
        wins: 0,
        sword_attack: 0,
        shield_defense: 0
    };
    meow_vm_transfer(hero, owner);
}

// Rename the hero.
fn rename(hero: Hero, new_name: string) {
    hero.name = new_name;
}

// Forge a sword. Higher attack increases power in duels.
// Cannot downgrade: the new value must be greater than the current one.
fn forge_sword(hero: Hero, attack: u64) {
    meow_vm_abort(attack > hero.sword_attack, 3, "New sword must be stronger than the current one");
    hero.sword_attack = attack;
}

// Forge a shield. Higher defense reduces XP loss when losing a duel.
// Cannot downgrade: the new value must be greater than the current one.
fn forge_shield(hero: Hero, defense: u64) {
    meow_vm_abort(defense > hero.shield_defense, 4, "New shield must be stronger than the current one");
    hero.shield_defense = defense;
}

// Duel two heroes. Both must be owned by the transaction sender.
// Power = level * 100 + experience + sword_attack. Attacker wins on a tie.
// Winner gains XP scaled to the loser's level and levels up automatically if threshold is reached.
// XP penalty on the loser is reduced by their shield_defense.
fn duel(attacker: Hero, defender: Hero) {
    meow_vm_abort(attacker.id != defender.id, 2, "A hero cannot duel itself");

    let attacker_power = attacker.level * 100 + attacker.experience + attacker.sword_attack;
    let defender_power = defender.level * 100 + defender.experience + defender.sword_attack;

    if attacker_power >= defender_power {
        attacker.experience = attacker.experience + defender.level * 25;
        attacker.wins = attacker.wins + 1;
        let required = attacker.level * 100;
        if attacker.experience >= required {
            attacker.experience = attacker.experience - required;
            attacker.level = attacker.level + 1;
        }
        let penalty = attacker.level * 10;
        if penalty > defender.shield_defense {
            let net_penalty = penalty - defender.shield_defense;
            if defender.experience >= net_penalty {
                defender.experience = defender.experience - net_penalty;
            } else {
                defender.experience = 0;
            }
        }
    } else {
        defender.experience = defender.experience + attacker.level * 25;
        defender.wins = defender.wins + 1;
        let required = defender.level * 100;
        if defender.experience >= required {
            defender.experience = defender.experience - required;
            defender.level = defender.level + 1;
        }
        let penalty = defender.level * 10;
        if penalty > attacker.shield_defense {
            let net_penalty = penalty - attacker.shield_defense;
            if attacker.experience >= net_penalty {
                attacker.experience = attacker.experience - net_penalty;
            } else {
                attacker.experience = 0;
            }
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

`spawn` takes a name string and automatically sends the hero to the transaction sender via `meow_vm_sender()`.

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

## Forge equipment

Equip a sword to boost attack power in duels. Equip a shield to reduce XP loss when losing. Neither can be downgraded.

```bash
# Forge a sword with attack power 30
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function forge_sword \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <HERO_ADDRESS> 30

# Forge a shield with defense power 15
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function forge_shield \
  --sender <YOUR_ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  <HERO_ADDRESS> 15
```

## Duel

Both heroes must be owned by the transaction sender. Spawn a second hero first, then pass both addresses as object arguments.

Power is computed as `level × 100 + experience + sword_attack` — equipment directly influences who wins. The attacker wins on a tie. The winner gains `loser.level × 25` XP and **levels up automatically** if their XP reaches `level × 100` (so level 1 → 2 at 100 XP, level 2 → 3 at 200 XP, and so on). The XP penalty on the loser is `winner.level × 10` reduced by their `shield_defense`.

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
