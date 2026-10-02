# M2M — Local Storage Encryption Design

> **Version**: 0.1.0 | **Status**: Draft | **Last Updated**: 2026-05-28

## 1. Storage Architecture

Plain SQLite with application-level AEAD envelopes. There is **no SQLCipher** — see
`docs/adr/002-app-level-encryption-vs-sqlcipher.md`, which rejected it in favour
of encrypting each row's content with XChaCha20-Poly1305 under a wrapped
content key. Three databases, not two.

```
~/.m2m/
├── keys.db      (identity keys, peer keys, trust state)
├── messages.db  (chat history, optional)
├── transfers.db (file-transfer state)
└── security.json (non-sensitive settings only — there is no config.toml,
                   and there is no attachments/ directory: transfer payloads
                   stream to a temp file)
```

## 2. Key Store (`keys.db`)

Encrypted with a key derived from user passphrase:
`db_key = Argon2id(passphrase, salt=identity_pub, ops=3, mem=64MiB, len=32)`

**The 256 MB figure in earlier revisions of this document was a 4x
overstatement.** `commands/util.rs` uses `m_cost = 65536` KiB. Overstating the
KDF cost in documentation is not harmless: it tells a reviewer the passphrase is
harder to attack than it is.

### Tables

**identity**: `id, public_key, encrypted_private_key, created_at`  
**peers**: `id, public_key, fingerprint, alias, verified, first_seen, last_seen`  
**consumed_invites**: `nonce, consumed_at`
**vault_meta**, **accounts**, **family** — three tables this document omitted

## 3. Message Store (`messages.db`)

Encrypted with a random 32-byte key stored in `keys.db`.
Can be disabled entirely (no message persistence).

### Tables

**conversations**: `id, peer_id, created_at, last_message_at`  
**messages**: `id, conversation_id, direction, content_encrypted, timestamp, delivered`
**reactions**, **groups**, **group_members**, **group_messages**,
**storage_stats** — five more this document omitted

## 4. File Transfer Storage

Transfers **stream to a temp file**; they are never buffered whole in memory and
never held in an `attachments/` table. Each 256 KiB chunk is hashed (SHA-256),
sent encrypted under the session key, and acknowledged individually, so a
transfer is resumable and an interrupted one leaves at most one chunk of
uncommitted data on disk. `transfers.db` holds the transfer *state* — offsets,
chunk bitmask, totals — not the bytes.

## 5. Secure Deletion

- Delete session: drop conversation + messages + attachment files + zeroize keys
- Delete all data: drop both DBs + attachment dir + regenerate identity
- Plain `VACUUM`, with `PRAGMA secure_delete = ON` and WAL truncation so freed
  pages are overwritten rather than merely released
