# Networking

> How Meow nodes discover peers and exchange transactions and blocks.

The networking layer (`meow-gossip-network`) uses libp2p gossipsub for pub/sub messaging and mDNS for local peer discovery. Each node subscribes to three topics on startup and routes incoming messages to the chain and mempool.

## Transport

All connections use TCP with [Noise](https://noiseprotocol.org/) encryption and Yamux stream multiplexing. Each node generates a fresh identity keypair on startup; gossipsub messages are signed with that key.

## Peer discovery

**Local network** — mDNS multicast. Discovered peers are added as explicit gossipsub peers; expired peers are removed. The query interval defaults to 5 minutes.

**Cross-machine** — Bootstrap peers supplied in the node config. The node dials each bootstrap address on startup and maintains the connection across reconnects.

## Topics

Three topics carry all node-to-node traffic:

| Topic | Payload | Description |
|-------|---------|-------------|
| `transactions` | BCS-encoded `SignedTransaction` | Submitted transactions broadcast to the network for inclusion in blocks. |
| `blocks` | BCS-encoded `Block` | Mined blocks broadcast to the network for validation and chain extension. |
| `peer-info` | UTF-8 RPC URL | Each node broadcasts its HTTP RPC URL to newly subscribing peers. |

The `peer-info` exchange is automatic: when a peer subscribes to the topic, the node immediately publishes its own RPC URL. This is how nodes discover each other's HTTP endpoints for chain sync.

## Catch-up sync

When a node receives a block whose height is more than one ahead of its local tip, it knows blocks are missing and initiates a sync:

1. Transitions to **Syncing** state.
2. Buffers any blocks that continue to arrive over gossip during the sync (deduplicated by hash).
3. Calls [`GET /blocks-since/{local_height+1}`](rpc.md#get-blocks-sinceheight) on the peer that sent the gap block, or any other known peer if the sender's URL is not yet known.
4. Sorts the pulled blocks by height and applies them to the chain.
5. Applies the buffered gossip blocks in height order.
6. Returns to **Working** state.

If no peer RPC URL is known yet when the gap is detected, the sync request is skipped and a warning is logged. Blocks that arrive over gossip continue to buffer until a sync completes.

## Configuration

`GossipNetworkConfig` controls the networking layer:

| Field | Default | Description |
|-------|---------|-------------|
| `listen_address` | — | Multiaddr to listen on, e.g. `/ip4/0.0.0.0/tcp/0` for any available port. |
| `bootstrap_peers` | `[]` | Multiaddrs of known peers to dial on startup. Empty on a local-only node. |
| `mdns_query_interval` | 300 s | How often mDNS re-broadcasts discovery queries. |
| `check_explicit_peers_ticks` | 300 | Heartbeat ticks between reconnection attempts to explicit peers. |
