# M2M — Security Hardening Checklist

> **Last Updated**: 2026-05-28

## Cryptography
- [x] Use only proven libraries (libsodium via sodiumoxide)
- [x] No custom crypto constructions
- [x] Ed25519 for signatures
- [x] X25519 for key exchange
- [x] XChaCha20-Poly1305 for AEAD
- [x] HKDF-SHA256 for key derivation
- [x] Argon2id for passphrase-based key derivation
- [x] Zeroize all sensitive memory on drop
- [ ] Verify no secret material in core dumps

## Network
- [x] Length-prefixed framing with size limits
- [x] Timeout on all network operations
- [ ] Rate limiting on incoming messages
- [x] Heartbeat with bounded retry
- [x] Graceful disconnect protocol
- [x] No plaintext ever on the wire after handshake

## Protocol
- [x] Version byte on every packet
- [x] Strict packet type validation
- [x] Reject unknown packet types
- [x] No silent fallback to insecure behavior
- [x] Replay protection via monotonic counters
- [x] Message sequencing validation

## Authentication
- [x] Signed handshake messages
- [x] Fingerprint display for out-of-band verification
- [x] Invite signature verification before any processing
- [x] Invite expiry enforcement
- [x] One-time invite consumption tracking

## Storage
- [x] Keys encrypted at rest (Argon2id + SQLCipher)
- [x] Messages encrypted at rest (separate key)
- [x] Attachments encrypted individually
- [x] Secure deletion with VACUUM
- [x] Optional message history disable

## UI / UX
- [x] No auto-open of files
- [x] No auto-render of untrusted content in main process
- [x] Clear security state indicators
- [x] Sanitized chat display (no XSS)
- [x] Fingerprint comparison flow

## Logging
- [x] No keys in logs
- [x] No plaintext in logs
- [x] No invite contents in logs
- [x] No IPs in logs (where avoidable)
- [x] Redaction layer active

## Build & Supply Chain
- [x] Minimal dependencies
- [ ] No telemetry or analytics
- [ ] Reproducible builds
- [x] Dependency audit
- [x] CSP headers in Tauri webview
