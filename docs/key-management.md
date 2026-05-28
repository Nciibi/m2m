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

- **Algorithm**: Ed25519 (via libsodium `crypto_sign_keypair`)
- **Generated**: On first app launch, never regenerated unless user explicitly resets
- **Storage**: Private key encrypted at rest in key store (separate from message DB)
- **Fingerprint**: SHA-256 of public key, displayed as hex groups (e.g., `A1B2:C3D4:...`)
- **Export**: Public key only, embedded in invite links

## 3. Session Keys

- **Key Exchange**: X25519 (`crypto_box_keypair` for ephemeral, `crypto_scalarmult`)
- **Derivation**: `HKDF-SHA256(ikm=shared_secret, salt=sorted_pubkeys, info="m2m-v1-session")`
- **Lifetime**: Single TCP session, max 24 hours
- **Rotation**: New session key on reconnect; mid-session rotation every 1 hour
- **Zeroization**: Session keys zeroized immediately on disconnect or expiry

## 4. Nonce Management

- 24-byte nonces for XChaCha20-Poly1305
- Constructed: `random_prefix (16B) || counter (8B)`
- Counter is monotonically increasing, tracked per peer per session
- Received counters below the high-water mark are rejected (replay protection)

## 5. Storage Encryption

- Key store DB key derived from user passphrase via Argon2id
- Message DB uses a separate random key, itself stored in the key store
- Both DBs are SQLCipher (AES-256-CBC with HMAC-SHA256)

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
