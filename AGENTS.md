# CryptoChat Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-11-04

## Project Overview

CryptoChat is a cross-platform, peer-to-peer secure messaging platform inspired by Signal with a PGP-first architecture.

## Active Technologies

### Core Stack
- **Rust 1.80+**: Core libraries, node service, and platform bindings
- **Sequoia PGP**: OpenPGP implementation for automated encryption/decryption
- **libp2p 0.54**: Decentralized overlay networking (Kademlia DHT, request-response, QUIC/TCP transport)
- **Tokio**: Async runtime for node service
- **Sled 0.34**: Embedded key-value store for overlay queues and local state

### Windows Desktop
- **windows-rs 0.58**: Official Microsoft Rust bindings for Win32 API
- **WinUI 3**: Native Windows 11 UI framework (future target)
- **Win32 API**: Direct Windows system calls for window creation and rendering

### Android Mobile
- **Kotlin 1.9+**: Primary language
- **Jetpack Compose**: Modern declarative UI
- **Coroutines**: Async/concurrency primitives
- **JNI**: Native bindings to Rust core

## Commands

### Rust Development
```bash
cargo fmt                        # Format all Rust code
cargo check                      # Check workspace for compilation errors
cargo test -- --nocapture        # Run all tests with verbose output
cargo build --release            # Build release binaries
cargo run -p cryptochat-node     # Run the node service locally
```

### Windows Native Client
```bash
cd clients/windows-native
cargo check                      # Verify compilation
cargo build --release            # Build optimized .exe
cargo run                        # Launch native window
```

### Spec Kit Workflow
```bash
# Refresh Codex agent context after updating specs/plans
powershell -NoLogo -Command "$env:SPEC_ID='001-secure-messaging'; .\.specify\scripts\powershell\update-agent-context.ps1 -AgentType codex"
```

## Code Style

### Rust Guidelines
- **Edition**: Rust 2021
- **Formatting**: Use `rustfmt` with default settings (`cargo fmt`)
- **Linting**: Enable Clippy warnings (`cargo clippy -- -D warnings`)
- **Error Handling**: Use `anyhow::Result` for application errors, `thiserror` for libraries
- **Async**: Use `tokio` for async runtime, prefer `async/await` over `raw futures
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types
- **Documentation**: Document all public APIs with doc comments (`///`)
- **Safety**: Avoid `unsafe` without RFC and thorough documentation (Win32 APIs may require `unsafe`)

## Architecture Principles

### Constitution Compliance
Refer to `.specify/memory/constitution.md` for governing principles:

1. **Frictionless PGP Security**: All cryptographic operations automated via `shared/crypto-core`
2. **Cross-Platform Parity**: Shared Rust core ensures consistent crypto/messaging logic
3. **Privacy by Architecture**: Overlay peers only handle encrypted payloads
4. **Reproducible Trust**: Deterministic builds, signed artifacts, comprehensive crypto tests
5. **Humane Secure UX**: Security defaults enabled without user configuration
6. **Native UI Only**: Windows uses WinUI 3 + windows-rs; Android uses Jetpack Compose. Web-based frameworks (Electron, Tauri, WebView) are prohibited to eliminate browser engine attack surface.

### Key Design Decisions
- **Decentralized Architecture**: No centralized message relay; every device runs a peer node
- **Shared Rust Core**: Single source of truth for crypto, messaging, and overlay protocols
- **Native Platform UI**: Direct OS API access for minimal attack surface
- **Overlay Networking**: libp2p-based DHT with request-response for message replication
- **Local-First Storage**: Encrypted SQLite for user state, embedded KV store for queues

### Security Model
- **End-to-End Encryption**: OpenPGP (via Sequoia) for all message payloads
- **Transport Security**: TLS 1.3 with forward secrecy on all network connections
- **Key Storage**: OS-provided secure storage (Windows Credential Manager, Android Keystore)
- **Trust Verification**: Automated signature verification with trust warnings on key changes
- **No Browser Engine**: Native UI eliminates HTML/JS/WebView attack vectors

## Performance Goals
- **Crypto Operations**: <150 ms per message (encrypt/sign or decrypt/verify)
- **Online Message Delivery**: 95% delivered within 5 seconds
- **Offline Delivery**: Within 5 minutes when replica quorum available
- **Onboarding**: Complete device-to-device setup and first message in <3 minutes

## Recent Changes

### 2025-11-05: Message Request System (In Progress) 🚧
**Status**: Foundation built - implementing one-way message requests for better UX
**Problem**: Current flow requires both users to exchange QR codes. Users want Alice to send Bob a message request without Bob having to share his keys first.

