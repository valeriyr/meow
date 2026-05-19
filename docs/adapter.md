# Adapter & Native Functions

> How the Meow VM connects to the chain, and what native functions the adapter provides.

The **adapter** (`meow-vm-adapter`) is the glue layer between the stack-based Meow VM and the MEOW chain's object store. It:

- Deserialises input objects from BCS into VM values.
- Supplies native function implementations (`meow_vm_fresh_id`, `meow_vm_transfer`, etc.).
- Runs the bytecode verifier before storing any new module.
- Translates VM execution results back into chain effects (created / changed / destroyed objects).

## Native functions

Native functions are built into the runtime and cannot be defined by user code. They are split into two groups: **language built-ins** (always present) and **adapter-supplied** (provided by the chain layer).

### Language built-in

| Function | Signature | Gas | Description |
|----------|-----------|----:|-------------|
| `meow_vm_abort(cond, code, msg)` | `(bool, u64, string) → void` | — | Aborts execution if `cond` is `false`. The transaction fails with `code` and `msg` in the result. |

### Adapter-supplied

| Function | Signature | Gas | Description |
|----------|-----------|----:|-------------|
| `meow_vm_fresh_id()` | `() → meow_object::Id` | 10 | Allocates a new unique object identity derived from the transaction digest and a per-transaction counter. The returned `Id` must eventually be consumed by `meow_vm_transfer` (via the object that holds it) or `meow_vm_destroy`. |
| `meow_vm_transfer(obj, owner)` | `(struct, address) → void` | 20 | Transfers ownership of `obj` to `owner`. Accepts any struct. If `obj` contains an `id: meow_object::Id` first field it is saved to the object store; otherwise execution aborts. |
| `meow_vm_destroy(id)` | `(meow_object::Id) → void` | 10 | Destroys the object identified by `id`. The object is removed from the store at the end of the transaction. |
| `meow_vm_sender()` | `() → address` | 1 | Returns the 32-byte address of the transaction sender. |
| `meow_vm_rand()` | `() → u64` | 10 | Returns the next value from the block's pseudo-random sequence. Deterministic across re-executions; seeded from the block's mining hash and the transaction digest. |
| `meow_vm_timestamp()` | `() → u64` | 1 | Returns the block timestamp in Unix milliseconds. Same for all transactions in a block. |

## The `meow_object` system module

`meow_object` is published at the fixed address `0x01`. It exports a single type:

```meow
pub struct Id {
    inner: address
}
```

`Id` is an opaque wrapper around a 32-byte address. The inner field is private — user code cannot read or manipulate it directly. The only way to obtain an `Id` is via `meow_vm_fresh_id()`.

Any struct whose first field is `id: meow_object::Id` is recognised by the adapter as an **on-chain object**. This is a layout convention enforced at publish time by the bytecode verifier and at execution time by the adapter.

```meow
use meow_object@0x01;

pub struct Hero {
    id: meow_object::Id,   // first field — marks this as an on-chain object
    name: string,
    level: u64
}
```

## On-chain object lifecycle

Every object ID allocated by `meow_vm_fresh_id()` must be either transferred (as part of the struct that holds it) or destroyed before the transaction ends. An ID that is neither transferred nor destroyed causes the transaction to fail.

| State | How reached | Store effect |
|-------|-------------|--------------|
| **Created** | `meow_vm_fresh_id()` then `meow_vm_transfer(obj, owner)` | Inserted |
| **Mutated in place** | Fields modified; no transfer or destroy | Overwritten with new content |
| **Transferred** | `meow_vm_transfer(obj, new_owner)` | Overwritten with new owner |
| **Destroyed** | `meow_vm_destroy(obj.id)` | Removed |

Objects created and destroyed within the same transaction leave no trace in the store.

## The `meow_coin` system module

`meow_coin` is published at the fixed address `0x10`. It provides `MeowCoin` — the native coin of the MEOW chain.

```meow
pub struct MeowCoin {
    id: meow_object::Id,
    balance: u64
}

pub struct MeowCoinBalance {
    amount: u64
}
```

`MeowCoinBalance` is a plain (non-object) struct that wraps a coin amount. It can be embedded as a field inside other on-chain objects. Use `to_balance` / `from_balance` to convert between the two types.

Public functions:

