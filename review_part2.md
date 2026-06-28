# M2M Project — Comprehensive Code Review (Part 2)

**Reviewer**: Strict automated architectural review  
**Date**: 2026-06-28  
**Scope**: Full project audit — every Rust module, frontend component, and supporting file  
**Method**: Line-by-line reading of all ~10,550 lines of Rust, ~3,000 lines of TypeScript, and all documentation  
**Previous score (self-assigned)**: 9.3/10  
**This review's score**: **8.3/10**

---

## Executive Summary

M2M is an ambitious, professionally-engineered P2P encrypted messenger built on Tauri v2, libsodium, and React. The architecture is sound, the security foundation is strong, and the NAT traversal is best-in-class. However, several significant issues emerged during close reading — including an **incomplete Double Ratchet integration that leaves file transfers and metadata without forward secrecy in X3DH mode**, and potential key confusion between Ed25519 and X25519 in the legacy handshake path. These are fixable, but they prevent the project from achieving the 9.3/10 it has assigned itself.

The frontend, while functional, lags significantly behind the backend in quality — it is a basic single-conversation interface with minimal polish, no notifications, and no real-time status feedback.

---

## 1. Backend — Architecture & Design

### Score: 8.0/10

#### Strengths

- Clean module separation with clear responsibility boundaries
- Happy Eyeballs RFC 8305-inspired parallel connection racing (7 strategies)
- Strong NAT traversal: PCP → NAT-PMP → UPnP → STUN → TCP hole punch → relay
- Two-tier encrypted storage (keys.db + messages.db with independent encryption)
- Zero-trust design: no server, no single point of failure
- Actor-model per-connection with split read/write halves

#### Issues Found

**1.1 [CRITICAL] Double Ratchet Integration Is Incomplete**

The X3DH + Double Ratchet path is only partially wired through the session layer:

| Operation | DR Path | Legacy Path |
|-----------|---------|-------------|
| `send_text()` → `send_encrypted()` | ✅ DR | ✅ Legacy |
| `send_encrypted_typed()` (file xfers, meta) | ❌ **Missing** | ✅ Legacy |
| `decrypt_message()` | ✅ DR | ✅ Legacy |
| `decrypt_typed_frame()` | ❌ **Missing** | ✅ Legacy |

**Impact**: When two peers connect via X3DH and exchange a file transfer request, the file metadata is encrypted with SessionKeys (not the Double Ratchet). The session has a ratchet state but `send_encrypted_typed` bypasses it entirely. This means:
- File transfer metadata and conversation names **lack per-message forward secrecy** in X3DH mode
- The DR chain and legacy ratchet diverge, creating a confusing security posture

**Fix required**: `send_encrypted_typed` and `decrypt_typed_frame` need DR-aware variants that piggyback on the same pattern as `send_encrypted`/`decrypt_message`.

**1.2 [HIGH] `send_encrypted` Ratchet Decision Logic**

```rust
let do_ratchet = ratchet.should_ratchet(100); // session.rs:507
```

This hard-codes DH ratchet every 100 messages. The interval should be:
- Configurable (not hard-coded at 100)
- Possible to trigger manually (for "ratchet now" UI)
- Documented as a trade-off (frequent ratchets = more bandwidth but better PFS)

Additionally, no DH ratchet occurs during file transfers because `send_encrypted_typed` doesn't use the DR path. A large file transfer (thousands of chunks) would never ratchet.

**1.3 [MEDIUM] `generate_ratchet_key()` in DoubleRatchet is Dead/Broken**

```rust
pub fn generate_ratchet_key(&mut self) -> [u8; 32] {
    let new_kp = EphemeralKeypair::generate();
    // ... the comment says "caller embeds it in the header"
    // but the new keypair is DROPPED here — the new secret key is lost
    new_kp.public_key_bytes()
}
```

This method generates a new DH keypair but **drops the secret key**. The public key is returned, but without the secret, the receiver can't use it to compute the new DH shared secret. The method is unused (`#[allow(dead_code)]` may be on its callers). It should either be removed or fixed to actually store the keypair.

