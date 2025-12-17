# 🔐 CryptoChat

A peer-to-peer, end-to-end encrypted messaging app with a **PGP-first architecture**. No central servers, no data collection, just secure communication.

![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Android-blue)
![Language](https://img.shields.io/badge/language-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-green)

## ✨ Features

- **🔑 Automated PGP** - Key generation, exchange, signing, and verification happen seamlessly
- **🌐 Peer-to-Peer** - Direct device-to-device messaging with no central relay
- **📱 Cross-Platform** - Native Windows client (Rust + Win32), Android coming soon
- **🔒 Zero Trust** - Messages encrypted client-side, only you and your recipient can read them
- **📦 Lightweight** - ~1.3MB Windows binary (no Electron/web engine bloat)

## 🏗️ Architecture

```
CryptoChat/
├── clients/
│   ├── windows-native/   # Pure Rust + Win32 API (current focus)
│   └── android/          # Jetpack Compose + JNI bindings (planned)
├── shared/
│   ├── crypto-core/      # Sequoia PGP wrapper (Cv25519 keys)
│   └── messaging/        # Message structs, requests, contacts
├── node/                 # libp2p overlay service (future)
└── tests/                # Contract and E2E tests
```

## 🚀 Getting Started

### Prerequisites
- Rust 1.80+ (`rustup update stable`)
- Windows 10/11 for the Windows client

### Build & Run

```bash
# Clone the repository
git clone https://github.com/yourusername/CryptoChat.git
cd CryptoChat

# Build the Windows client
cd clients/windows-native
cargo build --release

# Run the app
./target/release/cryptochat.exe
```

### Quick Test (Two Instances)

```bash
# Terminal 1 - First instance
./target/release/cryptochat.exe --instance 1

# Terminal 2 - Second instance  
./target/release/cryptochat.exe --instance 2
```

## 📖 How It Works

1. **Generate Keys** - Click "Generate Encryption Keys" (30-60 seconds, creates Cv25519 keypair)
2. **Share Your Key** - Copy your public key or generate a QR code
3. **Add Contact** - Paste their public key and click "Import Key"
4. **Connect** - Enter their IP:PORT and click "Start Chat"
5. **Message** - All messages are end-to-end encrypted with PGP

## 🔐 Security Model

| Layer | Technology |
|-------|------------|
| Encryption | Sequoia PGP (OpenPGP standard) |
| Key Type | Cv25519 (modern elliptic curve) |
| Key Storage | Windows Credential Manager + DPAPI |
| Transport | TCP with length-prefixed JSON envelopes |
| Signing | All messages signed for authenticity |

## 🗺️ Roadmap

- [x] Windows native client MVP
- [x] PGP key generation and storage
- [x] QR code key exchange
- [x] Message request workflow
- [x] Bidirectional encrypted messaging
- [ ] Message history persistence (SQLite)
- [ ] libp2p overlay for NAT traversal
- [ ] Android client (Jetpack Compose)
- [ ] iOS client

## 🤝 Contributing

Contributions are welcome! Please read the codebase guidelines in `AGENTS.md` before submitting PRs.

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

---

**⚠️ Note**: This project is under active development. Use at your own risk for sensitive communications.
