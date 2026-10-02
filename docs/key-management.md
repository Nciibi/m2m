# M2M — Key Management Design

> **Version**: 0.1.0 | **Status**: Draft | **Last Updated**: 2026-05-28

## 1. Key Hierarchy

```
Identity Layer (permanent)
├── Ed25519 Signing Keypair (identity)
│   ├── Signs invites
│   ├── Signs handshake ephemeral keys
│   └── Generates fingerprint for verification
│
Session Layer (ephemeral, per-connection)
├── X25519 Ephemeral Keypair
│   └── Used in DH key exchange, then discarded
├── Session Key (derived via HKDF)
│   └── Used for XChaCha20-Poly1305 encryption
└── Nonce Counter (per session)
```

## 2. Identity Keypair

- **Algorithm**: Ed25519 (via `ed25519-dalek`)
- **Generated**: On first app launch, never regenerated unless user explicitly resets
- **Storage**: Private key encrypted at rest in key store (separate from message DB)
- **Fingerprint**: SHA-256 of public key, displayed as hex groups (e.g., `A1B2:C3D4:...`)
- **Export**: Public key only, embedded in invite links

## 3. Session Keys

- **Key Exchange**: X3DH over X25519 (`x25519-dalek`). DH1 = IK, DH2 = SPK,
  DH3 = SPK↔OPK, DH4 = IK↔X25519 identity.
  *An earlier revision of this document named the libsodium functions
  `crypto_box_keypair` and `crypto_scalarmult`. There has been no libsodium in
  this crate since the RustCrypto migration.*
- **Derivation**: `HKDF-SHA256(salt=&[0u8; 32], ikm=X3DH output, info="m2m-kx-v1", len=64)`
  — the real label is `m2m-kx-v1`, not `m2m-v1-session`, and the salt is a
  fixed 32 zero bytes rather than sorted public keys.
  Chain and DH-ratchet derivations use `M2M-MSG-KEY` and `M2M-DH-RATCHET`
  respectively (`crypto.rs:837`, `:873`).
- **Ratchet**: Signal Double Ratchet. The symmetric chain advances per message;
  a fresh DH step every `ratchet_interval` messages (default 100).
- **Lifetime**: Single TCP session, max 24 hours
  (`MAX_SESSION_DURATION_SECS`, `protocol.rs:95`)
- **Rotation**: New session key on reconnect. **There is no 1-hour mid-session
  rotation** — an earlier revision of this document claimed one and it does not
  exist. The only scheduled rekey is the 24-hour session ceiling, plus the
  per-message chain step.
- **Zeroization**: Session keys zeroized immediately on disconnect or expiry

## 4. Nonce Management

- 24-byte nonces for XChaCha20-Poly1305
- Constructed: `random_prefix (16B) || counter (8B)`
- Counter is monotonically increasing, tracked per peer per session
- Received counters below the high-water mark are rejected (replay protection)

## 5. Storage Encryption

- Key store DB key derived from user passphrase via Argon2id
- Message DB uses a separate random key, itself stored in the key store
- **There is no SQLCipher.** This document previously said "Both DBs are
  SQLCipher (AES-256-CBC with HMAC-SHA256)". SQLCipher was evaluated and
  rejected — see `docs/adr/002-app-level-encryption-vs-sqlcipher.md`. What
  actually happens: plain `rusqlite` (bundled SQLite) with **application-level**
  XChaCha20-Poly1305 envelopes. Each row's content is sealed individually with
  its own 24-byte nonce, and the *content key* for that row is itself wrapped
  under a KEK derived from the passphrase.
- There are **three** databases, not two: `keys.db`, `messages.db`, and
  `transfers.db`.

## 6. Key Lifecycle

| Event | Action |
|-------|--------|
| First launch | Generate Ed25519 identity keypair |
| Create invite | Sign invite with identity key |
| Accept connection | Generate ephemeral X25519 keypair |
| Handshake complete | Derive session key, zeroize ephemeral private key |
| Session timeout (1hr) | Rotate session key via new DH exchange |
| Disconnect | Zeroize session key |
| Session expiry (24hr) | Force disconnect + zeroize |
| App shutdown | Zeroize all in-memory keys |
| User reset | Delete key store, regenerate identity |