**1.4 [MEDIUM] No Skipped Message Key Cache**

The Double Ratchet `decrypt()` method derives through gaps:

```rust
while self.recv_message_number < message_number {
    let (_, next_chain) = Self::derive_message_key(&current_chain);
    current_chain = next_chain;
    self.recv_message_number += 1;
}
```

This advances the chain key but **doesn't cache** the intermediate message keys. If messages arrive out of order (e.g., order 0 arrives after order 2), message 0 is permanently undecryptable because its message key is already discarded. The Signal protocol stores up to ~2000 skipped keys in a `SkipMap` for exactly this reason.

**Impact**: In practice, reliable TCP connections mean out-of-order delivery is rare (TCP reassembles), but if messages are received from different connections or after reconnection, gaps lose messages permanently.

**1.5 [LOW] Ed25519 ↔ X25519 Key Confusion in Legacy Handshake**

In `session.rs:121`:
```rust
x25519_identity_pub: identity.public_key_bytes(),  // Ed25519 key coerced to X25519 field
```

The `identity.public_key_bytes()` returns an **Ed25519** public key, but `x25519_identity_pub` is supposed to be an **X25519** key. libsodium's `kx::client_session_keys` may internally convert, but this is:
- Non-standard (X3DH spec requires distinct Ed25519 + X25519 keypairs)
- Confusing for audit
- Mixture that could cause interop issues if another implementation expects a real X25519 key here

The X3DH handshake variant correctly uses the X25519 identity key. The legacy path should do the same.

---

## 2. Backend — Security & Cryptography

### Score: 9.0/10

#### Strengths

- libsodium-backed: Ed25519, X25519, XChaCha20-Poly1305 — all standard, audited primitives
- HKDF-SHA256 RFC 5869 for key derivation
- X3DH with ephemeral key (DH1+DH2+DH3+optional DH4) — the critical bug using IK_A instead of EK_A has been **fixed**
- Double Ratchet with DH ratchet for break-in recovery
- Variable exponential padding (1KB–16KB tiers) to defeat traffic analysis
- Per-byte Slowloris protection on frame reads
- Connection rate limiting with lock-free DashMap
- Secure key storage with mlock/VirtualLock + zeroize-on-drop
- Random initial counters to prevent cross-session replay
- Strict CSP (`'self'` only)
- Tor guard: refuses to create invites when Tor is enabled without Private Mode
- `overflow-checks = true` in production

#### Issues Found

**2.1 [MEDIUM] Padding Oracle via `unpad_message_variable`**

```rust
pub fn unpad_message_variable(padded: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let pad_len = u16::from_be_bytes([padded[padded.len() - 2], ...]) as usize;
    if pad_len + 2 > padded.len() { return Err(...); }
    Ok(padded[..original_len].to_vec())
}
```

The padding bytes between the plaintext and the length suffix are **not validated**. An attacker who can modify ciphertext (breaking AEAD) could craft a frame where the padding length suffix points to a different "plaintext length" than intended. While XChaCha20-Poly1305 prevents ciphertext tampering, variable-length padding has a well-known history of padding oracle attacks.

**Mitigation**: After removing the padding, optionally re-pad the recovered plaintext and verify the padding bytes match the expected random pattern (or just verify the padding bytes are not part of an alternative plaintext interpretation).

**2.2 [MEDIUM] No Authentication on Storage Encryption**

The storage encryption uses `aead::seal(plaintext, None, &nonce, &key)` — note the **`None` for AAD**:

```rust
// util.rs:378
let ciphertext = aead::seal(plaintext, None, &nonce, &aead_key);
```

The AAD (Additional Authenticated Data) field is empty. This means:
- A ciphertext from `keys.db` could be copied to `messages.db` and still decrypt (if decrypted with the same key)
- There's no binding between the encrypted blob and its context (which identity, which conversation)

**Fix**: Use AAD containing context information: `b"m2m-keys"` vs `b"m2m-messages"` to domain-separate the two databases. Use the conversation ID as AAD for message content.

