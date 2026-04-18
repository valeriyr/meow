# Contracts

> Writing, publishing, and calling smart contracts on MEOW.

## The Meow language

### Comments

Use `//` for single-line comments. Everything from `//` to the end of the line is ignored. `//` inside a string literal is not treated as a comment.

```meow
// This is a top-level comment.
fn add(a: u64, b: u64) -> u64 {
    // add two numbers
    return a + b; // inline comment
}
```

### Types

| Type | Semantics | Notes |
|------|-----------|-------|
| `bool` | Value | `true` or `false` |
| `u64` | Value | 64-bit unsigned integer |
| `address` | Value | 32-byte identifier, freely copyable |
| `string` | Value | UTF-8 string; freely copyable |
| `struct <Name>` | Value | Named record with primitive fields; freely copyable |
| `object <Name>` | Move | Owned on-chain resource; must have `id: address` as its first field |

Objects have **move semantics** — passing an object to a function consumes it. At the end of execution the adapter handles it in one of three ways (see [Object Model](object-model.md) for the full lifecycle):

- **Transferred** (`meow_vm_transfer`) — saved with the new owner.
- **Destroyed** (`meow_vm_destroy`) — removed from state.
- **Mutated in place** — if neither was called, the object is written back to its original owner with any field mutations applied. Ownership does not change.

Struct and object fields must be primitive (`bool`, `u64`, `address`, `string`). Objects cannot be nested inside other objects or structs.

### Statements

| Statement | Syntax |
|-----------|--------|
| Local variable | `let name = expr;` |
| Reassignment | `name = expr;` |
| Field assignment | `obj.field = expr;` |
| Return | `return expr;` or `return;` |
| Conditional | `if cond { ... }` or `if cond { ... } else { ... }` |
| Bare expression | `expr;` (value discarded) |

### Operators

Arithmetic: `+` `-` `*` `/` `%`  
Comparison: `==` `!=` `<` `<=` `>` `>=`  
Logical: `&&` `||`

### Native functions

These built-ins are always available and cannot be defined by user code.

| Function | Signature | Gas | Effect |
|----------|-----------|----:|--------|
| `meow_vm_fresh_id()` | `() → address` | 10 | Returns a new unique on-chain address |
| `meow_vm_transfer(obj, owner)` | `(object, address) → void` | 20 | Transfers ownership of `obj` to `owner` |
| `meow_vm_destroy(obj)` | `(object) → void` | 10 | Permanently destroys `obj` |
| `meow_vm_sender()` | `() → address` | 1 | Returns the transaction sender's address |
| `meow_vm_rand()` | `() → u64` | 10 | Returns the next pseudo-random `u64` from the block's RNG sequence |
| `meow_vm_timestamp()` | `() → u64` | 1 | Returns the block timestamp (Unix milliseconds) at the time the block was mined |
| `meow_vm_abort(cond, code, msg)` | `(bool, u64, string) → void` | — | Aborts execution if `cond` is `false` |

### Randomness

`meow_vm_rand()` advances a per-transaction sequence and returns a `u64`. Each call returns the next value in the same sequence — calling it twice in one function gives two different values.

The sequence is seeded from the block's mining hash and the transaction digest, so results are deterministic across re-executions but unpredictable at submission time. Successive calls within the same transaction are independent from other transactions in the block.

