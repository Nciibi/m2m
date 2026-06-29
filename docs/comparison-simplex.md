# M2M vs SimpleX Chat — Architecture & Design Comparison

**Date**: 2026-06-29  
**M2M version**: 2.8.x (Rust/Tauri desktop)  
**SimpleX version**: v6.5.5 (Haskell/Kotlin/Swift, multi-platform)  
**Comparison basis**: Public repository analysis, protocol documentation, and codebase review

---

## 1. High-Level Comparison

| Dimension | M2M | SimpleX Chat | Advantage |
|-----------|:---:|:------------:|:---------:|
| **Architecture** | P2P (direct TCP) | Client-server (SMP relay queues) | Different goals |
| **Network topology** | Fully decentralized, no servers | Decentralized relays, no federation | Different |
| **Identifiers** | Ed25519 public key (persistent identity) | **None at all** | SimpleX |
| **Metadata resistance** | Peer sees your IP; server sees nothing | Relays see unlinkable ciphertext flows | SimpleX |
| **NAT traversal** | 7 strategies (STUN, UPnP, hole punch, relay, ...) | Not needed (client-server architecture) | M2M (solves harder problem) |
| **Encryption** | X3DH + Double Ratchet (XChaCha20-Poly1305) | TLS + NaCl per-queue + Double Ratchet + PQ ratchet | SimpleX (PQ added) |
| **Post-quantum** | No | Yes (on every ratchet step) | SimpleX |
| **Audits** | Self-reviewed | Trail of Bits (2022, 2024) | SimpleX |
| **Platforms** | Desktop (Tauri Win/Mac/Linux) | iOS, Android, Desktop, CLI, Bot SDK | SimpleX |
| **Audio/Video calls** | No | Yes (E2E WebRTC) | SimpleX |
| **Groups** | 1:1 only | Groups, communities, public channels | SimpleX |
| **File transfer** | Streaming encrypted (no full buffering) | XFTP (separate protocol) | Comparable |
| **Development maturity** | ~6K commits, small team | 15K ★, 259 releases, 218 contributors, 6K commits | SimpleX |
| **Languages** | Rust + TypeScript | Haskell + Kotlin + Swift + TS | M2M (Rust ecosystem) |

---

## 2. Architectural Philosophy

### SimpleX: Client-Server with Ephemeral Queues

SimpleX's architecture is built around **unidirectional SMP message queues** hosted on relay servers:

```
Alice → [SMP queue A→B] → Bob
Alice ← [SMP queue B→A] ← Bob
```

Each queue has two cryptographic addresses (sender, recipient). Servers are **stateless with respect to messages** — they hold messages only until delivered, then delete them. Servers never communicate with each other.

**Key trade-off**: SimpleX depends on relay servers being available. Users can run their own, and the default ones are best-effort. But the architecture fundamentally requires infrastructure.

**Why this matters for metadata**: Because each conversation uses **distinct queue addresses** with no common identifier, even a global adversary who monitors all relay servers cannot determine that Alice and Bob are communicating through different queues — there's no shared metadata to link them.

### M2M: True P2P with Happy Eyeballs

M2M's architecture is pure peer-to-peer:

```
Alice ↔ [direct TCP / hole punch / relay] ↔ Bob
```

No servers required. Connection establishment races 7 strategies in parallel and takes the first successful path.

**Key trade-off**: P2P requires NAT traversal, which is inherently unreliable. M2M handles this with extensive tooling (7 strategies + relay fallback), but symmetric NAT pairs may fail entirely. The relay server (optional) provides a backup path.

**Why this matters for metadata**: The peer **must know your IP address** to connect. While private mode hides it from invites, the connected peer always sees your source IP. This is a fundamental metadata leak that P2P cannot eliminate.

### Verdict

