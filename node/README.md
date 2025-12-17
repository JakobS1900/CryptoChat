# CryptoChat Node Service

Peer-to-peer node responsible for overlay participation, message routing, and offline queue replication. Each client bundles this service locally; no centralized relay is required.

## Module Layout

- `overlay/config.rs` — Overlay configuration (bootstrap peers, replication factor, TTLs).
- `overlay/transport.rs` — libp2p transport bootstrap (Noise + QUIC/TCP) and shutdown hooks.
- `overlay/discovery.rs` — Kademlia peer discovery, bootstrap, and peer change events.
- `overlay/replication.rs` — Encrypted envelope replication and receipt publication.
- `overlay/subscriptions.rs` — Event fan-out to the rest of the node and UI bindings.

## Next Steps

- Wire `overlay/transport.rs` to an actual libp2p Swarm with Noise + QUIC/TCP and connection limits.
- Implement discovery bootstrap and peer scoring to mitigate Sybil/Eclipse attacks.
- Flesh out replication strategies (proximity based, receipt-driven cleanup) and integration tests in `tests/overlay/`.
