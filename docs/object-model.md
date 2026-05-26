# Object Model

> How state is represented, owned, versioned, and mutated on MEOW.

All persistent state on MEOW lives in **objects** — typed, owned, versioned records stored in a flat address-keyed map. Transactions consume input objects and produce output objects. There is no other mutable state.

## What is an object?

Every object has six fields:

| Field | Type | Description |
|-------|------|-------------|
| `address` | `Address` | Permanent 32-byte identity. Never changes across mutations. |
| `owner` | `ObjectOwner` | Who controls this object. |
| `transaction` | `Digest` | Digest of the transaction that created or last mutated this object. |
| `version` | `u64` | Monotonically increasing mutation counter. Starts at 1. Bumped by 1 on every mutation. |
| `type_` | `ObjectType` | Whether this is a compiled module or a typed instance. |
| `content` | `Vec<u8>` | BCS-encoded list of `(field_name, value)` pairs. The `id` field is **not** stored here — it lives in `address`. |

The **object digest** is `Blake2b-256(bcs(object))` computed over all six fields. It is not stored directly; it is recomputed on demand and used to build `ObjectRef`.

## Ownership

`ObjectOwner` has two variants:

| Variant | Meaning |
|---------|---------|
| `Address(addr)` | Owned by a specific account. Can be passed as an argument to a call. |
| `Immutable` | No owner. Cannot be mutated or transferred. All published modules have this owner. |

Only `Address`-owned non-module objects are accepted as call arguments.

## Types

`ObjectType` discriminates two kinds of objects:

| Variant | Meaning |
|---------|---------|
| `Module` | On-chain compiled module bytecode. `content` holds raw BCS-encoded `Module` bytes. |
| `Object(module, name)` | An instance of a user-defined type. `module` is the address of the declaring module; `name` is the declared struct name (e.g. `Hero`). |

The fully-qualified type of an instance is displayed as `<module_address>::<name>`.

## ObjectRef — version-pinned pointers

Transactions do not reference objects by address alone. Every object argument (including the gas coin) is specified as an `ObjectRef`:

```
ObjectRef { address, version, digest }
```

Before execution the node checks:
- the live object's `version` matches the `ObjectRef`'s version
- the live object's computed digest matches the `ObjectRef`'s digest

**This protects against two things:**

1. **Optimistic concurrency** — if another transaction mutated the object between when you read it and when your transaction executes, the version no longer matches and your transaction is rejected rather than operating on stale state.
2. **Replay attacks** — after a transaction mutates the gas coin its version is bumped. Any replayed transaction carries the old `ObjectRef` and is rejected. The `gas_coin: ObjectRef` therefore acts as both fee payment and nonce — no separate sequence number is needed.

## Object lifecycle

During execution objects follow exactly one of three paths:

| Lifecycle | How it happens | Store effect |
|-----------|----------------|--------------|
| **Created** | `meow_vm_fresh_id()` allocates an ID; `meow_vm_transfer(obj, owner)` hands it to an owner | Added to store |
| **Transferred** | `meow_vm_transfer(obj, new_owner)` | Updated in store with new owner |
| **Destroyed** | destructure to extract `id`, then `meow_vm_destroy(id)` | Removed from store |

An object created and destroyed within the same transaction has no net effect on the store.

Every ID allocated by `meow_vm_fresh_id()` must be either transferred or destroyed before the transaction ends. If it is not, execution fails with an error.

## Object IDs

In the Meow Language, every on-chain object is a `struct` whose first field is `id: meow_object::Id` (from the built-in [`meow_object` system module](adapter.md#the-meow_object-system-module) at address `0x10`):

```meow
use meow_object@0x10;

struct Hero { id: meow_object::Id, level: u64, experience: u64 }
```

At creation time the ID is obtained by calling `meow_vm_fresh_id()`, which returns a `meow_object::Id`:

```meow
let hero = Hero { id: meow_vm_fresh_id(), level: 1, experience: 0 };
```

Internally this computes `Blake2b-256([tag=0] ++ tx_digest ++ counter)`, where `counter` increments for each call within the same transaction. Because the transaction digest commits to the sender, gas coin reference, and call arguments — and the gas coin's `ObjectRef` is never reusable — the `(tx_digest, counter)` pair is globally unique.

The `id` field is not stored in the object's `content` bytes. The encoding layer strips it before serialization and re-injects it from `address` when the VM reads the object, to avoid duplicating state.

## The gas coin

The gas coin is an ordinary [`MeowCoin`](meow-coin.md) object, not a special-cased field. It is owned by the sender and referenced like any other object argument:

```json
"gas_coin": { "address": "<Address>", "version": 1, "digest": "<Digest>" }
```

Fee deduction runs unconditionally after execution, on success or failure:

1. A base cost of **1000 gas** is charged at the start.
2. Individual operations charge additional gas — see [Gas metering](adapter.md#gas-metering) for the full table.
3. The gas coin's `balance` is reduced by the total spent, floored at 0.
4. The updated coin always appears in `changed_objects` in the execution result.

Because the gas coin is an object, every transaction — even a failed one — bumps the coin's version, permanently invalidating its `ObjectRef` and preventing replay.

## The object store

The store is a flat `BTreeMap<Address, Object>`. Each committed block applies its execution results atomically in a single pass: created objects are inserted, changed objects overwrite their previous entry, destroyed objects are removed.

`ChainState` keeps a full store snapshot per block up to `snapshot_depth` blocks behind the head. On a reorg, the node simply restores the snapshot at the new chain tip — no undo log is needed. See [Consensus — Fork choice and reorgs](consensus.md#fork-choice-and-reorgs).

## In the RPC

Objects are returned by `GET /object/{addr}`, `GET /objects_owned/{owner}`, and `GET /objects?address=...`. The wire representation maps directly to the fields above. See [RPC API](rpc.md) for the full response shape.
