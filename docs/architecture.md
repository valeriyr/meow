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
| **meow-gossip-network** | libp2p peer-to-peer networking |
| **meow-gossip-types** | Network-level shared types |
| **meow-genesis** | Genesis file loading and validation |
| **meow-framework** | Built-in modules (`meow_coin`) |
| **meow-e2e-tests** | End-to-end, network, and security tests |

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