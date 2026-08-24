# v4.0.0 — Security Hardening Release

The security-hardening milestone. Every item from the
[security hardening roadmap](docs/SECURITY-HARDENING.md) is implemented,
with the remaining gaps honestly documented rather than claimed.

## Highlights

### Cryptography
- **Migrated from archived libsodium/sodiumoxide to pure-Rust RustCrypto**
  (`ed25519-dalek` 2, `x25519-dalek` 2, `chacha20poly1305` 0.10) — no C FFI,
  actively maintained. Byte-compatibility with previous formats is proven by
  golden vectors captured from libsodium plus RFC 8032/7748 known-answer tests.

### Critical fixes
- Remote DoS: frame parser panicked on frames declaring `< 2` payload bytes
  (found by fuzzing on day one)
- Double Ratchet: transactional receive — a forged frame can no longer
  permanently desync a session (H3)
- Deterministic zero-start counters; removed broken legacy session fallback (H4)
- One-time prekeys actually used in X3DH (H6)
- Optional contact allowlist gate for incoming connections (H5)
- Imported identities re-sealed under passphrase-derived keys, never the
  publicly computable legacy key (H1)

### New security features
- **Encrypted heartbeats** with ack-timeout liveness detection
- **Duress passphrase** — entering it at unlock silently wipes everything
- **Air-gap mode** — LAN-only operation, blocks STUN/UPnP/relay/Tor/discovery
- **Ephemeral mode** — conversations never touch SQLite
- **Panic hotkey** (Ctrl+Alt+Shift+W, armable) — instant wipe + exit
- Send batching jitter + typing-indicator cover traffic
- Crypto-shredding deletion for messages, edits, conversations

### Anti-surveillance suite
- macOS capture protection (`NSWindowSharingNone`) via libobjc FFI;
  honest per-platform capability reporting (X11 = unsupported, not faked)
- Capture-protection persistence across restarts/webview recreation
- On-screen randomized keyboard for passphrase entry
- Blur-on-focus-loss overlay
- Capture-software detection (OBS, Snipping Tool, …) with warning banner

### Infrastructure
- Fuzzing crate targeting all internet-facing parsers + deterministic CI regressions
- Reproducible-build scaffolding (pinned toolchain, locked deps, hash publication)
- Signed-update channel wired (inert until maintainer pubkey/endpoint — see docs/SIGNED-UPDATES.md)
- mlock/VirtualLock on long-term secrets; crash dumps disabled at startup
- Metadata-at-rest encryption: reactions, family contacts, transfer filenames
- Hardened VM image (`vm/`) and Tails/Whonix/Qubes operational guide

## Verification
- 323 backend tests · 123 frontend tests · clean clippy · production build green
- Golden vectors prove byte-identical crypto output across the library migration

## Notes
- Legacy pre-X3DH sessions do not interop across this version boundary
  (intentional; no installed base).
- Update channel requires maintainer key setup before first use.
