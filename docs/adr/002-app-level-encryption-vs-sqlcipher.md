# ADR 002: Application-Level Encryption Instead of SQLCipher

**Status**: Accepted  
**Date**: 2026-06-28  

## Context

M2M needs encrypted local storage for identity keys and message history. Options: SQLCipher (transparent full-database encryption via OpenSSL) or application-level encryption (encrypt individual fields with XChaCha20-Poly1305 before writing to plain SQLite).

## Decision

Use **plain SQLite with application-level encryption** via XChaCha20-Poly1305.

## Rationale

- **No OpenSSL dependency**: SQLCipher links against OpenSSL, adding a large, complex C dependency. M2M is already linked against libsodium, which provides XChaCha20-Poly1305.
- **Selective encryption**: Only sensitive fields (private keys, message contents) are encrypted. Metadata (timestamps, message counts) remains indexable by SQLite.
- **Two-tier key separation**: Identity keys are encrypted with a passphrase-derived Argon2id key. Messages are encrypted with a storage key locked in RAM via mlock/VirtualLock.
- **Domain separation via AAD**: Each storage domain (keys.db vs messages.db) uses a distinct AAD context, preventing ciphertext substitution.

## Consequences

- SQLite internals never see plaintext key material — only encrypted blobs.
- WAL mode still provides good read concurrency.
- Export files use the same encryption scheme but with a separate AAD context.
