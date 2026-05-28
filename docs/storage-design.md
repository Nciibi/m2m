# M2M — Local Storage Encryption Design

> **Version**: 0.1.0 | **Status**: Draft | **Last Updated**: 2026-05-28

## 1. Storage Architecture

Two separate encrypted SQLite databases:

```
~/.m2m/
├── keys.db      (SQLCipher — identity keys, peer keys, trust state)
├── messages.db  (SQLCipher — chat history, optional)
├── attachments/  (encrypted files, each with unique key)
└── config.toml  (non-sensitive settings only)
```

## 2. Key Store (`keys.db`)

Encrypted with a key derived from user passphrase:
`db_key = Argon2id(passphrase, salt, ops=3, mem=256MB, len=32)`

### Tables

**identity**: `id, public_key, encrypted_private_key, created_at`  
**peers**: `id, public_key, fingerprint, alias, verified, first_seen, last_seen`  
**consumed_invites**: `nonce, consumed_at`

## 3. Message Store (`messages.db`)

Encrypted with a random 32-byte key stored in `keys.db`.
Can be disabled entirely (no message persistence).

### Tables

**conversations**: `id, peer_id, created_at, last_message_at`  
**messages**: `id, conversation_id, direction, content_encrypted, timestamp, delivered`

## 4. Attachment Storage

Each attachment encrypted individually:
`attachment_key = random 32 bytes` (stored in messages.db alongside message)
`encrypted_file = XChaCha20-Poly1305(file_bytes, attachment_key, nonce)`

## 5. Secure Deletion

- Delete session: drop conversation + messages + attachment files + zeroize keys
- Delete all data: drop both DBs + attachment dir + regenerate identity
- SQLCipher VACUUM after deletions to reclaim and overwrite freed pages
