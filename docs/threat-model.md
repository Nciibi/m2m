# M2M — Threat Model

> **Version**: 1.2.0  
> **Status**: Current  
> **Last Updated**: 2026-06-28

## 1. Threat Actors

### 1.1 Passive Network Observer
- **Capability**: Can observe all traffic on the network segment.
- **Goal**: Read message contents, identify communication partners, build metadata profiles.
- **Mitigation**: All traffic encrypted with authenticated encryption. No plaintext on the wire. Variable padding hides message lengths. Minimal metadata in headers.

### 1.2 Active Network Attacker (MITM)
- **Capability**: Can intercept, modify, inject, replay, and drop packets.
- **Goal**: Impersonate a peer, tamper with messages, inject malicious content.
- **Mitigation**:
  - X3DH key agreement binds session keys to Ed25519 + X25519 identity keys.
  - Double Ratchet provides self-healing forward secrecy after compromise.
  - Fingerprint verification via out-of-band 4x4 grid modal.
  - Replay protection via monotonic counters + AEAD AAD binding.
  - Per-message forward secrecy via chain key ratchet.
  - Protocol version validation with reserved version rejection.

### 1.3 Malicious Peer
- **Capability**: Controls one end of the connection. Can send arbitrary data.
- **Goal**: Exploit parsing bugs, cause denial of service, exfiltrate data.
- **Mitigation**:
  - Strict input validation on all received data.
  - Frame size limits (16 MiB max), message size limits (64 KiB).
  - DashMap-based per-IP rate limiting.
  - Slowloris protection via per-byte read timeouts (1s).
  - Streaming file transfers to temp file (no RAM buffering).
  - Chunk hash verification on file transfers.
  - Filename sanitization (`[a-zA-Z0-9._-]` only, no path traversal).

### 1.4 Physical/OS Attacker
- **Capability**: Has local or kernel access to the device. Can read process memory, swap, disk.
- **Goal**: Extract encryption keys, read message history.
- **Mitigation**:
  - Storage encryption key **mlock()'d** (VirtualLock on Windows) — cannot be paged to swap.
  - Memory zeroization (`Zeroize` trait) on all session keys, message bodies, and sensitive structs.
  - At-rest encryption via XChaCha20-Poly1305 with Argon2id-derived key.
  - Encrypted databases via application-level AEAD (no plaintext SQLite).
  - Private key never in plain memory outside of short unlock window.

### 1.5 Network-Level Attacker (DoS)
- **Capability**: Can flood connection with spurious data or half-open connections.
- **Goal**: Deplete server resources, prevent legitimate connections.
- **Mitigation**:
  - DashMap-based per-IP connection rate limiter.
  - Tokio timeouts on all I/O operations.
  - Per-byte read timeouts (Slowloris defense).
  - Frame size validation before allocation.
  - Connection count tracking.

## 2. Assets

| Asset | Protected By | Impact if Compromised |
|-------|-------------|----------------------|
| Long-term Ed25519 identity key | Argon2id-encrypted vault, mlocked key | Identity theft, impersonation |
| X25519 DH identity key | Argon2id-encrypted vault, mlocked key | Forward secrecy failure |
| Session keys | mlocked memory, zeroized on disconnect | One session's messages readable |
| Message history | XChaCha20-Poly1305 at-rest encryption | Historical message access |
| Peer fingerprints | Out-of-band verification process | MITM undetected |

## 3. Key Assumptions

1. **Machine is trusted**: If the OS is compromised, mlock provides defense-in-depth but can be bypassed by a kernel attacker.
2. **TLS is not used**: M2M avoids TLS in favor of application-layer encryption for metadata minimization.
3. **TCP ordering**: The Double Ratchet implementation assumes ordered delivery (skipped message keys are derived sequentially, not stored).
4. **Relay server is operated by the user**: The TURN relay protocol is not authenticated — anyone can connect. Only use trusted relays.

## 4. Cryptographic Guarantees

| Property | Implementation |
|----------|---------------|
| Forward secrecy (legacy) | SHA-256 KDF ratchet after each message |
| Forward secrecy (X3DH) | Double Ratchet: message key = HKDF(chain_key, "M2M-MSG-KEY") |
| Post-compromise security | DH ratchet every 100 messages |
| Authentication | Ed25519 signatures on handshake + X25519 DH |
| Replay protection | Monotonic counters + AEAD AAD binding |
| Key agreement | X3DH: 3 DH ops (4 with OPK) → HKDF → root + chain key |
| Side-channel resistance | Constant-time libsodium primitives |

## 5. Network Attack Mitigations

| Attack | Mitigation |
|--------|-----------|
| Eavesdropping | XChaCha20-Poly1305 AEAD encryption |
| Tampering | AEAD authentication tag verification |
| Replay | Monotonic u64 counter + AAD binding |
| Traffic analysis | Variable exponential padding (1KB–16KB tiers) |
| Slowloris | Per-byte 1s read timeout |
| DoS (connection flood) | DashMap per-IP rate limiter |
| DNS poisoning | Cross-server STUN consistency check |
| MITM (first use) | Fingerprint comparison modal |
| SYM flooding | tokio Accept with backpressure |
| DoS (large frames) | Frame size validation (16 MiB cap) |
| DoS (file transfer) | Streaming to temp file, chunk hash verification |
| Reaction injection | Max 10-char reaction string, validated emoji, stored via upsert |
| Edit injection (replay) | Edited_at timestamp prevents replay; old content replaced<br>Edit only allowed for messages in the current session |
| Edit injection | Only replaces content for known message_id. Old content zeroized. |
| Delete injection | Only marks deleted=1. Original ciphertext stays in DB. |
| Self-destruct bypass | Timer is a client-side UX hint. Expired messages pruned by both storage query filter and periodic cleanup. |

## 6. Data Flow Security

```
Invite Creation:
  Ed25519 sign(invite_payload) → encoded as base64url → m2m:// prefix
  X3DH: SignedPrekey = X25519 ephemeral keypair → Ed25519 sig → embedded in payload

Handshake:
  X3DHInit: DH(IK_A, SPK_B) || DH(EK_A, IK_B) || DH(EK_A, SPK_B) → HKDF → root_key
  Double Ratchet: root_key → DH ratchet → chain keys → message keys

Message Encryption:
  padded_msg = pad_message_variable(plaintext)  ← variable padding
  nonce = XChaCha20-Poly1305 random nonce
  aad = packet_type || counter
  ciphertext = AEAD_encrypt(padded_msg, aad, nonce, msg_key)
  envelope = { nonce, counter, ciphertext, dr_header }

Storage:
  storage_key = Argon2id(passphrase, salt=pub_key)
  encrypted = XChaCha20-Poly1305(plaintext, key=storage_key)
  storage_key → StorageKey::new() → mlock()'d
```
