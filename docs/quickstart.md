# Quick Start

> Key generation, genesis, node startup, and first query from the terminal.

## Prerequisites

```bash
cargo build
```

This produces two binaries: `meow` (CLI) and `meow-node` (full node).

## 1. Generate a Key

```bash
meow keytool generate ed25519
```

Inspect the keystore:

```bash
meow keytool list
```

## 2. Create Allocations

Create `allocations.csv` with one `<address>,<amount>` pair per line:

```csv
0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1000000
0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,2000000
```

## 3. Build Genesis

```bash
meow genesis build allocations.csv genesis.bin
```

This produces a BCS-serialized genesis file that `meow-node` can load.

## 4. Start the Node

```bash
meow-node run --genesis genesis.bin
```

<details>
<summary>Node options</summary>

| Flag | Default | Description |
|------|---------|-------------|
| `--rpc-listen` | `127.0.0.1:8600` | HTTP API bind address |
| `--listen-address` | `/ip4/0.0.0.0/tcp/0` | libp2p listen address |
| `--bootstrap-peers` | — | Multiaddr of an existing peer |
| `--difficulty` | `8` | Proof-of-work leading zero bits |

</details>

## 5. Query the Node

```bash
# Single object
meow client get-object <OBJECT_ADDRESS>

# All objects for an owner
meow client get-objects <OWNER_ADDRESS>

# Committed transaction
meow client get-transaction <TRANSACTION_DIGEST>

# Transaction result (execution effects)
meow client get-transaction-result <TRANSACTION_DIGEST>
```

## 6. Build, Sign and Submit a Transaction

```bash
# Build a publish transaction
meow transaction publish path/to/module.meow \
  --sender <ADDRESS> --gas-coin <OBJECT_ADDRESS>

# Or build a call transaction
meow transaction meow-call \
  --module <MODULE_ADDRESS> --function <FUNCTION_NAME> \
  --sender <ADDRESS> --gas-coin <OBJECT_ADDRESS> [ARGS...]

# Sign it
meow transaction sign --transaction <BASE64_TRANSACTION>

# Submit it
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

## 7. Run a Contract Locally

```bash
meow smart-contract run path/to/module.meow add 3 5
```

> This executes locally through the CLI. No transaction is submitted.