- **SimpleX wins** on metadata resistance and reliability (no NAT issues).
- **M2M wins** on sovereignty (zero infrastructure required) and simplicity of deployment (install and run).
- The architectures serve different threat models: M2M assumes the threat is centralized infrastructure; SimpleX assumes the threat is also metadata correlation from peer connections.

---

## 3. Metadata Protection — The Decisive Gap

This is the single largest difference between the projects.

| Attack / Scenario | M2M | SimpleX |
|---|---|---|
| Server sees who talks to whom | N/A (no servers) | Impossible (servers see only queues, no user identity) |
| Peer learns your IP | ✅ **Always** (TCP connection) | ❌ Relayed via SMP queue |
| Global passive adversary correlates traffic | Possible (P2P has timing signals) | Future planned: message mixing |
| Graph analysis | Limited (no central directory) | Impractical (no identifiers across queues) |
| ISP sees you're using M2M/SimpleX | Visible (direct TCP to peer IPs) | Visible (TCP to relay, but destination is relay, not peer) |
| Sender concealment | ❌ Peer sees sender IP | ✅ Private routing (v6.0+, sender IP hidden from recipient's relay) |

**The fundamental constraint**: P2P requires the peer's IP address for connection. Any P2P messenger — including Session (Oxen), Tox, Briar — shares this metadata leakage. SimpleX's client-server model avoids it entirely.

### Mitigations M2M Could Adopt

1. **Tor integration (partially done)**: M2M already has Tor SOCKS5 support for outgoing connections. This hides M2M's IP from the peer, but at significant latency cost.
2. **Relay as default, not fallback**: Currently the relay is lowest-priority in Happy Eyeballs. Making relay the default for internet peers (with direct connections only for LAN) would trade latency for metadata protection.
3. **Multi-hop relay**: Route through two relays to conceal the sender from the recipient's relay (Similar to SimpleX's private routing v6.0).
4. **Per-conversation queue addresses**: Adopt the simplex queue model for the relay path so the relay can't link conversations.

---

## 4. Encryption & Cryptography

| Feature | M2M | SimpleX |
|---------|:---:|:-------:|
| **Key exchange** | X3DH (Curve25519) | X3DH-variant (Curve448) |
| **Double Ratchet** | Yes (SHA-256 based) | Yes (Signal algorithm) |
| **Post-quantum ratchet** | **No** | **Yes** (on every step) |
| **Transport encryption** | TCP-level (no TLS) | TLS 1.2/1.3 (restricted ciphers) |
| **Per-queue encryption** | N/A | NaCl cryptobox |
| **Storage encryption** | XChaCha20-Poly1305 + Argon2id | Passphrase-encrypted DB |
| **Padding** | Tiered exponential (1KB–16KB) | Multiple levels (details TBD) |
| **Zero-knowledge proofs** | No | No |
| **Crypto libraries** | libsodium (Rust bindings) | libsodium (C, via Haskell bindings) |

### Post-Quantum Readiness

SimpleX **adds post-quantum key exchange at every Double Ratchet step**, not just during initial handshake. This is more aggressive than:
- **Signal's PQXDH**: PQ only on initial key agreement
- **Apple's iMessage PQ3**: PQ on initial + periodic rotation

M2M has no PQ story at all. If a CRQC (Cryptographically Relevant Quantum Computer) arrives, all M2M message history becomes decryptable retroactively.

**M2M gap**: Adding PQ KEM to the DH ratchet step would bring M2M to parity. The `crypto.rs` hkdf-based ratchet is a natural extension point — replace the DH shared secret computation with a KEM encapsulation + shared secret.

### Audit Gap

SimpleX has undergone **two independent Trail of Bits audits** (implementation review 2022, cryptographic protocol review 2024). M2M has no external audit — all security review has been self-conducted.

---

## 5. NAT Traversal & Connectivity

This is where **M2M clearly excels** — because it must. SimpleX avoids the problem entirely.

