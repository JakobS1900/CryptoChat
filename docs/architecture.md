# CryptoChat Architecture Overview (Decentralized Revision)

## System Context

CryptoChat operates as a decentralized, peer-to-peer overlay with the following building blocks:

1. **Peer Node Service (Rust)** — A lightweight daemon bundled with each client. It maintains peer connections over QUIC/libp2p-like transports, participates in the distributed hash table (DHT), manages offline queues, and exposes a local API to the UI shells.
2. **Windows Desktop Client (Tauri + React)** — UI surface that communicates with the local node service, renders conversations, and handles Windows notifications.
3. **Android Client (Jetpack Compose + Kotlin)** — Mobile companion that mirrors desktop functionality, talking to the same local node service via JNI bindings and delivering push notifications through platform channels.

All peers are equal participants. There is **no centralized relay**; optional rendezvous/bootstrap nodes can be self-hosted or community run, similar to Tor directory authorities.

## Networking & Discovery

- **Overlay Transport**: QUIC/WebTransport with Noise-based authentication layered on top of OpenPGP identities. NAT traversal leverages STUN/TURN; if direct connections fail, peers fall back to voluntary relay peers.
- **Peer Discovery**: A Kademlia-style DHT keyed by OpenPGP fingerprints and ephemeral session IDs. Bootstrap lists can be shipped with the app or added manually.
- **Presence & Push**: Peers publish ephemeral presence records to the DHT. When a peer is offline, nearby nodes buffer encrypted payloads until the recipient reconnects. Android/Windows push notifications reference opaque message tokens pulled from the DHT (never plaintext).

## Message Flow

1. **Key Establishment**: Each device owns an OpenPGP identity (generated via `crypto-core`). For group chats, a per-group session key (OpenPGP symmetric key) is maintained; membership changes trigger re-encryption for active members.
2. **Send Path**:
   - UI passes plaintext to the node service.
   - Node signs and encrypts using OpenPGP (per recipient for 1:1, per-member payload for groups).
   - Envelope is gossiped through the overlay. Nearby peers cache the ciphertext until acknowledged.
3. **Receive Path**:
   - Node monitors overlay subscriptions for envelopes addressed to its device fingerprint.
   - Once received, the message is decrypted locally, signature checked, and acknowledged.
   - UI is notified, triggering local display and push notifications as needed.

## Offline Delivery

- Envelopes carry time-to-live metadata and are redundantly stored on a small set of neighboring peers (configurable replication factor).
- Receipts are signed and broadcast so other peers delete cached copies once delivery is confirmed.
- Group membership updates follow a similar path; new keys are distributed as encrypted control messages.

## Deployment Considerations

- **Bootstrap Nodes**: The project can publish reference bootstrap peers, but users may self-host or peer through trusted operators.
- **Security Hardening**: Node service runs as a sandboxed process with minimal filesystem access. Key material never leaves the client.
- **Upgrades**: Nodes advertise supported protocol versions over the DHT to coordinate rolling upgrades without downtime.

## Open Questions

- How many replicas of queued messages are required to balance availability and metadata minimization?
- Which libraries best satisfy the P2P needs (libp2p, Quinn, custom QUIC stack) while remaining audit-friendly?
- What user experience should we present for manual peer exchange (QR/fingerprint verification) to mitigate Sybil attacks?