**Solution Design**:
1. Bob shares QR code publicly
2. Alice scans Bob's QR (auto-generates ephemeral keys if needed)
3. Alice sends message request to Bob
4. Bob sees pending request inbox with accept/reject options
5. After acceptance → normal two-way encrypted chat

**Completed (Phase 1 - Data Layer)** ✅:
- Created `shared/messaging/src/requests.rs` module with:
  - `MessageRequest` struct (tracks request ID, status, sender fingerprint/key, timestamps)
  - `RequestStatus` enum (Pending/Accepted/Rejected)
  - `Contact` struct (created after accepting requests)
- Created `clients/windows-native/src/request_store.rs` for persistence:
  - JSON-based storage in `%USERPROFILE%/.cryptochat/`
  - `requests.json` - stores all message requests
  - `contacts.json` - stores accepted contacts
  - Helper functions: load/save/delete requests and contacts
- Fixed UI button overlap issues (Scan QR + peer address positioning)
- Fixed QR code generation (switched to EcLevel::L for larger payloads)
- Cleaned up debug logging from production code

**Completed (Phase 2 - Auto-Key Generation)** ✅:
- Modified "Scan QR Code" button handler to detect if user has keys
- If no keys exist, prompt user: "Generate keys now to send a message request?"
- Auto-generate keys silently in background (30-60s)
- After generation, automatically re-prompt for QR scan
- Updated WM_KEYGEN_COMPLETE handler to support two modes:
  - Manual (wparam=0): Show full key details + fingerprint
  - Auto (wparam=1): Show brief success + immediate QR scan prompt
- Keys are saved to Windows Credential Manager in both modes
- Success message changed from "QR scanned" to "Contact added! You can now send them a message request"

**How it works now**:
1. Alice clicks "Scan QR Code" without having keys
2. Prompt: "Generate keys to send message request?"
3. Keys generate silently (30-60s wait)
4. "Keys generated! Now scan your contact's QR code"
5. File picker opens automatically
6. Alice scans Bob's QR → "Contact added!"
7. Alice can now send message request to Bob

**Completed (Phase 3 - Request Inbox UI)** ✅:
- Created separate "Message Requests" window ([requests.rs](clients/windows-native/src/ui/requests.rs))
- Added "📬 View Requests" button to main onboarding screen (top-right)
- Request window UI includes:
  - List box showing all pending requests with shortened fingerprints
  - Fingerprint display area (shows full fingerprint when request selected)
  - Message preview area (for first message in request)
  - ✓ Accept Request button
  - ✗ Reject Request button
  - 🔄 Refresh button to reload requests from disk
- Window procedure handles: WM_CREATE, WM_COMMAND, WM_DESTROY
- `populate_requests_list()` loads from `request_store::load_pending_requests()`
- Currently shows "(No pending requests)" when empty
- Accept/Reject buttons show "coming soon" placeholders

