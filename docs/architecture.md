# Architecture

> Rust workspace with a clean split between types, chain logic, networking, node runtime, VM toolchain, and end-user CLIs.

## Crates at a Glance

| Crate | Role |
|-------|------|
| **meow** | CLI — keys, genesis, transactions, contracts, node client |
| **meow-node** | Full node — HTTP RPC, gossip integration, startup |
| **meow-node-client** | Typed HTTP client for node endpoints |
| **meow-nakamoto** | Chain state, block validation, mempool, mining, object store |
| **meow-nakamoto-types** | Block and block-header type definitions |
| **meow-types** | Addresses, digests, objects, transactions, keys |
| **meow-vm** | Smart contract runtime |
| **meow-vm-compiler** | The Meow Language source → bytecode |
| **meow-vm-bytecode-verifier** | Bytecode verifier — structural and type-safety checks at publish time |
| **meow-vm-adapter** | VM ↔ chain glue |
| **meow-vm-types** | VM type definitions |
| **meow-gossip-network** | libp2p gossipsub + mDNS networking |
| **meow-gossip-types** | Network-level shared types and config |
| **meow-genesis** | Genesis file loading and validation |
| **meow-framework** | System framework modules |
| **meow-vm-examples** | Runnable example contracts with integration tests |
| **meow-e2e-tests** | End-to-end, network, and security tests |

## Subsystems

**[Consensus](consensus.md)** — Nakamoto proof-of-work. `meow-nakamoto` owns the chain, block validation, mempool, miner, and object store. `meow-nakamoto-types` holds the block and block-header definitions shared between it and the networking layer.

**[Object model](object-model.md)** — State is a flat map of typed objects, each identified by a 32-byte address and owned by a key pair. Transactions consume and produce objects; the gas coin is itself an object. Core types live in `meow-types`.

**[Smart contracts](contracts.md)** — Programs are written in the [Meow Language](language.md) and compiled to bytecode by `meow-vm-compiler`. `meow-vm-bytecode-verifier` validates modules at publish time. `meow-vm` executes bytecode; `meow-vm-adapter` bridges the VM to the chain's object model — see [Adapter & Native Functions](adapter.md). Shared VM type definitions live in `meow-vm-types`. The `meow-framework` crate provides the built-in system modules (`meow_object`, `meow_coin`).

**[Networking](networking.md)** — Nodes communicate over libp2p gossipsub. Peers are discovered on the local network via mDNS; cross-machine peering uses explicit bootstrap addresses. `meow-gossip-network` owns the transport; `meow-gossip-types` provides shared message types and config.

**[RPC](rpc.md)** — `meow-node` exposes a JSON-over-HTTP API on `127.0.0.1:8600` by default. `meow-node-client` is the typed HTTP client library for that API, used by the `meow` CLI and in tests.

## Data Flow

```text
   User                CLI               Node              Chain
    │                   │                  │                  │
    │  sign transaction │                  │                  │
    │──────────────────►│                  │                  │
    │                   │  POST /submit-transaction           │
    │                   │─────────────────►│                  │
    │                   │                  │  validate + mine │
    │                   │                  │─────────────────►│
    │                   │                  │                  │
    │                   │                  │  commit block    │
    │                   │                  │◄─────────────────│
    │                   │  GET /object/{addr}                 │
    │                   │─────────────────►│                  │
    │                   │       200 OK     │                  │
    │                   │◄─────────────────│                  │
    │  display result   │                  │                  │
    │◄──────────────────│                  │                  │
```

## Testing Strategy

| Layer | Scope | Examples |
|-------|-------|---------|
| **Unit** | Data structures, helpers | Address parsing, BCS round-trips |
| **Integration** | Crate-level behavior | CLI commands, store operations |
| **End-to-end** | Full node lifecycle | Submit tx → query result |
| **Security** | Invalid inputs, attack vectors | Malformed transactions, double spends |
| **Network** | Multi-node scenarios | Peer discovery, block propagation |

> Some e2e tests are serial — they start real nodes, bind ports, and depend on discovery timing.