| Strategy | M2M | SimpleX |
|----------|:---:|:-------:|
| Direct LAN TCP | ✅ | ❌ (relayed) |
| IPv6 direct | ✅ | ❌ (relayed) |
| UPnP/NAT-PMP/PCP port mapping | ✅ (3 protocols, ordered fallback) | ❌ |
| STUN server-reflexive | ✅ (RFC 8489, parallel multi-server, consensus) | ❌ |
| TCP hole punch | ✅ (simultaneous open) | ❌ |
| Custom relay | ✅ (lowest-priority fallback) | ✅ (primary path) |
| **Total strategies** | **7** | **1** |

**M2M's advantage**: On a LAN between two peers on the same subnet, M2M connects in <1ms with direct TCP. SimpleX routes through an internet relay even for "same room" connections, adding latency and dependency.

**SimpleX's advantage**: Always works (subject to relay availability). No NAT surprises. No failed hole punches. No STUN server dependencies.

---

## 6. Feature Comparison

| Feature | M2M | SimpleX |
|---------|:---:|:-------:|
| Text messaging | ✅ | ✅ |
| File transfer | ✅ (streaming, no RAM buffer) | ✅ (XFTP) |
| Voice messages | ❌ | ✅ |
| Video messages | ❌ | ✅ |
| Audio calls | ❌ | ✅ (E2E WebRTC) |
| Video calls | ❌ | ✅ (E2E WebRTC) |
| Disappearing messages | ✅ (per-conversation retention) | ✅ |
| Message reactions | ❌ | ✅ |
| Message editing | ❌ | ✅ |
| Groups | ❌ (1:1 only) | ✅ (groups, communities, channels) |
| Multiple profiles | ❌ | ✅ |
| Hidden chat profiles | ❌ | ✅ |
| Database encryption | ✅ (XChaCha20 + Argon2id) | ✅ (passphrase) |
| App passcode | ❌ | ✅ |
| Tor support | ✅ (SOCKS5) | ✅ (native) |
| Private message routing | ❌ | ✅ (v6.0+) |
| Bot SDK | ❌ | ✅ (TypeScript) |
| Markdown formatting | ❌ | ✅ |
| Notifications | ✅ (OS-level) | ✅ (OS-level + APNs relay) |
| Export conversation | ✅ (encrypted JSON) | ✅ |

---

## 7. Development & Community

| Metric | M2M | SimpleX |
|--------|:---:|:-------:|
| GitHub Stars | ~50 | ~15,000 |
| Contributors | ~5 | 218 |
| Commits | ~600 | 6,183 |
| Releases | ~50 | 259 |
| Core language | Rust (1 language) | Haskell + Kotlin + Swift (3 languages) |
| Codebase size | ~11,500 Rust + ~3,000 TS | ~58,000 Haskell + ~47,000 Kotlin + ~44,000 Swift |
| Build system | Cargo | Cabal + Gradle + Xcode |
| CI/CD | GitHub Actions | GitHub Actions (multi-platform) |
| Funding | None disclosed | Donations + investor funding |
| License | MIT | AGPL-3.0 |

### Staffing Complexity

SimpleX maintains **three separate native UI codebases** (Kotlin for Android, Swift for iOS, TypeScript for bot SDK) plus the Haskell core. This is a significant maintenance burden but gives native platform quality.

M2M's **Tauri + React** approach provides a single codebase for desktop (Windows/Mac/Linux) with the option to extend to mobile via Tauri Mobile. The trade-off is that the UI feels less native.

---

## 8. What M2M Could Learn from SimpleX

### High Impact (Architecture)

1. **Post-quantum ratchet steps**: Adding a KEM to the DH ratchet step would future-proof M2M's forward secrecy. The existing `hkdf(root_key, dh_shared_secret, ...)` pattern in `crypto.rs` is a natural integration point.

2. **Private relay routing**: Making relay the default (not fallback) for internet peers would conceal the sender's IP from the recipient — a major metadata improvement. Currently the relay is lowest-priority in Happy Eyeballs and only used when all direct strategies fail.