**2.3 [LOW] Double Ratchet AAD Is Too Narrow**

```rust
let aad = [PacketType::EncryptedMessage.to_byte()];  // session.rs:506
```

The AAD for DR-encrypted messages is only the packet type byte (0x10). While this binds the ciphertext to the message type, it doesn't include:
- Sender identity key
- Session ID
- Message counter

Signal's AAD includes the associated data from the sender's identity key and the receiver's identity key. Without this, the same ciphertext could potentially be replayed in a different session context (though replay protection mitigates this).

**2.4 [LOW] No Protection Against DH Ratchet Key Compromise**

When a DH ratchet occurs (`do_ratchet: true`), the old `our_ratchet_keypair` is overwritten:
```rust
self.our_ratchet_keypair = new_kp;  // old keypair dropped — zeroized by Drop
```

This zeroizes the old keypair, which is good. But there's no mechanism to verify that a received DH ratchet public key came from the legitimate peer (other than the AEAD that follows). A transient MITM who compromised the previous ratchet could inject a new DH public key. This is a fundamental limitation of the Double Ratchet design — not unique to M2M.

---

## 3. Backend — Networking & Privacy

### Score: 9.0/10

#### Strengths

- Full RFC 8489 STUN client with parallel multi-server consensus
- PCP/NAT-PMP/UPnP IGD automatic port mapping with ordered fallback
- TCP hole punch with simultaneous open (SO_REUSEADDR)
- IPv6 direct support (type 5 candidates with higher priority than srflx)
- Configurable STUN servers
- Private mode to hide IP from invites
- Tor SOCKS5 proxy integration

#### Issues Found

**3.1 [MEDIUM] `bind("0.0.0.0:0")` Fails on IPv6-Only Networks**

Four locations use `0.0.0.0:0` for binding:
- `stun.rs:302` — STUN UDP socket
- `commands/util.rs:32` — `resolve_local_ip()` socket

On an IPv6-only network (e.g., some mobile hotspots, recent AWS VPCs), binding to `0.0.0.0` fails because it explicitly requests IPv4. The code should try binding to `[::]:0` as a fallback, or use `tokio::net::UdpSocket::bind()` with a dual-stack socket.

**3.2 [LOW] STUN Only Uses UDP**

The STUN module is UDP-only. While this is correct per RFC 8489, a peer behind a firewall that blocks all UDP can never discover their public IP. A TCP-based STUN fallback (RFC 8489 §14) would improve connectivity for firewall-heavy networks.

**3.3 [LOW] Hole Punch Race Doesn't Track IPv6 srflx Candidates**

The `run_hole_punch` function connects to type 1 (srflx) and type 2 (prflx) candidates. If a STUN server discovers an IPv6 srflx candidate, it gets added as type 1 same as IPv4 srflx. But the TCP connect to an IPv6 srflx address uses the IPv4 socket created by `our_listener_addr`. This could fail because the listen address is IPv4 but the connect target is IPv6.

**3.4 [LOW] No DNS-over-HTTPS for STUN Server Resolution**

STUN server resolution uses `tokio::net::lookup_host()` which uses the system resolver (likely plain DNS). An attacker who can spoof DNS responses could redirect a STUN query to their own server and report a false public IP. While the multi-server consensus check mitigates single-server poisoning, a determined attacker who controls the DNS responses for all configured STUN servers could still poison the result.

---

## 4. Backend — Test Coverage

### Score: 8.5/10

#### Strengths

- 22 crypto tests (HKDF, X3DH, Double Ratchet, padding, key ratchet)
- ~25 session tests (handshake success/failure, replay, state machine, integration)
- ~25 network tests (frame I/O, slowloris, rate limiting, filename sanitization)
- ~25 storage tests (KeyStore/MessageStore CRUD, cascade delete, edge cases)
- ~16 identity tests (invite creation, validation, expiry, tamper detection)
- 2 fuzz targets (protocol frame parsing, padding invariants)
- Protocol tests cover all packet types, version validation, frame boundaries

