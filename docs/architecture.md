# M2M — Architecture Document

> **Version**: 0.1.0  
> **Status**: Draft  
> **Last Updated**: 2026-05-28

## 1. Overview

M2M (Machine-to-Machine, Mouth-to-Mouth) is a peer-to-peer encrypted desktop messenger
designed for journalists, whistleblowers, and high-risk users. It prioritizes:

- **No central server** in the message path.
- **No accounts** — identity is a cryptographic keypair.
- **No metadata leakage** — minimal protocol fields, no telemetry.
- **Auditability** — open source, reproducible builds, small codebase.

## 2. System Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Desktop App (Tauri)               │
│  ┌───────────────────────────────────────────────┐  │
│  │              React UI (WebView)                │  │
│  │  ┌─────────┐ ┌─────────┐ ┌────────────────┐  │  │
│  │  │ Invite  │ │  Chat   │ │  File Transfer │  │  │
│  │  │  Flow   │ │  View   │ │     View       │  │  │
│  │  └────┬────┘ └────┬────┘ └───────┬────────┘  │  │
│  └───────┼───────────┼──────────────┼────────────┘  │
│          │     Tauri IPC Bridge      │               │
│  ┌───────┴───────────┴──────────────┴────────────┐  │
│  │              Rust Backend Core                 │  │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────────┐ │  │
│  │  │  Crypto  │ │ Network  │ │   Storage     │ │  │
│  │  │  Module  │ │  Module  │ │   Module      │ │  │
│  │  └──────────┘ └──────────┘ └───────────────┘ │  │
│  │  ┌──────────┐ ┌──────────┐ ┌───────────────┐ │  │
│  │  │ Protocol │ │ Identity │ │   Session     │ │  │
│  │  │  Module  │ │  Module  │ │   Module      │ │  │
│  │  └──────────┘ └──────────┘ └───────────────┘ │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
            │                           │
            │    Direct TCP (encrypted) │
            ▼                           ▼
┌─────────────────────────────────────────────────────┐
│                  Remote Peer (same architecture)     │
└─────────────────────────────────────────────────────┘
```

## 3. Module Responsibilities

### 3.1 Crypto Module (`/src-tauri/src/crypto/`)
- Long-term identity keypair generation (Ed25519)
- Session key derivation (X25519 + HKDF)
- Authenticated encryption (XChaCha20-Poly1305)
- Signature creation and verification
- Secure memory zeroization

### 3.2 Network Module (`/src-tauri/src/network/`)
- TCP listener and connector
- Length-prefixed framing
- Connection state machine
- Timeouts, heartbeats, rate limiting
- Graceful disconnect handling

### 3.3 Protocol Module (`/src-tauri/src/protocol/`)
- Versioned packet format
- Strict schema validation
- Message type registry
- Handshake protocol
- Invite format encoding/decoding

### 3.4 Identity Module (`/src-tauri/src/identity/`)
- Long-term keypair management
- Fingerprint generation and display
- Invite creation and validation
- Contact trust model

### 3.5 Session Module (`/src-tauri/src/session/`)
- Session state machine
- Replay protection (nonce tracking)
- Message sequencing
- Key rotation
- Session expiry

### 3.6 Storage Module (`/src-tauri/src/storage/`)
- Encrypted SQLite (sqlcipher)
- Separate key store and message store
- Secure deletion
- Optional history disable

### 3.7 UI Layer (`/src/`)
- React-based minimal interface
- Invite generation/joining flow
- Chat view with security indicators
- File transfer controls
- Fingerprint verification display

## 4. Data Flow

### 4.1 Invite Exchange
```
Alice                                    Bob
  │                                       │
  ├─── Generate invite ──────────────────►│
  │    (signed, contains pubkey,          │
  │     address hint, expiry)             │
  │                                       │
  │◄── Validate invite ──────────────────┤
  │    (check signature, expiry)          │
  │                                       │
  ├─── TCP connect ──────────────────────►│
  │                                       │
  ├─── Handshake (X25519 DH) ───────────►│
  │◄── Handshake response ──────────────┤│
  │                                       │
  ├─── Encrypted messages ◄─────────────►│
```

### 4.2 Message Flow
1. Plaintext → serialize → encrypt (XChaCha20-Poly1305) → frame → TCP send
2. TCP recv → unframe → decrypt → validate sequence → deserialize → display

## 5. Security Boundaries

| Boundary | Trust Level |
|----------|-------------|
| User ↔ Local App | Trusted (user's own machine) |
| App ↔ OS/Filesystem | Partially trusted (encrypted storage) |
| App ↔ Network | Untrusted (all data encrypted) |
| App ↔ Peer | Untrusted until verified |
| App ↔ Attachments | Hostile (sandboxed handling) |

## 6. Technology Choices

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Language | Rust | Memory safety, no GC, strong type system |
| Desktop Shell | Tauri v2 | Smaller than Electron, Rust backend |
| UI | React + TypeScript | Well-known, auditable frontend |
| Signing | Ed25519 (libsodium) | Proven, fast, compact signatures |
| Key Exchange | X25519 (libsodium) | Standard ECDH on Curve25519 |
| AEAD | XChaCha20-Poly1305 | Extended nonce, misuse-resistant |
| KDF | HKDF-SHA256 | Standard key derivation |
| Storage | SQLCipher | AES-256 encrypted SQLite |
| Serialization | MessagePack | Compact, well-specified, no ambiguity |

## 7. Non-Goals

- Mobile support (future consideration only)
- Group chat (out of scope for MVP)
- Voice/video calls
- Cloud backup
- Federation with other systems
- Plugin system
