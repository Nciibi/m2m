# M2M — Threat Model

> **Version**: 0.1.0  
> **Status**: Draft  
> **Last Updated**: 2026-05-28

## 1. Threat Actors

### 1.1 Passive Network Observer
- **Capability**: Can observe all traffic on the network segment.
- **Goal**: Read message contents, identify communication partners, build metadata profiles.
- **Mitigation**: All traffic is encrypted with authenticated encryption. No plaintext ever crosses the wire. Minimal metadata in protocol headers.

### 1.2 Active Network Attacker (MITM)
- **Capability**: Can intercept, modify, inject, replay, and drop packets.
- **Goal**: Impersonate a peer, tamper with messages, inject malicious content.
- **Mitigation**: 
  - Authenticated handshake binds session keys to long-term identity keys.
  - Fingerprint verification enables out-of-band identity confirmation.
  - Replay protection via monotonic nonces and sequence numbers.
  - Tamper detection via AEAD authentication tags.

### 1.3 Malicious Peer
- **Capability**: Controls one end of the connection. Can send arbitrary data.
- **Goal**: Exploit parsing bugs, cause denial of service, exfiltrate data via crafted messages.
- **Mitigation**:
  - Strict input validation on all received data.
  - Message size limits enforced at the framing layer.
  - Rate limiting on incoming messages.
  - No auto-execution of attachments or media.
  - Sandboxed preview rendering.

### 1.4 Local Attacker (Physical Access)
- **Capability**: Has access to the filesystem or the running process.
- **Goal**: Extract keys, read message history, impersonate the user.
- **Mitigation**:
  - Keys encrypted at rest with a user-derived key.
  - Chat history encrypted with separate keys.
  - Secure memory zeroization for sensitive data in memory.
  - Optional message history disablement.
  - Secure session deletion.

### 1.5 Compromised Dependency / Supply Chain
- **Capability**: Injected malicious code into a dependency.
- **Goal**: Exfiltrate keys or plaintext.
- **Mitigation**:
  - Minimal dependency footprint.
  - No telemetry, analytics, or reporting.
  - All dependencies auditable.
  - Reproducible builds.
  - No dynamic code loading.

## 2. Assets to Protect

| Asset | Sensitivity | Storage | Protection |
|-------|-------------|---------|------------|
| Long-term identity private key | Critical | Encrypted key store | Encrypted at rest, zeroized in memory |
| Session keys | Critical | Memory only | Never persisted, zeroized after use |
| Message plaintext | High | Encrypted DB (optional) | XChaCha20-Poly1305 at rest |
| Contact list / peer identities | Medium | Encrypted DB | Encrypted at rest |
| Invite data | Medium | Ephemeral | Signed, time-limited, one-use |
| Attachments | Medium-High | Encrypted on disk | Encrypted, not auto-opened |
| Connection metadata (IPs, timing) | Medium | Not persisted | Not logged by default |

## 3. Attack Surfaces

### 3.1 Network Attack Surface
- TCP listener (accepts connections from any peer)
- Framing parser (processes untrusted byte streams)
- Handshake protocol (processes untrusted cryptographic material)
- Message deserializer (processes untrusted payloads)
- File transfer handler (processes untrusted file chunks)

### 3.2 Local Attack Surface
- Encrypted database files
- Key storage files
- Log files (must not contain secrets)
- Tauri IPC bridge (UI ↔ backend)
- Attachment storage directory

### 3.3 UI Attack Surface
- Invite paste input (untrusted string from clipboard)
- Chat message display (must sanitize for XSS)
- File download prompts (must not auto-execute)
- Fingerprint display (must not be spoofable via formatting)

## 4. Threat Matrix

| Threat | Likelihood | Impact | Mitigation Status |
|--------|-----------|--------|-------------------|
| MITM during handshake | Medium | Critical | Mitigated (signed handshake + fingerprint verification) |
| Message replay | Medium | High | Mitigated (nonce + sequence tracking) |
| Message tampering | Medium | Critical | Mitigated (AEAD authentication) |
| Malformed packet DoS | High | Medium | Mitigated (size limits, rate limiting, timeouts) |
| Key extraction (local) | Low-Medium | Critical | Mitigated (encrypted storage, zeroization) |
| Malicious attachment | High | High | Mitigated (no auto-open, sandboxed preview) |
| Log data leakage | Medium | High | Mitigated (redaction layer, no secret logging) |
| Invite forgery | Medium | High | Mitigated (Ed25519 signatures, expiry) |
| Decompression bomb | Medium | Medium | Mitigated (size caps, bounded decompression) |
| XSS via chat message | Medium | Medium | Mitigated (React escaping, CSP, no innerHTML) |

## 5. Trust Boundaries

```
┌─────────── TRUSTED ───────────────────┐
│  User's Intent                        │
│    ↓                                  │
│  UI Layer (React)                     │
│    ↓ (Tauri IPC — validated)          │
│  Rust Backend                         │
│    ↓                                  │
│  Crypto Module (libsodium)            │
│    ↓                                  │
│  Encrypted Storage                    │
└───────────────────────────────────────┘
         ↕ TRUST BOUNDARY (network)
┌─────── UNTRUSTED ─────────────────────┐
│  TCP Transport                        │
│  Remote Peer                          │
│  Attachments                          │
│  Invite Data (until verified)         │
└───────────────────────────────────────┘
```

## 6. Assumptions

1. The user's operating system is not fully compromised (kernel-level rootkit defeats all protections).
2. The Rust compiler and libsodium are not backdoored.
3. The user can perform out-of-band fingerprint verification when needed.
4. The user's clipboard is not being actively monitored by malware during invite exchange (risk documented, not mitigable at app level).
5. Time on both peers is approximately correct (for invite expiry validation).

## 7. Known Limitations

1. **Per-message ratchet is SHA-256 not Double Ratchet**: The current ratchet provides forward secrecy (compromising `tx_key_N` does not reveal earlier messages), but it does not provide self-healing (compromising `tx_key_N` does reveal all future messages until the next DH exchange). A full Double Ratchet (X3DH + DH ratchet) is planned for Phase 1.
2. **No deniability**: Messages are authenticated, which means they are provably authored. Deniable authentication is a future consideration.
3. **Single device**: No multi-device sync. The identity key exists on one machine.
4. **IP exposure**: Direct TCP connections reveal IP addresses to the peer. Tor/VPN usage is the user's responsibility.
5. **No mlock()**: Session keys are zeroized on drop but may be paged to disk. Full `mlock()` protection is planned for Phase 4.