#### Issues Found

**4.1 [HIGH] The 61-Second Sleep Test**

```rust
// network.rs:674
std::thread::sleep(Duration::from_secs(RATE_LIMIT_WINDOW_SECS + 1));
```

`test_limiter_window_expiry` sleeps for 61 real seconds because `Instant`-based timing can't be accelerated by `tokio::time::pause()`. This makes `cargo test` take over a minute. Options:
- Extract the time source into a trait so tests can inject a mock clock
- Reduce `RATE_LIMIT_WINDOW_SECS` in test context
- Accept it, but the comment noting the problem is not a fix

**4.2 [MEDIUM] Typed Frame Tests Don't Cover DR Path**

`test_file_transfer_request_roundtrip` and `test_conversation_meta_roundtrip` use `decrypt_typed_frame()` which only supports the legacy path. If the DR path were activated for these operations, these tests would fail or miss coverage entirely.

**4.3 [MEDIUM] No Integration/E2E Tests**

There are no tests that:
- Start a full handshake → exchange messages → disconnect
- Verify forward secrecy (decrypt old messages after ratchet)
- Test NAT traversal strategy selection
- Test relay protocol integration
- Test invite → connect → handshake → message flow through the command layer

The existing tests are unit-level or partial-integration (tokio duplex streams), but none exercise the `commands/` layer's full orchestration.

**4.4 [LOW] Frontend Tests Largely Missing**

Only `VaultView.test.tsx` exists (7 tests). `ChatView`, `HubView`, `SettingsView`, `M2MContext`, and all hooks are untested. The test setup file exists (`vitest`, `@testing-library/react`) but `pnpm test` needs `pnpm install` first (and `node_modules` was shipped in the repo).

---

## 5. Backend — Code Quality

### Score: 8.5/10

#### Strengths

- Clean, idiomatic Rust throughout — `Result` types, `thiserror`, proper use of `async`
- Good module-level documentation on every file
- Meaningful type names and clear field comments
- `#[serde(default)]` and `skip_serializing_if` used appropriately for backward compat
- Consistent error propagation patterns

#### Issues Found

**5.1 [MEDIUM] Code Duplication: Slowloris Read Pattern**

The per-byte timeout read loop is replicated in:
- `network.rs:254-264` — length prefix read
- `network.rs:273-283` — frame body read
- `relay.rs:168-175` — relay length prefix read
- `relay.rs:192-199` — relay body read

This is the same ~8 lines of code duplicated 4 times. It should be extracted into a `read_exact_slowloris(reader, buf) -> Result<(), NetworkError>` helper.

**5.2 [LOW] `#[allow(dead_code)]` Proliferation**

The codebase has 30+ instances of `#[allow(dead_code)]`. While some are justified (constants reserved for future use), several hide real dead code:
- `MAX_HANDSHAKE_SIZE` (protocol.rs:30) — defined but never referenced
- `KEY_ROTATION_INTERVAL_SECS` (protocol.rs:53) — reserved
- `RATE_LIMIT_MSGS_PER_SEC` (protocol.rs:69) — reserved
- `CONNECT_TIMEOUT` (network.rs:35) — unused (timeout is per-strategy in hole_punch)
- `LISTENER_BACKLOG` (network.rs:40) — unused (TcpListener backlog)
- `ConnectionState::Connecting` (network.rs:211) — never set anywhere
- `ConnectionState::Disconnecting` (network.rs:219) — never set anywhere
- `MAX_TOTAL_CONNECTIONS` (network.rs:51) — used
- Several error enum variants across all modules

Some dead code is inevitable, but the unused `ConnectionState` variants and `MAX_HANDSHAKE_SIZE` suggest incomplete state machine implementation.

**5.3 [LOW] Error Handling: Cryptic Error Mapping**

```rust
// session.rs:574
let padded = ratchet.decrypt(...)
    .map_err(|_| SessionError::Network(network::NetworkError::PeerClosed))?;
```