**Completed (Phase 8 - Message Request Network Integration)** ✅:
- **Implemented wire protocol for message requests**:
  - Created `MessageEnvelope` enum with Request/RegularMessage variants ([network.rs:18-29](clients/windows-native/src/network.rs#L18))
  - Requests include: sender_fingerprint, sender_public_key, sender_device_id, first_message (encrypted)
  - All messages now sent as JSON envelopes over TCP
- **Request sending**:
  - "Start Chat" button now sends Request envelope ([mod.rs:247-252](clients/windows-native/src/ui/mod.rs#L247))
  - Initial message: "Hello! I'd like to chat with you." (encrypted with recipient's key)
  - Continues to chat even if request send fails
- **Request receiving**:
  - Added WM_REQUEST_RECEIVED handler ([mod.rs:98-100](clients/windows-native/src/ui/mod.rs#L98))
  - `handle_request_received()` decrypts preview, saves to disk, shows notification ([mod.rs:870-940](clients/windows-native/src/ui/mod.rs#L870))
  - Requests stored in AppData\Local\CryptoChat\requests\
- **Network protocol changes**:
  - `handle_incoming_connection()` deserializes JSON and routes to correct handler ([network.rs:114-132](clients/windows-native/src/network.rs#L114))
  - Regular messages wrapped in `MessageEnvelope::RegularMessage` ([app.rs:102-105](clients/windows-native/src/app.rs#L102))
- **UX improvements**:
  - Removed encrypted base64 preview from sender's chat history ([mod.rs:322-323](clients/windows-native/src/ui/mod.rs#L322))
  - Shows clean "[You]: message" format
- **How it works now**:
  - Alice imports Bob's key → clicks "Start Chat" → sends Request envelope to Bob's port
  - Bob receives → sees notification "New Message Request from [fingerprint]"
  - Bob opens "View Requests" → sees Alice's request with decrypted preview
  - Bob can accept/reject (handlers to be implemented)

**Completed (Phase 7 - Port Display & UX Improvements)** ✅:
- **Added visible listening port display**:
  - New "My Port" field shows listening port immediately after key generation ([onboarding.rs:23-24](clients/windows-native/src/ui/onboarding.rs#L23))
  - Port is displayed next to fingerprint for easy sharing ([onboarding.rs:102-130](clients/windows-native/src/ui/onboarding.rs#L102))
  - Format: "Listening on port XXXX" for clarity
- **Auto-start network on key generation**:
  - Network starts immediately when keys are generated ([mod.rs:583-599](clients/windows-native/src/ui/mod.rs#L583))
  - Also starts network on app launch when existing keys are loaded ([mod.rs:39-48](clients/windows-native/src/ui/mod.rs#L39))
  - No need to wait until "Start Chat" button - port is always listening
- **Fixed peer address label**:
  - Added proper ID to peer address label ([onboarding.rs:245](clients/windows-native/src/ui/onboarding.rs#L245))
  - Now properly shows/hides when importing recipient key
- **User-friendly workflow**:
  - Bob generates keys → sees "Listening on port 5432"
  - Bob tells Alice: "Connect to 127.0.0.1:5432"
  - Alice pastes Bob's public key → enters 127.0.0.1:5432 → clicks Start Chat
  - No more confusion about ports or connection details

**Completed (Phase 6 - Font Rendering Fix)** ✅:
- Fixed text rendering artifacts (horizontal striping) by adding proper font assignment:
  - Created `set_default_font()` helper in [mod.rs:23-27](clients/windows-native/src/ui/mod.rs#L23) using GetStockObject(DEFAULT_GUI_FONT)
  - Applied WM_SETFONT to all 17 onboarding controls in [onboarding.rs:24-281](clients/windows-native/src/ui/onboarding.rs#L24)
  - Applied WM_SETFONT to all 5 chat controls in [chat.rs:15-92](clients/windows-native/src/ui/chat.rs#L15)
  - Removed emoji characters from button labels (replaced with plain text for better font compatibility)
- Added window background brush:
  - Set hbrBackground to COLOR_WINDOW + 1 in WNDCLASSW ([main.rs:66](clients/windows-native/src/main.rs#L66))
  - Ensures proper background painting for the main window
- Root cause: Win32 controls don't inherit fonts automatically - must explicitly call SendMessageW(WM_SETFONT) on each control
- All text now renders cleanly without artifacts

**Completed (Phase 5 - UI Fixes & Port Validation)** ✅:
- Fixed window resize artifacts and painting issues:
  - Implemented proper WM_PAINT handler with BeginPaint/EndPaint ([mod.rs:45-50](clients/windows-native/src/ui/mod.rs#L45))
  - Added WM_SIZE handler to force redraw on resize ([mod.rs:52-56](clients/windows-native/src/ui/mod.rs#L52))
  - Added WM_GETMINMAXINFO to enforce minimum window size (900x680) ([mod.rs:57-65](clients/windows-native/src/ui/mod.rs#L57))
  - Removed WS_VSCROLL flag and reduced default window size to 920x700 ([main.rs:75-79](clients/windows-native/src/main.rs#L75))
- Fixed port parsing errors:
  - Added peer address validation (checks for ":" and valid port) ([mod.rs:174-196](clients/windows-native/src/ui/mod.rs#L174))
  - Changed default peer address from "127.0.0.1:" to "127.0.0.1:5000" ([onboarding.rs:226](clients/windows-native/src/ui/onboarding.rs#L226))
  - Shows helpful error dialogs if address format is invalid
- Improved UI layout for better usability:
  - Reorganized controls into more compact horizontal layout
  - Moved buttons side-by-side (Copy Key + QR Code, Import + Scan QR, Peer Address + Start Chat)
  - Reduced vertical spacing between controls
  - Better use of horizontal space (30px margins, controls span 840px width)
  - All controls now fit in 650px vertical space (was 950px)
  - Updated hide_onboarding_controls to hide all 12 controls ([mod.rs:609-634](clients/windows-native/src/ui/mod.rs#L609))
- Window is now fully resizable without artifacts
- Controls no longer overlap in any state

**Completed (Phase 4 - Accept/Reject Flow)** ✅:
- Implemented request selection handler:
  - Listbox sends LBN_SELCHANGE notification when selection changes
  - `handle_request_selection()` loads selected request and displays:
    - Formatted fingerprint in fingerprint field
    - First message preview (or "(No preview available)")
- Implemented Accept button logic ([mod.rs:831-893](clients/windows-native/src/ui/mod.rs#L831)):
  - Gets selected request from listbox
  - Calls `request.accept()` to mark as accepted
  - Saves updated request to `requests.json`
  - Creates `Contact` from request via `Contact::from_request()`
  - Saves contact to `contacts.json`
  - Shows success message with formatted fingerprint
  - Refreshes request list (accepted requests are filtered out)
  - Clears detail fields
- Implemented Reject button logic ([mod.rs:894-955](clients/windows-native/src/ui/mod.rs#L894)):
  - Gets selected request from listbox
  - Shows confirmation dialog: "Are you sure? This cannot be undone"
  - Deletes request via `delete_request()` (removes from `requests.json`)
  - Shows confirmation: "Request rejected and deleted"
  - Refreshes request list
  - Clears detail fields
- Error handling for all storage operations
- User feedback for all actions (success/error dialogs)

### 2025-11-04: Native Windows Client MVP Complete ✅
**Status**: MVP functional - end-to-end encrypted P2P messaging with persistent keys
**Implementation**:
- `clients/windows-native/` - Pure Rust + windows-rs 0.58 (~1.3MB binary vs 80MB Tauri)
- Onboarding: PGP Cv25519 key generation (30-60s), fingerprint display, QR code exchange
- Chat UI: Message history, input field, Send button, connection status display
- Crypto: Sequoia PGP encrypt_and_sign/decrypt_and_verify with base64 encoding (10 tests passing)
- Networking: TCP P2P with length-prefix protocol, background listener, WM_MESSAGE_RECEIVED handler
- Threading: Background key generation with PostMessageW, thread-local AppState via RefCell
- Keystore: Windows Credential Manager integration with DPAPI encryption

**Working Features**:
- Generate Cv25519 keypair and display fingerprint
- **✨ NEW**: Persistent key storage with Windows Credential Manager + DPAPI
- **✨ NEW**: Automatic keystore loading on app startup
- **✨ NEW**: QR code generation with cryptographic signatures (prevent spoofing)
- **✨ NEW**: QR code scanning with native file picker dialog (PNG/JPEG support)
- **✨ NEW**: Automatic signature verification when scanning QR codes
- **✨ NEW**: 5-minute timestamp expiry for QR codes
- Copy public key to clipboard
- Import recipient's public key with validation
- Enter peer address (127.0.0.1:PORT)
- Start network listener on random port
- Send encrypted messages over TCP
- Receive and decrypt messages from peer
- Display "[You]:" and "[Recipient]:" in chat history

**Security Improvements**:
- QR code payloads cryptographically signed to prevent forgery/tampering
- Fingerprint verification on all key imports
- Keys encrypted with Windows DPAPI (hardware-bound when TPM available)
- Automatic signature verification before importing recipient keys

**Current Limitations**:
- TCP localhost only (no LAN/WAN yet)
- Window resize artifacts
- No message history persistence

**Next**:
- Upgrade TCP → libp2p overlay with Kademlia DHT
- Add message history persistence (SQLite)
- Android Jetpack Compose client

### 001-secure-messaging (2025-10-21 - In Progress)
**Status**: MVP implementation in progress
**Added**:
- Core project structure with shared Rust workspace (`crypto-core`, `messaging`, `node`)
- libp2p overlay runtime with Kademlia DHT, request-response protocol, QUIC/TCP transport
- Node service with health/echo API endpoints and storage layer
- Android JNI bindings stub for Rust core integration
- Spec Kit workflow with constitution, spec, plan, and task artifacts
- Sequoia PGP integration with 10 passing crypto tests
- Message pipeline with 4 passing tests
- Protocol contract tests (8 passing)

## Resources

- **Constitution**: `.specify/memory/constitution.md` - Governing principles (v1.1.0)
- **Current Spec**: `specs/001-secure-messaging/spec.md` - User scenarios and requirements
- **Implementation Plan**: `specs/001-secure-messaging/plan.md` - Technical approach
- **Tasks**: `specs/001-secure-messaging/tasks.md` - Work breakdown
- **libp2p Docs**: https://docs.libp2p.io/
- **Sequoia PGP**: https://sequoia-pgp.org/
- **windows-rs**: https://microsoft.github.io/windows-docs-rs/
- **Win32 API**: https://learn.microsoft.com/en-us/windows/win32/

<!-- MANUAL ADDITIONS START -->
- Security baseline: Overlay sessions use Noise + OpenPGP identities
- Release cadence: Coupled Windows MSI + Android AAB signed builds
- Architecture pivot rationale: User feedback on Tauri debugging difficulties and security model misalignment with constitution principles
<!-- MANUAL ADDITIONS END -->
