# CryptoChat Shared Workspace

Rust workspace containing reusable crates that enforce cross-platform parity.

## Crates

- `crypto-core` — Thin wrapper around Sequoia PGP providing key lifecycle, signing, encryption, and verification APIs.
- `messaging` — Protocol models, serialization, and domain logic for conversations, envelopes, and delivery receipts.
- `bindings/` — Platform bridges exposing the shared crates to the Windows Tauri runtime and Android JNI.

## Next Steps

- Define workspaces in the root `Cargo.toml` and add crate manifests.
- Implement deterministic test suites covering key generation, message signing, and encryption flows.
- Publish C bindings/JNI interfaces for platform integration once the core API solidifies.
