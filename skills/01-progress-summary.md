# Progress Summary: M2M Secure Messenger

**Status**: Backend core implemented. Ready for file transfer logic and frontend UI.
**Last Updated**: 2026-05-28

## Core Architecture Completed
The foundation of the M2M secure messenger has been established using Rust and Tauri v2. The project strictly adheres to the requested threat model, ensuring no central servers, no accounts, and no metadata leakage.

## Backend Modules Built (`src-tauri/src/`)

1. **Crypto Module (`crypto.rs`)**
   - Implemented using audited `sodiumoxide` primitives.
   - Long-term identity keys use **Ed25519**.
   - Ephemeral session keys use **X25519** Diffie-Hellman and **HKDF-SHA256**.
   - Authenticated encryption is done via **XChaCha20-Poly1305**.
   - All sensitive keys in memory use `zeroize` on drop.
   - SHA-256 fingerprint generation works.

2. **Protocol Module (`protocol.rs`)**
   - Strict versioning (`PROTOCOL_VERSION: u8 = 0x01`).
   - Length-prefixed framing format implemented.
   - Strict size limits defined for handshakes, text messages, and file chunks.
   - Fully typed MessagePack structures for all wire data (Handshake, EncryptedEnvelope, MessageBody, Error, etc.).

3. **Network Module (`network.rs`)**
   - TCP listener and connector built using `tokio::net`.
   - Timeout wrappers on all network operations to prevent DoS.
   - Connection state machine transitions (Disconnected -> Connecting -> Handshaking -> Established).
   - Functions for reading/writing fully framed bytes safely.

4. **Identity & Invite Module (`identity.rs`)**
   - Tamper-evident, signed invite links (`m2m://...` base64url encoded).
   - 8-step invite validation (version check, expiry check, clock skew tolerance, Ed25519 signature verification).

5. **Session Module (`session.rs`)**
   - Full asymmetric handshake implementation for both Initiator and Responder.
   - Replay protection mechanism enforcing monotonically increasing packet counters.
   - Transparent encryption and decryption of payload data.
   - Time-bound session expirations.

6. **Storage Module (`storage.rs`)**
   - Implemented SQLite (`rusqlite`) storage with **application-level encryption**.
   - Bypassed OpenSSL requirements of SQLCipher by encrypting private keys and chat histories with `XChaCha20-Poly1305` before writing them to the database.
   - Implemented secure deletion pragmas (`PRAGMA secure_delete=ON`) and `VACUUM` capability.
   - Separated key store (`keys.db`) and message store (`messages.db`).

7. **Tauri Commands Bridge (`commands.rs` & `state.rs`)**
   - Thread-safe `AppState` wrapping the identity, TCP listeners, and active peer connections.
   - Clean, safe Tauri commands that return strictly typed structs to the frontend.
   - *Security note: No secret keys ever cross the Tauri IPC boundary to the React frontend.*

## Current State of the Code
- The Rust code compiles successfully (`cargo check` passes).
- The npm dependencies (`react`, `vite`, `@tauri-apps/api`) are installed.
- The `tauri.conf.json` is hardened with strict CSPs and window sizing.
