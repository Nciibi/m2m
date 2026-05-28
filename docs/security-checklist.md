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
- [ ] Zeroize all sensitive memory on drop
- [ ] Verify no secret material in core dumps

## Network
- [ ] Length-prefixed framing with size limits
- [ ] Timeout on all network operations
- [ ] Rate limiting on incoming messages
- [ ] Heartbeat with bounded retry
- [ ] Graceful disconnect protocol
- [ ] No plaintext ever on the wire after handshake

## Protocol
- [ ] Version byte on every packet
- [ ] Strict packet type validation
- [ ] Reject unknown packet types
- [ ] No silent fallback to insecure behavior
- [ ] Replay protection via monotonic counters
- [ ] Message sequencing validation

## Authentication
- [ ] Signed handshake messages
- [ ] Fingerprint display for out-of-band verification
- [ ] Invite signature verification before any processing
- [ ] Invite expiry enforcement
- [ ] One-time invite consumption tracking

## Storage
- [ ] Keys encrypted at rest (Argon2id + SQLCipher)
- [ ] Messages encrypted at rest (separate key)
- [ ] Attachments encrypted individually
- [ ] Secure deletion with VACUUM
- [ ] Optional message history disable

## UI / UX
- [ ] No auto-open of files
- [ ] No auto-render of untrusted content in main process
- [ ] Clear security state indicators
- [ ] Sanitized chat display (no XSS)
- [ ] Fingerprint comparison flow

## Logging
- [ ] No keys in logs
- [ ] No plaintext in logs
- [ ] No invite contents in logs
- [ ] No IPs in logs (where avoidable)
- [ ] Redaction layer active

## Build & Supply Chain
- [ ] Minimal dependencies
- [ ] No telemetry or analytics
- [ ] Reproducible builds
- [ ] Dependency audit
- [ ] CSP headers in Tauri webview
