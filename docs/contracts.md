# Contracts

> Writing, publishing, and calling smart contracts on MEOW.

Smart contracts are written in the Meow Language. For the complete language reference see [Language Reference](language.md); for native functions, object IDs, and the bytecode verifier see [Adapter & Natives](adapter.md).

## Randomness

`meow_vm_rand()` advances a per-transaction sequence and returns a `u64`. Each call returns the next value in the same sequence — calling it twice in one function gives two different values.

The sequence is seeded from the block's mining hash and the transaction digest, so results are deterministic across re-executions but unpredictable at submission time. Successive calls within the same transaction are independent from other transactions in the block.

`meow_vm_rand()` is appropriate for low-stakes game mechanics or cosmetic variation. Avoid it for outcomes where miner manipulation would be economically rational. See [Consensus — Randomness](consensus.md#randomness) for the full security model.

## Time

`meow_vm_timestamp()` returns the **block timestamp** as Unix milliseconds — the value recorded in the block header by the miner when the block was produced.

```meow
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

See [Consensus — Timestamps](consensus.md#timestamps) for validation rules and miner behavior.

## Transaction entry points

Only `pub fn` functions can be called directly from a transaction. Two things are rejected before execution begins:

- Targeting a private function.
- Targeting a `pub fn` whose return type contains any struct value — either directly or inside a tuple. This restriction applies only at the transaction entry point; the same function can be called freely from other contract functions. Only primitive return values (`bool`, `u64`, `address`, `string`) are allowed at transaction entry points; they are silently ignored by the caller.

Native built-in functions (`meow_vm_transfer`, `meow_vm_fresh_id`, etc.) cannot be called directly from a transaction — they are only available from within contract code.

```meow
mod vault;

fn internal_helper() -> u64 { return 1; }  // cannot be called from a transaction (private)
pub fn mint() -> Coin { ... }              // cannot be called from a transaction (returns a struct)

pub fn get_balance(coin: Coin) -> u64 { ... } // valid — u64 return is discarded
pub fn deposit(amount: u64) { ... }           // valid transaction target
pub fn withdraw(amount: u64) { ... }          // valid transaction target
```

## CLI

### Build

Compiles the source and prints any errors without producing a transaction.

```bash
meow contract build my_module.meow
```

### Run locally

Compiles and executes a function in a local VM without submitting a transaction. A running node is always required — dependency modules are fetched from it. The distinction is in the arguments: primitive arguments (`bool`, `u64`, `address`, `string`) need no additional on-chain objects, while object arguments (`0x<hex>`) require the referenced objects to already exist on-chain.

```bash
meow contract run my_module.meow function_name arg1 arg2
```

### Run locally (privileged)

Like `run`, but allows calling private functions. Useful for testing functions that are intentionally private (e.g. `mint`). Never submits a transaction.

```bash
meow contract run-privileged my_module.meow function_name arg1 arg2
```

### Publish on-chain

```bash
# 1. Build and create the publish transaction
meow transaction publish my_module.meow --sender <ADDRESS> --gas-coin <GAS_COIN_ADDRESS>

# 2. Sign
meow transaction sign <BASE64_TRANSACTION>

# 3. Submit
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

Fetch the result to find the new module's on-chain address:

```bash
meow client get-transaction-result <TRANSACTION_DIGEST>
```

### Call a function

```bash
meow transaction meow-call \
  --module <MODULE_ADDRESS> \
  --function <FUNCTION_NAME> \
  --sender <ADDRESS> \
  --gas-coin <GAS_COIN_ADDRESS> \
  arg1 arg2 ...
```

### Query an object

```bash
meow client get-object <OBJECT_ADDRESS>
```

### Call argument format

| Value | Format | Example |
|-------|--------|---------|
| `bool` | `true` or `false` | `true` |
| `u64` | digits only | `100` |
| Raw address | `@0x<hex>` | `@0xabcd...` |
| On-chain object (resolved) | `0x<hex>` | `0xabcd...` |
| String | any other text | `hello` |

Use `@0x<hex>` for `address`-typed arguments (e.g. an owner or recipient). Use `0x<hex>` (without `@`) for on-chain object arguments — the CLI fetches them from the node and passes them to the VM.

## Bytecode verification

Every module is verified automatically as part of `MeowModulePublish` — a module that fails verification is rejected and never reaches the object store. If your contract compiles without errors, it will also pass verification. See [Adapter & Natives — Bytecode verification](adapter.md#bytecode-verification) for the full rule set.

## System modules

| Module | Address | What it covers |
|--------|---------|----------------|
| `meow_object` | `0x10` | Object identity — the `Id` type used as the first field of every on-chain object |
| [Meow Coin](meow-coin.md) | `0x20` | The built-in coin — transfer, split, merge, burn |

## Examples

| Example | What it covers |
|---------|----------------|
| [Hero game](example-hero-game.md) | Full lifecycle: write, test locally, publish, `spawn`, award XP, level up, `duel`, `transfer`, `retire` |
| [Timelock coin](example-timelock-coin.md) | Using `meow_vm_timestamp()` to lock a `MeowCoin` balance until a future block time |