`meow_vm_rand()` is appropriate for low-stakes game mechanics or cosmetic variation. Avoid it for outcomes where miner manipulation would be economically rational. See [Consensus — Randomness](consensus.md#randomness) for the full security model.

### Time

`meow_vm_timestamp()` returns the **block timestamp** as Unix milliseconds — the value recorded in the block header by the miner when the block was produced.

```
let now = meow_vm_timestamp(); // e.g. 1712534400000
```

- **Block time, not submission time.** The value reflects when the block was mined, not when you sent the transaction.
- **Same for every transaction in the block.** Does not advance between transactions.
- **Millisecond precision.** Delays should be expressed in milliseconds (e.g. `86_400_000` for one day).

**Typical patterns:**

```meow
// Store a deadline at creation time
let unlock_time = meow_vm_timestamp() + delay_ms;

// Guard a function with a time check
meow_vm_abort(meow_vm_timestamp() >= capsule.unlock_time, 1, "still locked");

// Rate-limiting: reject actions that happen too soon
meow_vm_abort(meow_vm_timestamp() >= item.last_used + cooldown_ms, 2, "cooldown active");
```

See [Consensus — Timestamps](consensus.md#timestamps) for validation rules and miner behaviour.

## Access control

All functions, structs, objects, and struct fields are **private by default**. The `pub` keyword makes them accessible from other modules.

### Rules at a glance

| Declaration | Effect |
|-------------|--------|
| `fn foo(...)` | Private — callable only within this module |
| `pub fn foo(...)` | Public — callable from any module that imports this one |
| `struct Foo { ... }` | Private — not nameable from other modules |
| `pub struct Foo { ... }` | Public — other modules can use `Foo` as a type and receive values of this type |
| `object Foo { ... }` | Private — not nameable from other modules |
| `pub object Foo { ... }` | Public — other modules can use `Foo` as a type and receive values of this type |
| `field: u64` | Private — not readable or writable from other modules |
| `pub field: u64` | Public readable — readable from other modules; writes are still module-local |

### Construction is always module-local

Struct and object literals (`TypeName { field: value, ... }`) can only appear inside the module that declares the type, regardless of `pub`. Other modules must call a constructor function:

```meow
// shapes module
pub struct Point { pub x: u64, pub y: u64 }
pub fn make_point(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }

// user module
use shapes@0x...;
fn run() -> u64 {
    let p = shapes::make_point(3, 7); // ok — uses constructor
    // let p = shapes::Point { x: 3, y: 7 }; // rejected — cross-module construction
    return p.x;  // ok — x is pub
}
```

### The `id` field is immutable

The `id` field of every object is set at creation time and cannot be reassigned anywhere — even inside the declaring module:

```meow
object Coin { id: address, balance: u64 }
fn bad(c: Coin, new_id: address) {
    c.id = new_id; // compile error: 'id' is immutable
}
```

### Transaction entry points

Only `pub fn` functions can be called directly from a transaction. Sending a transaction that targets a private function is rejected by the VM before execution begins.

Native built-in functions (`meow_vm_transfer`, `meow_vm_fresh_id`, etc.) cannot be called directly from a transaction either — they are only available from within contract code.

```meow
mod vault;

fn internal_helper() -> u64 { return 1; } // cannot be called from a transaction

pub fn deposit(amount: u64) { ... }     // valid transaction target
pub fn withdraw(amount: u64) { ... }    // valid transaction target
```

### Summary of cross-module restrictions

| Operation | Cross-module allowed? |
|-----------|-----------------------|
| Call `pub fn` | Yes |
| Call private `fn` | No |
| Use `pub struct` / `pub object` as type | Yes |
| Use private struct / object as type | No |
| Construct any type with struct literal | No (always module-local) |
| Destroy object with `meow_vm_destroy` | No (always module-local) |
| Read `pub` field | Yes |
| Read private field | No |
| Write any field | No (always module-local) |
| Write `id` field | No (immutable everywhere) |

## Cross-module dependencies

Modules can import functions and types from other published modules using `use` declarations.

### Declaring a dependency

```meow
mod my_game;

use math@0x1a2b3c...;   // import the module named "math" at the given on-chain address
use utils@0x9f8e7d...;

fn level_up(hero: Hero) -> u64 {
    return math::scale(hero.xp, 2);
}
```

The `@<address>` suffix is the 32-byte on-chain address of the published module. The human-readable name before `@` is how you reference it in source (`math::fn_name`, `math::TypeName`).

### Using imported types and functions

- **Functions**: `module_name::function_name(args)` — only `pub fn` can be called cross-module.
- **Struct/object types**: receive values via `pub fn` return values or parameters; use `module_name::TypeName` as a type annotation. Direct construction (`module_name::TypeName { ... }`) is always rejected — use a constructor function exported by the dep module.

### Publishing a module with dependencies

Before publishing, declare dependencies via `use` in source. The CLI resolves and fetches all transitive deps from the node automatically:

```bash
meow transaction publish my_game.meow --sender <ADDRESS> --gas-coin <OBJECT_ADDRESS>
```

### Limits

| Limit | Default |
|-------|---------|
| `use` declarations per module (`max_imports`) | 64 |
| Total transitive dependency modules (`max_dep_modules`) | 64 |

The compiler enforces both limits at publish time. A module cannot be published if its transitive dependency graph exceeds `max_dep_modules`. Circular dependencies are also rejected.

### Running locally with dependencies

`meow contract run` fetches all transitive deps from the node automatically before executing:

```bash
meow contract run my_game.meow level_up 0x<hero_object>
```

## Bytecode verification

Every module is verified before it is stored on-chain. Verification runs automatically as part of `MeowModulePublish` execution — a module that fails verification is rejected and never reaches the object store.

The verifier operates on the raw `Module` bytecode struct, independent of how the bytecode was produced. It provides the same guarantees whether the module came from the compiler or was crafted by hand.

### What the verifier checks

**Structural (Phase 1)** — static shape checks, no stack simulation:

- All identifiers (module name, function names, struct/object names, field names) are valid.
- No duplicate function or struct/object names within a module.
- Every object definition has `id: address` as its first field.
- Object and struct fields are primitive types only — no nested objects.
- Local variable slot indices stay within the declared `local_count`.
- Jump offsets are forward-only and land on a valid instruction index.
- `NewStruct` field lists exactly match the struct definition.
- Cross-module `Call` targets are public functions.
- Cross-module `NewStruct` is forbidden (construction is always module-local).

**Abstract interpretation (Phase 2)** — symbolic execution over bytecode:

- Stack types are tracked through every instruction and across branch join points.
- Return type matches the function's declared return type at every `Return` instruction.
- Functions without a reachable `Return` are rejected.
- **Object linearity** — objects follow move semantics:
  - `Load` on an object slot consumes it; a second `Load` on the same slot is a use-after-move error.
  - `Pop` and `Dup` on an object are forbidden.
  - Overwriting a local slot that holds a live object is forbidden.
  - Any object that is still live at `Return` is an unconsumed-object error.
  - If a branch consumes an object and another does not, the join point is a liveness-conflict error.
- Native function calls are type-checked against the adapter-supplied signatures.
- Cross-module field reads and writes are checked for visibility.

### What this means for contract authors

If your contract compiles without errors, it will also pass verification — the compiler enforces all of these rules. Verification is a safety net for manually crafted or externally generated bytecode, not something a normal contract author needs to think about.

If you publish pre-built bytecode (e.g. from a custom tool), expect the same errors the compiler would have caught.

## Call argument format

| Value | Format | Example |
|-------|--------|---------|
| `bool` | `true` or `false` | `true` |
| `u64` | digits only | `100` |
| Raw address (not resolved) | `@0x<hex>` | `@0xabcd...` |
| On-chain object (resolved) | `0x<hex>` | `0xabcd...` |
| String | any other text | `hello` |

Use `@0x<hex>` when passing an `address`-typed argument (e.g. an owner or recipient). Use `0x<hex>` (without `@`) when passing an `object`-typed argument — the CLI fetches it from the node and passes it to the VM.

## System framework

| Module | What it covers |
|--------|----------------|
| [Meow Coin](meow-coin.md) | The built-in coin — transfer, split, merge, burn |

## Examples

| Example | What it covers |
|---------|----------------|
| [Hero game](example-hero-game.md) | Full lifecycle: write, test locally, publish, spawn, award XP, level up, transfer, retire |
| [Time capsule](example-time-capsule.md) | Using `meow_vm_timestamp()` to lock an object until a future block time |