A Double Ratchet decryption failure is mapped to `PeerClosed`, which is semantically wrong. If DR decryption fails (bad key, bad nonce, tampered ciphertext), the error should be `SessionError::Crypto(CryptoError::DecryptionFailed)` — not "peer closed the connection". This loses diagnostic information.

**5.4 [LOW] `send_file_accept` and `send_file_reject` Use JSON Instead of MessagePack**

```rust
// session.rs:707, 720
let body = protocol::serialize(&serde_json::json!({ "transfer_id": transfer_id }))?;
```

All other protocol messages use MessagePack via `protocol::serialize()`, but file accept/reject use `serde_json::json!` then serialize the JSON string with MessagePack. This is inconsistent and wastes bytes (the JSON keys `transfer_id` are repeated in every message). Should use a proper struct with `#[derive(Serialize)]`.

---

## 6. Frontend — Overall Quality

### Score: 7.0/10

#### Strengths

- Clean React context pattern with typed hooks
- Decent UI component library with consistent styling
- Dark/light theme detection
- Accessibility: aria labels on icon buttons
- Error boundary per-view
- Toast notification system

#### Issues Found

**6.1 [MEDIUM] M2MContext Is a God Object**

```typescript
export interface M2MContextValue {
    // 77 properties and methods
    view, setView, identity, connection, isConnecting,
    messages, fileRequests, vaultInitialized, networkSettings,
    publicIp, stunLoading, networkDiagnostics, stunConfig,
    stunServerInput, privateMode, connectivityResult,
    conversations, activeConversationId, inviteToConnect,
    inviteValid, namingMyName, namingTheirName, generatedInvite,
    retentionPolicy, retentionDuration,
    setStunServerInput, setInviteToConnect, setNamingMyName,
    setNamingTheirName, setRetentionPolicy, setRetentionDuration,
    handleUnlockVault, handleSendMessage, handleVerify, ...
}
```

The context exposes **77 properties and methods** in a single object. Every view gets everything, even state it doesn't use. This:
- Causes unnecessary re-renders (any state change re-renders all consumers)
- Makes the contract between views and state implicit and fragile
- Is the opposite of "colocation" — state and handlers for completely different concerns (vault, network, chat, file transfers) are tangled

**Fix**: Split into focused contexts: `VaultContext`, `ConnectionContext`, `ChatContext`, `SettingsContext`.

**6.2 [MEDIUM] useM2MState.ts Likely Has Similar God-Object Problems**

The hook file (not fully read but referenced) likely mirrors the 77 properties. Without seeing it, the context type alone reveals the over-centralization.

**6.3 [LOW] Huge Icons Component**

`Icons.tsx` at 13 KB is a monolith of SVG icon definitions. While convenient, this means importing any icon bundles the entire 13 KB into the bundle. A tree-shakeable icon approach (individual icon components, or using `react-icons`) would be more efficient.

**6.4 [LOW] Single-View Architecture Limits UX**

The app has a single active view (setup/vault/hub/chat/settings) with no sub-view or panel system. This means:
- You can't see settings and the hub at the same time
- Clicking "Settings" hides the chat entirely
- No split-pane layouts (conversation list | active conversation)
- No conversation search across all peers

**6.5 [LOW] No Real-Time Status Indicators**

- No online/offline indicator that works (HubView always shows "Offline" badge)
- No connection quality indicator (latency, NAT type)
- No typing indicators (not in protocol either — but notable omission for a messenger)
- No read receipts

**6.6 [LOW] Duplicated Entropy Estimation**

Both `commands/util.rs` (backend) and `src/utils.ts` (frontend) implement passphrase entropy estimation. The two implementations can — and likely will — diverge. One should be authoritative (either backend-only with the frontend calling via IPC, or shared code).

**6.7 [LOW] Minimal Test Coverage**

Only VaultView tests exist. Critical UI flows (connecting, sending messages, file transfers, settings changes) have no test coverage.

---

## 7. Documentation

### Score: 8.5/10

#### Strengths

