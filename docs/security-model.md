# CryptoChat Security Model (Decentralized Draft)

## Threat Assumptions

- Adversaries can observe, replay, or tamper with traffic across the peer-to-peer overlay.
- Bootstrap/DHT peers may be malicious (Sybil, Eclipse, or traffic analysis).
- Devices can be intermittently offline, compromised, or physically confiscated; key protection must remain device scoped.
- Push notification providers (Windows toast, Android FCM) are untrusted and must not receive plaintext or metadata beyond opaque tokens.

## Controls

- **OpenPGP Identity-first**: Every device owns an OpenPGP key pair. All transport sessions and envelopes are signed and encrypted per device or per group member.
- **Reproducible Crypto**: The `crypto-core` crate remains deterministic for audits but switches to real OpenPGP bindings for production. All cryptographic operations run locally.
- **Overlay Privacy**: Metadata stored in the DHT is minimized (fingerprint, relay tokens, TTL). Payload replication uses onion-style hop encryption to avoid revealing routes.
- **Group Sessions**: Group chats maintain symmetric session keys encrypted to each member’s OpenPGP key. Membership changes trigger automatic key rotation.
- **Offline Queues**: Encrypted messages are cached by a quorum of neighboring peers (default k=3) and deleted after signed receipt or TTL expiry.
- **Push Notifications**: Clients publish opaque wake tokens (no content) to push channels. Receipt of a notification prompts the client to poll the overlay.

## Operational Guidance

- Encourage manual fingerprint verification (QR codes, short auth strings) before first messaging.
- Support optional trusted bootstrap lists; users can add their own to mitigate Sybil-heavy environments.
- Provide continuous monitoring of anomalous overlay behavior (excessive routing failures, replay attempts).

## Open Items

- Choose the final P2P library (libp2p, Quinn + custom DHT, Yggdrasil) and audit its security posture.
- Design user-facing flows for bootstrap node curation and revocation.
- Decide on replication factors and TTLs that balance availability with traceability risk.
