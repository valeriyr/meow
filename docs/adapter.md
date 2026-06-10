# Adapter & Native Functions

> How the Meow VM connects to the chain, and what native functions the adapter provides.

The **adapter** (`meow-vm-adapter`) is the glue layer between the stack-based Meow VM and the MEOW chain's object store. It:

- Deserializes input objects from BCS into VM values.
- Supplies native function implementations (`meow_vm_fresh_id`, `meow_vm_transfer`, etc.).
- Runs the bytecode verifier before storing any new module.
- Translates VM execution results back into chain effects (created / changed / destroyed objects).
- Provides an `executor` API that drives full transaction execution — see [Executor](#executor) below.
- Provides a `builder` API for compiling `.meow` source into a deployable bytecode module — see [Builder](#builder) below.
- Provides a `runner` API for calling compiled module functions in tests without transaction overhead — see [Runner](#runner) below.

## Executor

The executor bridges chain-level transactions to the VM. Its three entry points are:

| Function | Use |
|----------|-----|
| `execute(transaction, inputs, external_context)` | Normal transaction execution — gas is charged and the gas coin is always returned. |
| `execute_genesis_transaction(transaction, inputs)` | Genesis path — privileged VM config, unlimited gas, no gas coin required. Accepts both `MeowCall` and `MeowModulePublish`. |
| `execute_system_transaction(transaction, inputs)` | System path for block rewards — privileged VM config, unlimited gas, no gas coin required. `MeowCall` only; `MeowModulePublish` is rejected. |

### Inputs

`inputs` must contain every object the transaction references: the module being called, the gas coin, and any object arguments. The executor looks up each object by address from this slice rather than touching the store directly.

### Transaction types

**`MeowCall`** — calls a named `pub fn` in a published module:

1. Resolves the module and its declared dependency modules from `inputs`.
2. Resolves call arguments — object args are matched by `ObjectRef` and must be address-owned by the sender; raw args are BCS-decoded to their declared types.
3. Rejects the call if the function's return type contains a struct (structs must be consumed within the call).
4. Builds the execution context and native functions, then runs the VM.
5. Collects object effects (created / changed / destroyed) into an `ExecutionResult`.

| Limit | Value |
|-------|------:|
| Maximum call arguments | 16 |
| Maximum call stack depth | 256 |
| Maximum dependency modules loaded | 64 |

Only `pub fn` functions are callable.

**`MeowModulePublish`** — stores a new bytecode module on-chain:

1. Checks the serialized module size (≤ 512 KiB).
2. Deserializes the module and runs both the language and adapter bytecode verifiers.
3. Charges gas at **10 gas per serialized byte**.
4. Stores the module as a new object at an address derived from the transaction digest.

The following limits apply when publishing a module:

| Limit | Value |
|-------|------:|
| Maximum serialized module size | 512 KiB |
| Maximum identifier length | 128 chars |
| Maximum structs per module | 128 |
| Minimum fields per struct | 1 |
| Maximum fields per struct | 32 |
| Maximum functions per module | 256 |
| Maximum parameters per function | 16 |
| Maximum local variables per function | 255 |
| Maximum instructions per function | 65 536 |
| Maximum tuple elements | 16 |
| Maximum `use` declarations | 64 |
| Maximum transitive dependency modules | 64 |

### Gas and failure

The gas coin's entire balance is used as the gas budget. A base cost of **1 000 gas** is charged upfront before any VM work. Gas is always deducted and the gas coin is always returned in `changed_objects`, even when execution fails — a failed `ExecutionResult` still carries `gas_used` and the updated coin.

## Builder

The builder compiles `.meow` source into a bytecode `Module` ready for on-chain publishing or local use with the runner.

### Entry points

| Function | Input | Use |
|----------|-------|-----|
| `build(source, deps)` | Source string + pre-loaded dep modules | Compile from an in-memory string |
| `build_from_file(file_path, deps)` | File path + pre-loaded dep modules | Compile from a `.meow` file |
| `extract_module_deps(source)` | Source string | Extract `(name, alias, address)` triples without full compilation — use this to know which dep modules to fetch before calling `build` |

Source input is limited to **64 KiB**. The serialized output must not exceed **512 KiB** — the same limit enforced by the executor at publish time; the builder surfaces this earlier.

### Dependencies

Every `use` declaration in the source must be satisfied by an entry in `deps`. Pass the pre-loaded `Module` values keyed by their on-chain address. `extract_module_deps` is provided specifically to discover which addresses to load before calling `build`.

## Runner

The runner executes a compiled module function directly — with real native functions and a live execution context, but without the gas-coin validation and object-store machinery of the full executor. It is intended for unit-testing contract logic in isolation.

### Entry points

| Function | Use |
|----------|-----|
| `run(module, fn_name, args, deps, external_context)` | Call a `pub fn` with unlimited gas |
| `run_privileged(module, fn_name, args, deps, external_context)` | Call any function, including private ones — useful for testing functions like `mint` that are intentionally private |

The `module` argument is `(Address, Module)` — the address is required to qualify struct type names produced by the VM (e.g. `@0x20::MeowCoin`).

### RunResult

```rust
RunResult {
    return_value: Option<Value>,         // return value of the call, if any
    transfers:    Vec<(Value, Address)>, // objects transferred out: (object, new_owner)
    destroyed:    Vec<Address>,          // IDs of objects destroyed during the call
    gas_spent:    u64,                   // gas units consumed
}
```

Gas is unlimited by default — `gas_spent` reflects what the call actually consumed, not what was budgeted.

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
| `meow_vm_transfer(obj, owner)` | `(local struct, address) → void` | 20 | Transfers ownership of `obj` to `owner` and saves it to the object store. `obj` must be an on-chain object defined in the calling module; the bytecode verifier rejects cross-module or non-object structs at publish time, and a non-object that slips through aborts at execution. |
| `meow_vm_destroy(id)` | `(meow_object::Id) → void` | 10 | Destroys the object identified by `id`. The object is removed from the store at the end of the transaction. |
| `meow_vm_sender()` | `() → address` | 1 | Returns the 32-byte address of the transaction sender. |
| `meow_vm_rand()` | `() → u64` | 10 | Returns the next value from the block's pseudo-random sequence. Deterministic across re-executions; seeded from the block's mining hash and the transaction digest. |
| `meow_vm_timestamp()` | `() → u64` | 1 | Returns the block timestamp in Unix milliseconds. Same for all transactions in a block. |

## The `meow_object` system module

`meow_object` is published at the fixed address `0x10`. It exports a single type:

```meow
pub struct Id {
    inner: address
}
```

`Id` is an opaque wrapper around a 32-byte address. The inner field is private — user code cannot read or manipulate it directly. The only way to obtain an `Id` is via `meow_vm_fresh_id()`.

Any struct whose first field is `id: meow_object::Id` is recognized by the adapter as an **on-chain object**. This is a layout convention enforced at publish time by the bytecode verifier and at execution time by the adapter.

```meow
use meow_object@0x10;

pub struct Hero {
    id: meow_object::Id,   // first field — marks this as an on-chain object
    name: string,
    level: u64
}
```

The `id` field cannot be reassigned — not even within the declaring module. The compiler rejects any write to `id` as a compile error. To destroy an object, destructure it to extract the `id`, then call `meow_vm_destroy`:

```meow
fn burn(c: Coin) {
    let Coin { id, .. } = c;
    meow_vm_destroy(id);
}
```

## On-chain object lifecycle

Every object ID allocated by `meow_vm_fresh_id()` must be either transferred (as part of the struct that holds it) or destroyed before the transaction ends. An ID that is neither transferred nor destroyed causes the transaction to fail.

| State | How reached | Store effect |
|-------|-------------|--------------|
| **Created** | `meow_vm_fresh_id()` then `meow_vm_transfer(obj, owner)` | Inserted |
| **Transferred** | `meow_vm_transfer(obj, new_owner)` | Overwritten with new owner |
| **Destroyed** | destructure to extract `id`, then `meow_vm_destroy(id)` | Removed |

Objects created and destroyed within the same transaction leave no trace in the store.

## The `meow_coin` system module

`meow_coin` is published at the fixed address `0x20`. It is the native coin of the MEOW chain.

See [Meow Coin](meow-coin.md) for the full module reference — types, public functions, and CLI usage.

The gas coin used for transaction fees is a `MeowCoin` object. It is deducted after execution (success or failure) and is always returned in `changed_objects`.

## Bytecode verification

Every module is verified by `meow-vm-bytecode-verifier` before it is stored on-chain. Verification runs as part of `MeowModulePublish` execution — a module that fails is rejected.

The verifier operates on raw `Module` bytecode, independent of whether the bytecode was produced by the compiler or crafted manually.

Both phases below always run; their errors are accumulated and reported together (Phase 2 is not skipped when Phase 1 finds errors).

### Phase 1 — structural checks

- All identifiers (module name, function names, struct names, field names) are valid.
- No duplicate function or struct names.
- Local variable slot indices stay within `local_count`.
- Jump offsets are forward-only and land on a valid instruction index.
- Structs must have at least one field — empty structs are rejected.
- All `Type::Struct` names in struct field types, function parameter types, and return types must resolve to a struct defined in the same module or a fully-qualified type from a registered dependency — unresolved type references are rejected. A referenced dependency struct must also be `pub`: naming a private dependency struct as a field, parameter, or return type is rejected.
- Struct field type definitions must be acyclic — a struct cannot directly or transitively have a field of its own type.
- Struct fields may not have tuple types — only primitives and struct types are allowed.
- `LoadField` and `StoreField` instructions must specify a non-empty field path — an empty path is rejected.
- `NewStruct` and `UnpackStruct` field lists exactly match the struct definition.
- Cross-module `Call` targets are public functions.
- Cross-module `NewStruct` and `UnpackStruct` are forbidden.

### Phase 2 — abstract interpretation

- Stack types tracked through every instruction and across branch join points.
- Return type matches the declared return type.
- Functions without a reachable `Return` are rejected.
- **Struct linearity**: every struct follows move semantics — use-after-move, pop/dup/overwrite on struct slots, comparing structs or tuples containing structs with `==`/`!=`, and unconsumed structs at `Return` are all errors.
- `GetField` on a struct that contains other struct-typed fields is rejected — consuming the struct to extract one primitive field would silently drop any remaining linear fields.
- `LoadField` and `StoreField` on struct-typed fields are rejected — loading would produce an untracked linear value; storing would implicitly drop the old one.
- Native call sites are type-checked against the adapter-supplied signatures.
- Cross-module field reads and writes are rejected.

### Adapter-level checks

After the language-level verifier passes, the adapter runs a second verification pass that enforces chain-specific object conventions:

**Object layout** — two rules keep the object convention (defined above) enforceable: an `id: meow_object::Id` field must be the first field, and no field may itself be an object type (objects cannot be nested inside other structs).

**ID freshness** — every `NewStruct` that constructs an on-chain object must supply an `id` value that originates directly from a `meow_vm_fresh_id()` call within the same function. IDs from parameters, local variables seeded elsewhere, or cross-module calls are rejected.

**Transfer type** — every `meow_vm_transfer` argument must be an on-chain object struct. The verifier tracks each value's type through the function (locals, `Dup`, returns, struct/tuple unpacking), so a non-object struct is rejected at publish time. Values whose type can't be determined statically are left to the runtime check inside `meow_vm_transfer`.

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