3. **Per-conversation relay identities**: When using relay mode, each conversation should use a distinct relay_id to prevent the relay from linking conversations to the same user.

### Medium Impact (Features)

4. **Message reactions**: A common feature request that's protocol-simple (a reaction is just a short text/emoji attached to a message ID). Would need new packet types and chat UI.

5. **Voice messages**: Encrypted audio files sent via the existing file transfer mechanism. The streaming file transfer already supports this — just needs a recorder UI.

6. **Markdown rendering**: Render markdown in message bubbles. The `MessageBody::Text` already carries raw text; frontend rendering is the only gap.

### Lower Impact (Polish)

7. **Connection quality indicator**: Show latency/NAT-type/direct-vs-relayed status in the chat header. M2M already tracks this in `StrategyResult` but doesn't display it.

8. **Online/offline indicators**: Replace the hard-coded "Offline" badge in HubView with real connection state from `AppState.connections`.

9. **App passcode**: A local PIN/passcode lock on top of the vault passphrase, for quick locking without entering the full Argon2id-derived passphrase.

---

## 9. What SimpleX Could Learn from M2M

1. **Direct LAN connections**: SimpleX routes all traffic through relays, even on the same LAN. Adding mDNS discovery + direct TCP for same-subnet peers would improve latency and reduce relay load for local communication.

2. **Streaming file transfers**: SimpleX's XFTP uses separate file relay servers with chunk-based transfer. M2M's approach of streaming encrypted chunks within the existing session avoids the complexity of a separate protocol and separate relay infrastructure.

3. **Lock-free rate limiting**: M2M's `DashMap`-based per-IP rate limiter eliminates the global mutex bottleneck. SimpleX's SMP relay could benefit from this pattern for connection handling.

4. **Skipped message key cache**: The Signal-ratified 2000-entry max is well-documented in M2M's implementation. SimpleX may have this already (it uses the Double Ratchet), but the explicit cap + clear-on-DH-ratchet behavior is worth documenting.

---

## 10. Summary Assessment

### M2M's Unique Strengths
- **True P2P**: Zero infrastructure dependency. Install and go.
- **NAT traversal excellence**: 7 parallel strategies, best-in-class for a P2P messenger.
- **Streaming file transfers**: Never buffers a full file in RAM — important for constrained devices.
- **Single codebase**: Rust + React works on Windows, Mac, Linux via Tauri.
- **Lock-free concurrency**: DashMap rate limiting and RwLock-based state management.
- **Rust ecosystem**: Memory safety, strong typing, excellent async.

### SimpleX's Unique Strengths
- **Metadata-free**: No identifiers at all — the strongest privacy model of any messenger.
- **Post-quantum readiness**: PQ ratchet on every step.
- **Audited**: Two Trail of Bits audits (implementation + cryptographic design).
- **Mature feature set**: Groups, calls, reactions, editing, voice/video, bots.
- **Multi-platform**: Native iOS, Android, Desktop, CLI, bot SDK.
- **Private message routing**: Default concealment of sender IP.

### Fundamental Trade-Off

```
M2M:   P2P (no infra)  ↔  IP visible to peer  ↔  best NAT traversal
SimpleX:  Relays required  ↔  IP hidden from peer  ↔  no NAT issues
```

These are architectural commitments that cannot be bridged by incremental changes. M2M will always have the IP-leakage problem because P2P requires addressability. SimpleX will always require relay infrastructure because its queue model needs a rendezvous point.

The right question is not "which is better" but **which threat model do you care about**:
- **M2M** protects against: centralized server surveillance, infrastructure seizure, metadata collection by a server operator.
- **SimpleX** protects against: all of the above **plus** metadata correlation by a global adversary, peer IP leakage, and quantum decryption of past traffic.

---

*End of comparison*
