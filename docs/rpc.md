# RPC API

> The node's JSON-over-HTTP interface.

`meow-node` listens on `127.0.0.1:8600` by default. Every response body is JSON. Non-2xx responses carry an error object:

```json
{ "code": "<machine_readable_code>", "message": "<human readable message>" }
```

The `meow client` CLI commands and the `meow-node-client` library crate are both built on top of this API.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | [`/submit-transaction`](#post-submit-transaction) | Add a signed transaction to the mempool |
| `POST` | [`/simulate-transaction`](#post-simulate-transaction) | Simulate an unsigned transaction without committing it |
| `GET` | [`/object/{addr}`](#get-objectaddr) | Fetch a single object by address |
| `GET` | [`/objects`](#get-objects) | Fetch objects by a list of addresses |
| `GET` | [`/objects_owned/{owner}`](#get-objects_ownedowner) | Fetch all objects owned by an address |
| `GET` | [`/transaction/{digest}`](#get-transactiondigest) | Fetch a committed transaction |
| `GET` | [`/transaction-result/{digest}`](#get-transaction-resultdigest) | Fetch a transaction's execution result |
| `GET` | [`/block/{digest}`](#get-blockdigest) | Fetch a committed block by its hash |
| `GET` | [`/block-snapshot/{digest}`](#get-block-snapshotdigest) | Fetch the state snapshot at a given block |
| `GET` | [`/chain-head`](#get-chain-head) | Fetch the current chain head digest |
| `GET` | [`/blocks-since/{height}`](#get-blocks-sinceheight) | Fetch all blocks from a given height |
| `GET` | [`/state-snapshot`](#get-state-snapshot) | Fetch the full chain state at the current head |

## Common types

```
Address   — 0x-prefixed hex string, 32 bytes (e.g. 0xaa is left-padded)
Digest    — base58-encoded 32-byte hash
ObjectRef — { address: Address, version: u64, digest: Digest }
```

See [Object Model](object-model.md) for how `ObjectRef` is used for versioned references and replay protection.

## POST /submit-transaction

Submit a signed transaction to the local mempool and broadcast it via gossip.

**Request body** — `SignedTransaction`:

```json
{
  "transaction": {
    "sender":   "<Address>",
    "gas_coin": { "address": "<Address>", "version": 0, "digest": "<Digest>" },
    "type_": {
      "MeowCall": {
        "module":    "<Address>",
        "function":  "<function_name>",
        "arguments": [ ... ]
      }
    }
  },
  "signature": "<base64-encoded signature>"
}
```

The `type_` field is a tagged union. The two variants are:

| Variant | Shape | Use |
|---------|-------|-----|
| `MeowCall` | `{ module, function, arguments }` | Call a function in a published module |
| `MeowModulePublish` | `[<bytes>]` | Publish a new module (BCS-serialised bytes) |

Each entry in `arguments` is also a tagged union:

| Variant | Shape | Use |
|---------|-------|-----|
| `Object` | `{ "Object": <ObjectRef> }` | Pass an on-chain object by reference |
| `Raw` | `{ "Raw": [<bytes>] }` | Pass a BCS-serialised primitive value |

**Success (`202 Accepted`):** empty body.

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_transaction` | Transaction failed validation |
| `400` | `invalid_object_reference` | Referenced object not found, wrong version, or wrong digest |
| `409` | `duplicate_transaction` | Digest already in mempool |
| `500` | `internal_error` | Unexpected error |

## POST /simulate-transaction

Simulate an unsigned transaction against the node's current state without committing it. Useful for dry-runs, estimating gas, and testing contract logic.

**Request body** — `Transaction` (same shape as the `transaction` field inside `SignedTransaction`, but without a signature):

```json
{
  "sender":   "<Address>",
  "gas_coin": { "address": "<Address>", "version": 0, "digest": "<Digest>" },
  "type_": {
    "MeowCall": {
      "module":    "<Address>",
      "function":  "<function_name>",
      "arguments": [ ... ]
    }
  }
}
```

**Success (`200`):** `ExecutionResult` (same shape as [`/transaction-result/{digest}`](#get-transaction-resultdigest)).

> **Note:** if the contract uses `meow_vm_rand()` or `meow_vm_timestamp()`, the result may differ from the actual committed transaction because the block hash and timestamp are unknown until the block is mined.

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_transaction` | Transaction failed structural validation |
| `400` | `invalid_object_reference` | Referenced object not found, wrong version, or wrong digest |
| `400` | `simulation_error` | Transaction failed during execution (VM error, abort, etc.) |
| `500` | `internal_error` | Unexpected error |

## GET /object/{addr}

Fetch the latest live object at an address.

**Path parameter:** `addr` — `Address`

**Success (`200`):**

```json
{
  "address":     "<Address>",
  "owner":       { "Address": "<Address>" },
  "transaction": "<Digest>",
  "version":     42,
  "type_":       { "Object": { "module": "<Address>", "name": "Hero" } },
  "content":     [<BCS bytes>]
}
```

`owner` is either `{ "Address": "<Address>" }` or `"Immutable"`.  
`type_` is either `"Module"` or `{ "Object": { "module": "<Address>", "name": "<TypeName>" } }`.  
`content` is the BCS-encoded object data.

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_address` | `addr` is not a valid address |
| `500` | `internal_error` | Unexpected error |

> **Not found:** returns `200` with a `null` body when no live object exists at that address.

## GET /objects

Fetch live objects for a list of addresses in one request. Each entry in the response corresponds positionally to the requested address — `null` means no live object exists at that address.

**Query parameters:** one or more `address` values, each an `Address`. Maximum **100 addresses** per request.

Example: `GET /objects?address=0xabc...&address=0xdef...`

**Success (`200`):** array of nullable `Object` (same shape as `/object/{addr}`):

```json
[ { "address": "0xabc...", ... }, null, { "address": "0xdef...", ... } ]
```

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `too_many_addresses` | More than 100 addresses in a single request |
| `400` | `invalid_address` | One of the `address` values is not a valid address |
| `500` | `internal_error` | Unexpected error |

## GET /objects_owned/{owner}

Fetch all live objects owned by an address. Returns an empty array when the owner has no objects.

**Path parameter:** `owner` — `Address`

**Success (`200`):** array of `Object` (same shape as `/object/{addr}`).

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_address` | `owner` is not a valid address |
| `500` | `internal_error` | Unexpected error |

## GET /transaction/{digest}

Fetch a committed transaction by its digest.

**Path parameter:** `digest` — base58 `Digest`

**Success (`200`):** `SignedTransaction` (same shape as the submit-transaction request body).

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_digest` | Not a valid base58 digest |
| `500` | `internal_error` | Unexpected error |

> **Not found:** returns `200` with a `null` body when no committed transaction has that digest.

## GET /transaction-result/{digest}

Fetch the execution result of a committed transaction.

**Path parameter:** `digest` — base58 `Digest`

**Success (`200`):**

```json
{
  "status":             "Success",
  "transaction_digest": "<Digest>",
  "gas_used":           42,
  "created_objects":    [ ... ],
  "changed_objects":    [ ... ],
  "destroyed_objects":  [ ... ]
}
```

`status` is either `"Success"` or `{ "Failure": "<error message>" }`.  
`gas_used` is the number of gas units consumed by the transaction.  
Each objects array contains `Object` entries (same shape as `/object/{addr}`).

**Errors:** same codes as `GET /transaction/{digest}`.

> **Not found:** returns `200` with a `null` body when no result exists for that digest.

## GET /block/{digest}

Fetch a committed block by its hash.

**Path parameter:** `digest` — base58 `Digest`

**Success (`200`):** `Block` (same shape as entries in [`/blocks-since/{height}`](#get-blocks-sinceheight)).

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_digest` | Not a valid base58 digest |
| `500` | `internal_error` | Unexpected error |

> **Not found:** returns `200` with a `null` body when the block is unknown (never seen by this node) or has been pruned (fell more than `snapshot_depth` blocks behind the chain head).

## GET /block-snapshot/{digest}

Fetch the full state snapshot at a specific block: the block itself and all live objects in the store at that point.

**Path parameter:** `digest` — base58 `Digest`

**Success (`200`):** `StateSnapshot` (same shape as [`/state-snapshot`](#get-state-snapshot)).

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_digest` | Not a valid base58 digest |
| `500` | `internal_error` | Unexpected error |

> **Not found:** returns `200` with a `null` body when the block is unknown or has been pruned. Blocks and their snapshots are pruned together once they fall more than [`snapshot_depth`](consensus.md#fork-choice-and-reorgs) blocks behind the chain head.

## GET /chain-head

Fetch the digest of the current best block (chain head).

**Success (`200`):** base58 `Digest` string:

```json
"5TT8P9mPy7Dk..."
```

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `500` | `internal_error` | Unexpected error |

## GET /blocks-since/{height}

Fetch all committed blocks from `height` onwards (inclusive). Returns an empty array when `height` is beyond the current chain tip. Used by nodes during chain sync.

**Path parameter:** `height` — `u64`

**Success (`200`):** array of `Block`:

```json
{
  "header": {
    "height":            42,
    "parent_hash":       "<Digest>",
    "transactions_root": "<Digest>",
    "reward_root":       "<Digest>" | null,
    "state_root":        "<Digest>",
    "timestamp":         1712534400000,
    "nonce":             99312
  },
  "transactions":             [ ... ],
  "results":                  [ ... ],
  "reward_transaction":       { ... } | null,
  "reward_transaction_result": { ... } | null
}
```

`transactions` and `results` are parallel arrays — index `i` of `results` is the outcome of index `i` of `transactions`.

`reward_transaction` and `reward_transaction_result` are `null` for blocks where all user transactions paid zero gas.

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `500` | `internal_error` | Unexpected error |

## GET /state-snapshot

Fetch a full state snapshot at the current chain head: the head block and all live objects in the store. Used by nodes when the height gap to a peer exceeds [`snapshot_depth`](consensus.md#fork-choice-and-reorgs) and block-by-block replay would be prohibitively slow.

**Success (`200`):** `StateSnapshot`:

```json
{
  "head": {
    "header": {
      "height":             42,
      "parent_hash":        "<Digest>",
      "transactions_root":  "<Digest>",
      "reward_root":        "<Digest>" | null,
      "state_root":         "<Digest>",
      "timestamp":          1712534400000,
      "nonce":              99312
    },
    "transactions":              [ ... ],
    "results":                   [ ... ],
    "reward_transaction":        { ... } | null,
    "reward_transaction_result": { ... } | null
  },
  "objects": [ ... ]
}
```

`objects` is the complete list of live objects at the head block — same shape as `/object/{addr}`.

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `500` | `internal_error` | Unexpected error |
