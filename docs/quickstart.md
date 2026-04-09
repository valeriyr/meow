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
0xaa,1000000
0xbb,2000000
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
| `--genesis` | _(none)_ | Path to a BCS-serialized genesis file |
| `--rpc-listen` | `127.0.0.1:8600` | HTTP API bind address |
| `--listen-address` | `/ip4/0.0.0.0/tcp/0` | libp2p listen address |
| `--bootstrap-peers` | _(none)_ | Multiaddr of an existing peer (repeatable) |
| `--mdns-query-interval` | `300` | Seconds between mDNS re-query broadcasts |
| `--check-explicit-peers-ticks` | `300` | Heartbeat ticks between reconnection attempts to explicit bootstrap peers |
| `--difficulty` | `8` | Proof-of-work leading zero bits |

</details>

## Running two nodes locally

Nodes discover each other automatically via mDNS when listening on `0.0.0.0`. Start each in a separate terminal:

```bash
# Terminal 1
meow-node run --genesis genesis.bin --rpc-listen 127.0.0.1:8601 --listen-address /ip4/0.0.0.0/tcp/30333

# Terminal 2 — discovers node 1 via mDNS automatically
meow-node run --genesis genesis.bin --rpc-listen 127.0.0.1:8600 --listen-address /ip4/0.0.0.0/tcp/30334
```

mDNS only works on the local network. To connect nodes on different machines, or to skip the discovery wait, pass the first node's address explicitly:

```bash
meow-node run --genesis genesis.bin \
  --rpc-listen 127.0.0.1:8600 \
  --listen-address /ip4/0.0.0.0/tcp/30334 \
  --bootstrap-peers /ip4/<node1-ip>/tcp/30333
```

Node 2 detects any block height gap and pulls the missing range from node 1 automatically. To reduce the mDNS discovery wait during local development:

```bash
meow-node run --genesis genesis.bin \
  --listen-address /ip4/0.0.0.0/tcp/30333 \
  --mdns-query-interval 5   # re-query every 5 seconds
  --check-explicit-peers-ticks 5   # recheck every 5 heartbeat ticks
```

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
meow transaction publish path/to/module.meow --sender <ADDRESS> --gas-coin <OBJECT_ADDRESS>

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

For a full worked example — writing a module, publishing it, calling its functions, and sending coins — see [Contracts](contracts.md).