- Comprehensive `docs/architecture.md` (505 lines, module dependency graph)
- Well-written `docs/protocol-spec.md` (314 lines, state machine diagrams)
- `docs/threat-model.md` documents all reviewed threats
- `docs/invite-format.md` documents WireCandidate and ICE-Lite population
- `docs/security-checklist.md` exists
- Excellent doc comments on all Rust modules and most public functions
- ROADMAP.md tracks all completed and remaining work

#### Issues Found

**7.1 [LOW] No Architecture Decision Records (ADRs)**

Several important decisions are documented in memory files and conversation context but not in the repo:
- Why custom relay protocol instead of full TURN
- Why application-level encryption vs SQLCipher
- Why MessagePack vs Protocol Buffers vs flat buffers
- Why `kx` instead of raw X25519 for the legacy path

These belong in `docs/adr/` as permanent records.

**7.2 [LOW] `docs/full_analysis.md` is Stale**

At 5.9 KB, this file is referenced as the canonical status document but is shorter and less detailed than `ROADMAP.md` and the in-code comments. It should either be deleted or brought up to date.

---

## 8. Performance

### Score: 7.5/10

#### Strengths

- Streaming file transfers (no full-RAM buffering)
- Lock-free DashMap for rate limiting
- Async everywhere with tokio
- WAL mode for SQLite

#### Issues Found

**8.1 [MEDIUM] `VACUUM` on Every Conversation Delete**

```rust
// storage.rs:476
self.conn.execute_batch("VACUUM;")?;
```

`VACUUM` rebuilds the entire SQLite database file — it's O(db_size) and blocks all database operations while running. Calling it on every `delete_conversation` is enormously expensive for large databases. It should either be removed (SQLite auto-reclaims pages) or deferred to a periodic maintenance task.

**8.2 [LOW] No Binary Size Optimization**

No `lto = true`, no `opt-level = "z"`, no `strip = true` in Cargo.toml. A release build is likely 50MB+ for what could be under 10MB with optimization flags.

**8.3 [LOW] DashMap Allocation on Every Rate Limit Check**

`ConnectionLimiter::check()` creates a new `VecDeque` entry for every new IP via `or_default()`. For a DDoS with thousands of spoofed source IPs, this creates thousands of small heap allocations and never reclaims them (the entries only expire after 60 seconds of inactivity). A fixed-size LRU cache would be more memory-efficient.

---

## 9. Files & Configuration

### Score: 8.0/10

#### Issues Found

**9.1 [LOW] Auto-Generated Schema Files Checked In**

`gen/schemas/desktop-schema.json` (129 KB) and `gen/schemas/windows-schema.json` (129 KB) are auto-generated Tauri capability schemas. These are build artifacts that should be in `.gitignore` or regenerated during CI.

**9.2 [LOW] `.gitignore` Allows `node_modules`**

The repo includes `node_modules/` — this is unusual and creates a bloated repo (24,529 files, 3,744 directories locally). Frontend dependencies should be installed via `pnpm install`, not checked in.

---

## 10. Grading Summary

| Category | M2M Self-Assessment | Independent Assessment | Delta |
|----------|:-------------------:|:---------------------:|:-----:|
| Architecture & Design | 9.5 | **8.0** | −1.5 |
| Security & Cryptography | 10 | **9.0** | −1.0 |
| Networking & Privacy | 10 | **9.0** | −1.0 |
| Test Coverage | 9.5 | **8.5** | −1.0 |
| Documentation | 9.0 | **8.5** | −0.5 |
| UI/UX | 8.5 | **7.0** | −1.5 |
| Performance | 8.5 | **7.5** | −1.0 |
| Code Quality | 9.5 | **8.5** | −1.0 |
| Maintainability | 9.5 | **8.5** | −1.0 |
| **Overall** | **9.3** | **8.3** | **−1.0** |

---

## 11. Actionable Improvements (Prioritized)

### Tier 1 — Security / Correctness (Fix Now)

