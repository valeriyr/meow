<div align="center">

<img src="docs/assets/meow-cat.svg" alt="MEOW cat mascot" width="280" />

# MEOW

**Minimal Experimental Open Web**

A compact blockchain playground written in Rust.

[![CI](https://github.com/valeriyr/meow/actions/workflows/rust.yml/badge.svg?branch=main&event=push)](https://github.com/valeriyr/meow/actions/workflows/rust.yml)
[![Rust 1.94+](https://img.shields.io/badge/rust-1.94%2B-orange?logo=rust)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

## What is MEOW?

MEOW is a self-contained blockchain written from scratch in Rust. It is designed to be small enough to read and understand in full, while still covering the core ideas of a production system: consensus, a typed object model, a smart contract VM, and a peer-to-peer gossip network.

The goal is not to ship a coin — it is to show how the pieces fit together.

## How it works

**[Consensus](docs/consensus.md)** — Nakamoto proof-of-work. The miner continuously attempts blocks over the pending mempool transactions. On reorg, only transactions that are no longer valid against the new chain head are evicted; the rest stay in the mempool and are re-mined automatically.

**[Object model](docs/object-model.md)** — State is a flat map of typed objects, each identified by a 32-byte address and owned by a key pair. Transactions consume and produce objects. The gas coin is itself an object, so fees are enforced the same way as any other balance movement.

**[Smart contracts](docs/contracts.md)** — Programs are written in the Meow Language, compiled to bytecode by `meow-vm-compiler`. The VM executes bytecode against the object store inside a transaction. Contracts can be run locally through the CLI without touching a node.

**[Networking](docs/networking.md)** — Nodes communicate over [libp2p](https://libp2p.io/) gossipsub. Peers are discovered automatically on the local network via mDNS; cross-machine peering uses explicit bootstrap addresses. When a node receives a block whose height is more than one ahead of its own chain tip, it requests the missing range from the sender and buffers any blocks that arrive during the catch-up.

**[RPC](docs/rpc.md)** — `meow-node` exposes a JSON-over-HTTP API on `127.0.0.1:8600` by default. The `meow client` CLI sub-commands and the `meow-node-client` library crate both talk to this API.

## Build & test

```bash
cargo build
cargo test
```

## Workspace

```text
crates/
├── meow                 CLI binary (keys, genesis, transactions, contracts, client)
├── meow-node            Full node binary — HTTP RPC + gossip integration
├── meow-node-client     Typed HTTP client library for the node RPC
├── meow-nakamoto        Chain, block validation, miner, mempool, object store
├── meow-nakamoto-types  Block and block-header type definitions
├── meow-types           Shared types — addresses, digests, objects, transactions, keys
├── meow-vm              Smart contract runtime
├── meow-vm-compiler              .meow source → bytecode compiler
├── meow-vm-bytecode-verifier     bytecode verifier — runs at publish time
├── meow-vm-adapter               VM ↔ chain glue layer
├── meow-vm-types                 VM type definitions
├── meow-gossip-network  libp2p gossipsub + mDNS networking
├── meow-gossip-types    Network-level shared types and config
├── meow-genesis         Genesis file loading and validation
├── meow-framework       System framework modules
├── meow-vm-examples     Runnable smart contract examples
└── meow-e2e-tests       End-to-end, network, and security tests
```

## Documentation

| Doc | What it covers |
|---|---|
| [Quick Start](docs/quickstart.md) | Key generation, genesis, running a node, sending transactions |
| [Object Model](docs/object-model.md) | Objects, ownership, versioning, gas coins, and lifecycle |
| [Consensus](docs/consensus.md) | PoW, block validation rules, fork choice, reorgs, mempool, timestamps, and randomness |
| [Networking](docs/networking.md) | Peer discovery, gossip topics, catch-up sync, and node configuration |
| [RPC API](docs/rpc.md) | HTTP endpoints — submit transactions, query objects and blocks |
| [Contracts](docs/contracts.md) | Practical guide: types, native functions, access control, bytecode verification |
| [Language Reference](docs/language.md) | Complete Meow Language syntax and type-system reference |
| [Adapter & Natives](docs/adapter.md) | Native functions, on-chain object lifecycle, bytecode verifier, gas metering |
| [Meow Coin](docs/meow-coin.md) | Built-in system coin reference |
| [Example: hero game](docs/example-hero-game.md) | Full contract lifecycle walkthrough |
| [Example: timelock coin](docs/example-timelock-coin.md) | Time-locked coin using `meow_vm_timestamp()` |
| [Architecture](docs/architecture.md) | Crate map, subsystems overview, data flow, and testing strategy |

## License

[MIT](LICENSE)
