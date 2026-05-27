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

The command prints the new address, public key, and a **mnemonic seed phrase**. Back up the seed phrase now — it is the only way to recover the key if the keystore file is lost. It will not be shown again.

Inspect the keystore:

```bash
meow keytool list
```

## 2. Create Allocations

Create `allocations.csv` with one `<address>,<amount>` pair per line (see [Tokenomics — Initial supply](tokenomics.md#initial-supply) for how these translate to on-chain coin objects):

```csv
0xaa,1000000
0xbb,2000000
```

## 3. Build Genesis

```bash
meow genesis build allocations.csv genesis.bin
```

This produces a BCS-serialized genesis file that `meow-node` can load.

> **All nodes in a network must use the same genesis file.** The genesis defines the initial chain state (framework modules, accounts, and balances) and serves as the shared identity of the network. Nodes using a different genesis file are on an incompatible chain and cannot communicate with the network.

## 4. Start the Node

```bash
meow-node run --genesis genesis.bin
```

<details>
<summary>Node options</summary>

| Flag | Default | Description |
|------|---------|-------------|
| `--genesis` | _(required)_ | Path to a BCS-serialized genesis file |
| `--rpc-listen` | `127.0.0.1:8600` | HTTP API bind address |
| `--listen-address` | `/ip4/0.0.0.0/tcp/0` | libp2p listen address |
| `--bootstrap-peers` | _(none)_ | Multiaddr of an existing peer (repeatable) |
| `--mdns-query-interval` | `300` | Seconds between mDNS re-query broadcasts |
| `--check-explicit-peers-ticks` | `300` | Heartbeat ticks between reconnection attempts to explicit bootstrap peers |
| `--difficulty` | `8` | Proof-of-work leading zero bits |
| `--miner-address` | _(none)_ | Address of the signing key to load from the keystore; if omitted, an ephemeral random keypair is used and rewards are lost on restart |
| `--miner-reward-address` | _(miner address)_ | Address that receives block reward coins; defaults to the miner's own address |
| `--keystore-path` | _(default path)_ | Path to the keystore file; requires `--miner-address` |
| `--batch-size` | `1` | Minimum transactions to queue before starting a mining round; each round drains exactly this many. Must be ≥ 1 and ≤ 256. |
| `--snapshot-depth` | `64` | Block snapshots retained behind the chain head. Sets the maximum safe reorg depth and the threshold above which state sync replaces block sync. Must be ≥ 1. |

</details>

### Running two nodes locally

Nodes discover each other automatically via mDNS when listening on `0.0.0.0`. See [Networking](networking.md) for peer discovery and catch-up sync details. Start each in a separate terminal:

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

To reduce the mDNS discovery wait during local development:

```bash
meow-node run --genesis genesis.bin \
  --listen-address /ip4/0.0.0.0/tcp/30333 \
  --mdns-query-interval 5 \
  --check-explicit-peers-ticks 5
```

Once connected, node 2 detects any block height gap automatically and initiates a catch-up sync — pulling missing blocks for small gaps or fetching a full state snapshot for large ones. See [Networking — Catch-up sync](networking.md#catch-up-sync) for details.

## 5. Query the Node

See [RPC API](rpc.md) for the full response shapes and all available endpoints.

```bash
# Single object
meow client get-object <OBJECT_ADDRESS>

# Fetch specific objects by address (one or more)
meow client get-objects <ADDR1> <ADDR2> ...

# All objects owned by an address
meow client get-objects-owned <OWNER_ADDRESS>

# Committed transaction
meow client get-transaction <TRANSACTION_DIGEST>

# Transaction result (execution effects)
meow client get-transaction-result <TRANSACTION_DIGEST>

# Block by digest
meow client get-block <BLOCK_DIGEST>

# State snapshot at a given block
meow client get-block-snapshot <BLOCK_DIGEST>

# Current chain head digest
meow client get-chain-head
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
meow transaction sign <BASE64_TRANSACTION>

# Submit it
meow client submit-transaction <BASE64_SIGNED_TRANSACTION>
```

## 7. Run a Contract Locally

```bash
meow contract run path/to/module.meow add 3 5
```

> Requires a running node to resolve dependency modules. No transaction is submitted.

To call a private function (e.g. during development), use `run-privileged`:

```bash
meow contract run-privileged path/to/module.meow mint 1000 0xaa
```

## 8. Execute a Transaction Locally

Execute an unsigned transaction locally — objects are fetched from the node but the transaction is not submitted:

```bash
meow transaction execute-locally <BASE64_TRANSACTION>
```

Optional flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--seed` | zero digest | Block hash used as the randomness seed (base58 digest) |
| `--timestamp` | current system time | Execution timestamp in Unix milliseconds |

> **Note:** the actual committed transaction may produce different results if the contract uses `meow_vm_rand()` or `meow_vm_timestamp()` — the real block hash and miner timestamp are unknown until the block is mined.

## 9. Simulate a Transaction

Simulate an unsigned transaction on the node without committing it. The node validates object references and runs the VM against its current state:

```bash
meow transaction simulate <BASE64_TRANSACTION>
```

> **Note:** if the contract uses `meow_vm_rand()` or `meow_vm_timestamp()`, the result of simulation may differ from the result of the actual committed transaction because the block hash and timestamp are unknown until the block is mined.

---

For a full worked example — writing a module, publishing it, calling its functions, and sending coins — see [Contracts](contracts.md).