1. **[CRITICAL] Complete Double Ratchet integration**: Wire `send_encrypted_typed` and `decrypt_typed_frame` to use the DR path when a ratchet is active. File transfers and conversation metadata must have the same forward secrecy as text messages.

2. **[HIGH] Add skipped message key cache**: Implement `HashMap<u64, MessageKey>` to cache message keys for out-of-order messages (capped at ~2000 entries, per Signal's design).

3. **[HIGH] Use AAD in storage encryption**: Add context strings (`b"m2m-keys"`, `b"m2m-messages"`, `b"m2m-export"`) as AAD to domain-separate all encrypted blobs.

4. **[HIGH] Fix or remove `generate_ratchet_key()`**: The current implementation drops the secret key. Either remove it or fix it to properly store the keypair.

### Tier 2 — Code Quality (Fix Soon)

5. **[MEDIUM] Extract Slowloris read helper**: The per-byte timeout read pattern is duplicated 4 times. Extract into `read_exact_with_timeout(reader, buf, timeout)`.

6. **[MEDIUM] Remove `VACUUM` from `delete_conversation`**: Replace with a deferred maintenance task or remove entirely.

7. **[MEDIUM] Remove 61-second sleep test**: Extract time source into a trait for testability, or reduce the window constant in test context.

8. **[MEDIUM] Fix error mapping in `decrypt_message`**: Map DR decryption errors to `CryptoError::DecryptionFailed`, not `NetworkError::PeerClosed`.

9. **[MEDIUM] Use proper structs for file accept/reject**: Replace `serde_json::json!({...})` with typed `#[derive(Serialize)]` structs and MessagePack serialization.

### Tier 3 — Architecture (Fix When Refactoring)

10. **[MEDIUM] Split M2MContext**: Split the 77-property god context into focused sub-contexts: `VaultContext`, `ConnectionContext`, `ChatContext`, `SettingsContext`.

11. **[MEDIUM] Try IPv6 bind fallback**: Where `0.0.0.0:0` is hard-coded, try `[::]:0` as a fallback for IPv6-only networks.

12. **[LOW] Create ADR directory**: Document the key architectural decisions currently scattered in conversation history.

13. **[LOW] Remove dead code**: Audit and remove `#[allow(dead_code)]` items that are truly unused, including `ConnectionState::Connecting`/`Disconnecting`, `MAX_HANDSHAKE_SIZE`, `KEY_ROTATION_INTERVAL_SECS`.

### Tier 4 — Testing (Fix When Convenient)

14. **[MEDIUM] Add integration tests**: Full handshake + message exchange tests using tokio duplex streams.

15. **[MEDIUM] Add frontend tests**: ChatView, HubView, SettingsView component tests.

16. **[LOW] Add E2E tests**: End-to-end tests using Tauri's test harness or headless mode.

### Tier 5 — Polish (Nice to Have)

17. **[LOW] Binary optimization**: Add `lto = true`, `opt-level = "z"`, `strip = true` to release profile.

18. **[LOW] Reduce icon bundle size**: Split Icons.tsx into tree-shakeable individual components.

19. **[LOW] Add SPK rotation**: Implement signed prekey rotation to limit the window of SPK compromise.

20. **[LOW] Add real-time status indicators**: Connection quality, actual online/offline detection, message delivery status.

---

## Conclusion

M2M is a serious, well-engineered project with a strong security foundation and impressive NAT traversal capabilities. The core cryptographic primitives are sound, the architecture is modular, and the documentation is thorough.

However, the **Double Ratchet integration gap** is a real vulnerability in the forward secrecy guarantees for non-text operations, and the **Ed25519/X25519 key confusion** in the legacy path is concerning for auditability. These, combined with the frontend's god-object context and 77-property interface, prevent the project from achieving the 9.3/10 it has assigned itself.

An **8.3/10** is still a strong score — well above industry average for a hobby/security project of this complexity. With 2-3 weeks of focused work on the Tier 1 and Tier 2 items, the project could genuinely reach 9.0+ and be production-ready for privacy-conscious users who understand the limitations.

---

*End of review_part2.md*