| Function | Signature | Description |
|----------|-----------|-------------|
| `balance(coin)` | `(MeowCoin) → (MeowCoin, u64)` | Returns the coin and its balance without consuming it. |
| `burn(coin)` | `(MeowCoin) → void` | Destroys the coin; balance is lost. |
| `transfer(coin, to)` | `(MeowCoin, address) → void` | Transfers the coin to a new owner. |
| `merge(from, to)` | `(MeowCoin, MeowCoin) → void` | Adds `from.balance` to `to.balance`, destroys `from`, then transfers `to` to the transaction sender. |
| `merge_and_transfer(from, to, recipient)` | `(MeowCoin, MeowCoin, address) → void` | Like `merge` but sends the result to `recipient` instead of the sender. |
| `split(from, amount)` | `(MeowCoin, u64) → void` | Splits `amount` out of `from` into a new coin sent to the sender. Aborts if balance < amount. |
| `split_and_transfer(from, amount, to)` | `(MeowCoin, u64, address) → void` | Like `split` but sends the new coin to `to`. |
| `to_balance(coin)` | `(MeowCoin) → MeowCoinBalance` | Converts the coin to a `MeowCoinBalance` (destroys the coin). |
| `from_balance(balance)` | `(MeowCoinBalance) → MeowCoin` | Creates a new coin from a balance struct. |

The gas coin used for transaction fees is a `MeowCoin` object. It is deducted after execution (success or failure) and is always returned in `changed_objects`.

## Bytecode verification

Every module is verified by `meow-vm-bytecode-verifier` before it is stored on-chain. Verification runs as part of `MeowModulePublish` execution — a module that fails is rejected.

The verifier operates on raw `Module` bytecode, independent of whether the bytecode was produced by the compiler or crafted manually.

### Phase 1 — structural checks

- All identifiers (module name, function names, struct names, field names) are valid.
- No duplicate function or struct names.
- Local variable slot indices stay within `local_count`.
- Jump offsets are forward-only and land on a valid instruction index.
- `NewStruct` field lists exactly match the struct definition.
- Cross-module `Call` targets are public functions.
- Cross-module `NewStruct` is forbidden.

### Phase 2 — abstract interpretation

- Stack types tracked through every instruction and across branch join points.
- Return type matches the declared return type.
- Functions without a reachable `Return` are rejected.
- **Struct linearity**: every struct follows move semantics — use-after-move, pop/dup/overwrite on struct slots, and unconsumed structs at `Return` are all errors.
- Native call sites are type-checked against the adapter-supplied signatures.
- Cross-module field reads and writes are rejected.

### Adapter native signatures

The verifier is a free function. Pass the adapter's native signatures so it can type-check native call sites:

```rust
use meow_vm_bytecode_verifier;
use meow_vm_adapter::natives;

meow_vm_bytecode_verifier::verify(&module, &deps, &natives::adapter_native_sigs(), &compiler_config)?;
```

`adapter_native_sigs()` is defined in `meow-vm-adapter/src/natives.rs` and returns signatures for all six adapter-supplied natives. The verifier also includes the language built-in (`meow_vm_abort`) automatically.

## Gas metering

Gas is charged per instruction. The gas coin's `balance` field is deducted after execution, floored at zero. A base cost of **1 000 gas** is always charged, even on failure.

Key costs:

| Operation | Gas |
|-----------|----:|
| Base transaction cost | 1 000 |
| `meow_vm_fresh_id` | 10 |
| `meow_vm_transfer` | 20 |
| `meow_vm_destroy` | 10 |
| `meow_vm_rand` | 10 |
| `meow_vm_sender` | 1 |
| `meow_vm_timestamp` | 1 |
| Module publish (per compiled byte) | 10 |

VM instruction costs:

| Instruction(s) | Gas |
|---|---:|
| `PushBool`, `PushU64`, `PushAddress` | 1 |
| `PushStr` | 2 |
| `Load`, `Store` | 1 |
| `LoadField` | 2 |
| `StoreField` | 5 |
| `Add`, `Sub`, `Mul` | 2 |
| `Div`, `Mod` | 5 |
| `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge` | 2 |
| `Not`, `And`, `Or` | 1 |
| `NewStruct` | 10 + 2 × field count |
| `GetField` | 3 |
| `Pop`, `Dup` | 1 |
| `Jump`, `JumpIf`, `JumpIfNot` | 2 |
| `Call` | 20 |
| `Return` | 2 |
| `MakeTuple(n)`, `UnpackTuple(n)` | n |
| `UnpackStruct` | field count |
