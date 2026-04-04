<div align="center">

<img src="docs/assets/meow-cat.svg" alt="MEOW cat mascot" width="280" />

# MEOW

**Minimal Experimental Open Web**

A compact blockchain playground written in Rust.

[![CI](https://github.com/valeriyr/meow/actions/workflows/rust.yml/badge.svg?branch=main&event=push)](https://github.com/valeriyr/meow/actions/workflows/rust.yml)
[![Rust 1.94+](https://img.shields.io/badge/rust-1.94%2B-orange?logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

## Highlights

| Area | Details |
|---|---|
| **Full node** | HTTP API + libp2p gossip networking |
| **CLI toolkit** | Key management, genesis, transactions, and smart contracts |
| **Object model** | Minimal object-based state with owner tracking |
| **VM toolchain** | Compile and execute `.meow` smart contracts |
| **Test coverage** | Unit, integration, e2e, security, and network |

> MEOW is an example project — ready for exploration, experimentation, and sharing.

## Quick Start

```bash
# Build everything
cargo build

# Run the full test suite
cargo test
```

**1. Generate a key**

```bash
meow keytool generate ed25519
```

**2. Create allocations and build genesis**

```csv
0xaaaa..aa,1000000
0xbbbb..bb,2000000
```

```bash
meow genesis build allocations.csv genesis.bin
```

**3. Start a node**

```bash
meow-node run --genesis genesis.bin
```

**4. Query the node**

```bash
meow client get-objects <OWNER_ADDRESS>
```

> See the full walkthrough in [docs/quickstart.md](docs/quickstart.md).

## CLI Reference

| Command | Purpose |
|---------|---------|
| `meow keytool` | Manage local keys |
| `meow genesis` | Build genesis state from a CSV allocation file |
| `meow transaction` | Build and sign transactions |
| `meow smart-contract` | Compile and run `.meow` programs locally |
| `meow client` | Talk to a running node over HTTP |
| `meow say-meow` | 🐱 |
| `meow-node run` | Run a full node |

## Workspace

```text
crates/
├── meow                 CLI binary
├── meow-node            Node binary & RPC layer
├── meow-node-client     HTTP client for node RPC
├── meow-nakamoto        Chain, miner, mempool & storage
├── meow-types           Shared types
├── meow-vm              VM runtime
├── meow-vm-compiler     .meow → bytecode compiler
├── meow-vm-adapter      VM ↔ chain glue
├── meow-vm-types        VM type definitions
├── meow-gossip-network  libp2p networking
├── meow-gossip-types    Network-level types
├── meow-genesis         Genesis loading
├── meow-framework       Built-in modules (meow_coin)
└── meow-e2e-tests       End-to-end & network tests
```

## Documentation

| Doc | What it covers |
|---|---|
| [Quick Start](docs/quickstart.md) | Key generation → genesis → node → first query |
| [Architecture](docs/architecture.md) | Crate map, data flow, and testing strategy |

## Notes

- Default RPC address: `http://127.0.0.1:8600`
- Some e2e tests are serial — they start real nodes and bind ports.

## License

[MIT](LICENSE)
