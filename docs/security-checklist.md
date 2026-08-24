# M2M — Security Checklist

> **Canonical, per-item status lives in [SECURITY-HARDENING.md](SECURITY-HARDENING.md).**
> This page is the quick-scan summary. Last updated: v4.0.0.

## Cryptography
- [x] Pure-Rust RustCrypto stack (`ed25519-dalek`, `x25519-dalek`, `chacha20poly1305`) — no C FFI, actively maintained
- [x] No custom crypto constructions; wire formats pinned by libsodium golden vectors + RFC 8032/7748 tests
- [x] Ed25519 signatures — X25519 key exchange — XChaCha20-Poly1305 AEAD — HKDF-SHA256 — Argon2id
- [x] Zeroize on drop for all key material

## Data at rest
- [x] Passphrase-derived storage keys (Argon2id) locked via mlock/VirtualLock
- [x] Crypto-shredding deletion (per-message wrapped CEKs)
- [x] Crash dumps disabled at process start
- [x] Encrypted metadata: reactions, family contacts, transfer filenames

## Network
- [x] X3DH + Double Ratchet with transactional receive (H3)
- [x] Handshake signatures cover candidates; encrypted heartbeats with ack-timeout liveness
- [x] Contact allowlist gate; connection rate limiting; frame size caps
- [x] Air-gap mode blocks all internet-facing operations on demand

## Operations
- [x] Fuzzing targets + deterministic CI regressions for all parsers
- [x] Reproducible-build scaffolding (pinned toolchain, locked deps)
- [x] Signed-update wiring (inert until maintainer pubkey/endpoint)
- [ ] External crypto audit ? the remaining gap