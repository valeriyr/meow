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
| `GET` | [`/object/{addr}`](#get-objectaddr) | Fetch a single object by address |
| `GET` | [`/objects/{owner}`](#get-objectsowner) | Fetch all objects owned by an address |
| `GET` | [`/transaction/{digest}`](#get-transactiondigest) | Fetch a committed transaction |
| `GET` | [`/transaction-result/{digest}`](#get-transaction-resultdigest) | Fetch a transaction's execution result |
| `GET` | [`/blocks-since/{height}`](#get-blocks-sinceheight) | Fetch all blocks from a given height |

## Common types

```
Address   — 0x-prefixed hex string, 32 bytes (e.g. 0xaa is left-padded)
Digest    — base58-encoded 32-byte hash
ObjectRef — { address: Address, version: u64, digest: Digest }
```

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

**Success:** `202 Accepted` — empty body.

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `400` | `invalid_transaction` | Transaction failed validation |
| `400` | `invalid_object_reference` | Referenced object not found, wrong version, or wrong digest |
| `409` | `duplicate_transaction` | Digest already in mempool |
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
| `404` | `object_not_found` | No live object at that address |
| `500` | `internal_error` | Unexpected error |

## GET /objects/{owner}

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
| `404` | `transaction_not_found` | No committed transaction with that digest |
| `500` | `internal_error` | Unexpected error |

## GET /transaction-result/{digest}

Fetch the execution result of a committed transaction.

**Path parameter:** `digest` — base58 `Digest`

**Success (`200`):**

```json
{
  "status": "Success",
  "transaction_digest": "<Digest>",
  "created_objects":   [ ... ],
  "changed_objects":   [ ... ],
  "destroyed_objects": [ ... ]
}
```

`status` is either `"Success"` or `{ "Failure": "<error message>" }`.  
Each objects array contains `Object` entries (same shape as `/object/{addr}`).

**Errors:** same codes as `GET /transaction/{digest}`.

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
    "state_root":        "<Digest>",
    "timestamp":         1712534400000,
    "nonce":             99312
  },
  "transactions": [ ... ],
  "results":      [ ... ]
}
```

`transactions` and `results` are parallel arrays — index `i` of `results` is the outcome of index `i` of `transactions`.

**Errors:**

| Status | `code` | Cause |
|--------|--------|-------|
| `500` | `internal_error` | Unexpected error |
