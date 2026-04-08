# Contracts

> Writing, publishing, and calling `.meow` smart contracts on MEOW.

## The `.meow` language

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
- **Survived** — if neither was called, the object is written back to its original owner with any field mutations applied. Ownership does not change.

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

Arithmetic: `+` `-` `*` `/`  
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
| `meow_vm_abort(cond, code, msg)` | `(bool, u64, string) → void` | — | Aborts execution if `cond` is `false` |

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
| [MeowCoin](meow-coin.md) | The built-in coin — transfer, split, merge, burn |

## Examples

| Example | What it covers |
|---------|----------------|
| [Hero game](example-hero-game.md) | Full lifecycle: write, test locally, publish, spawn, award XP, level up, transfer, retire |
