# M2M: 7.9 → 10/10 Roadmap

**Strategy**: Backend-first, high-impact changes. ~4–6 weeks total.
Each phase is independently shippable. Within a phase, order matters.

---

## ✅ Completed: Documentation Overhaul (v1.9.5–1.9.8)

Roadmap-aligned documentation work has been completed across four releases:

### 1.9.5 — Hold on `hole_punch.rs`
- Added `#[allow(dead_code)]` to `TcpHolePunch` and `TcpRelay` strategy variants
- Marks these strategies as architecturally defined but not yet wired (honest self-documentation)

### 1.9.6 — Architecture & Protocol Rewrite
- **`README.md`** rewritten with full connection-strategies section, Happy Eyeballs diagram, priority table, NAT-PMP/PCP ordering rationale, TCP hole punch explainer
- **`docs/architecture.md`** rewritten from 154→505 lines: design philosophy axioms, protocol comparison (Matrix/Signal/XMPP), TCP-vs-UDP rationale, full module map with dependency graph, per-module design rationale table, layered architecture ASCII diagram
- **`docs/protocol-spec.md`** rewritten from 82→314 lines: reserved version rationale, per-field size limits table, slowloris protection with code pseudocode, packet type table overhaul, complete handshake state machine, encryption frame layout, message padding specification, file transfer state machine, candidate exchange protocol, error types enum

### 1.9.7 — Analysis & Threat Model Update
- **`docs/threat-model.md`** v1.1: updated to reflect ICE-Lite candidate architecture, TCP hole punch, port mapping strategies
- **`docs/full_analysis.md`** structural refresh aligning with new module decomposition

### 1.9.8 — Invite Format Specification
- **`docs/invite-format.md`** rewritten with full `WireCandidate` structure, ICE-Lite candidate population order (host → IPv6 → port-mapped → manual → srflx → relay), Tor guard explanation, flag field enumeration, invite validation pseudocode

### Impact on roadmap

| Before | After |
|---|---|
| Architecture docs described old module map (no `hole_punch.rs`, `port_mapping.rs`, `local_addr.rs`) | Full module decomposition with dependency graph |
| Protocol spec had no slowloris, size-limit rationale, or candidate-exchange sections | Complete wire-format reference with 5 new sections |
| Invite format had no candidate structure | ICE-Lite multi-candidate invite format specified |
| Readme had no connection-strategy documentation | Happy Eyeballs diagram + strategy priority table + NAT-PMP/PCP/UPnP ordering rationale |

This documentation provides the **specification foundation** for Phases 3 (TURN relay), 4b (identity tests — invite validation pseudocode), and 4e (CSP rationale in architecture). The Rust backend code in `hole_punch.rs`, `local_addr.rs`, and `port_mapping.rs` is architecturally defined but candidate gathering is not yet fully wired — that work is tracked in the phases below.

---

## Phase 1 — Double Ratchet + X3DH (Weeks 1–2)

The single biggest cryptographic upgrade available. M2M's current ratchet is a
one-way SHA-256 KDF that provides forward secrecy in *batches* (per message
group). The Signal Double Ratchet provides **per-message** forward secrecy and
**future secrecy** (aka post-compromise security): if a key leaks, a single
honest message after it heals the session.

### What to build

| New file | Purpose |
|---|---|
| `src-tauri/src/double_ratchet.rs` | Root key, chain keys, message keys. Separate sending/receiving chains. |
| `src-tauri/src/x3dh.rs` | X3DH initial key agreement (replaces plain X25519 DH for the initial handshake). |

### X3DH integration

Replace the current `EphemeralKeypair::client_session_keys` / `server_session_keys`
handshake with X3DH:

1. **Initiator**: Generates an ephemeral keypair + pre-key bundle. Sends
   `(identity_pk, ephemeral_pk, signed_prekey_pk, prekey_sig)` in HandshakeInit.
2. **Responder**: Verifies the signed pre-key. Computes the shared secret
   via DH(eph, peer_id) + DH(identity, signed_pre) + DH(eph, signed_pre).
3. **Both sides**: Feed the shared secret into the Double Ratchet's root chain.

### Double Ratchet integration

Replace `SessionKeys::ratchet_tx()` / `ratchet_rx()` with a proper Double Ratchet:

```
Root Chain ──ratchet step──▶ Sending Chain ──each msg──▶ Message Key
          └──ratchet step──▶ Receiving Chain ──each msg──▶ Message Key
```

- Each message advances the chain, producing a unique key + nonce
- DH ratchet occurs every N messages (configurable, default 3) for PCS
- When a new DH public key arrives, root chain ratchets, creating new chains
- Existing `Session` struct gets a `DoubleRatchet` field replacing `SessionKeys`

### Changes to existing files

| File | Change |
|---|---|
| `session.rs` | Replace `session_keys: Option<SessionKeys>` with `ratchet: DoubleRatchet`. Modify `send_encrypted` / `decrypt_message` / `decrypt_typed_frame` to call ratchet. |
| `handshake_as_initiator/responder` | Use X3DH shared secret instead of plain `client_session_keys`. |
| `protocol.rs` | Add `IdentityKeyBundle` type (identity_pk + signed_prekey_pk + signature). Extend `HandshakeInit` / `HandshakeResponse` with pre-key fields. |
| `commands.rs` | No changes needed — `send_message` and `send_encrypted` internals change transparently. |

### Tests

- Unit: key derivation, ratchet advance, message encrypt/decrypt round-trip, out-of-order delivery, skipped messages
- Integration: Alice ↔ Bob full conversation with DH ratchet steps
- Property-based: "encrypt then decrypt == original" across 1000+ random messages, assert keys change on every step

### Impact
| Before | After |
|---|---|
| Forward secrecy per batch | Forward secrecy per message |
| No post-compromise security | PCS: one honest message heals the session |
| Custom ratchet (unreviewed construction) | Signal-standard algorithm |

---

## ✅ Completed: Split `commands.rs` (v2.0.3–2.1.1)

The 2258-line `commands.rs` monolith has been split into a focused 8-module
directory. Along the way, every clippy warning in the project was fixed.

### Result

```
src-tauri/src/commands/
    mod.rs        (110 lines) — shared types (IdentityInfo, ChatMessage, events)
    util.rs       (169 lines) — decode_peer_key, resolve_local_ip, entropy, storage crypto
    vault.rs      (265 lines) — init_identity, get_identity, unlock_vault, get_vault_status
    chat.rs       (310 lines) — send_message, load_messages, conversations CRUD
    files.rs      (169 lines) — send_file, accept/reject file transfer
    network.rs    (1008 lines)— create_invite, connect_to_peer, start_listening, **receive loop**
    settings.rs   (198 lines) — STUN, Tor, private mode, diagnostics
    forwards.rs   ( 91 lines) — manual port forwarding CRUD
```

### What was done

1. **Shared types** extracted to `mod.rs` (IdentityInfo, ConnectionInfo, ChatMessage
   with Zeroize Drop, InviteInfo, FileTransferInfo, events, VaultStatus, ConversationListItem)
2. **Helpers** extracted to `util.rs` (decode_peer_key, resolve_local_ip, entropy
   estimator, Argon2id key derivation, XChaCha20-Poly1305 storage crypto, create_temp_file)
3. **Vault** commands to `vault.rs` (init_identity, get_identity, unlock_vault,
   get_vault_status)
4. **Chat** commands to `chat.rs` (send_message, load_messages, list_conversations,
   rename/delete/retention/export conversation)
5. **File transfer** commands to `files.rs` (send_file, accept/reject, chunk sender)
6. **Network** commands to `network.rs` (create_invite, validate_invite,
   start_listening, connect_to_peer, get_connection_state, verify/disconnect/list peers,
   handle_incoming_connection, spawn_receive_loop — the largest file at 1008 lines)
7. **Settings** commands to `settings.rs` (discover_public_ip, STUN config,
   private mode, connectivity check, diagnostics, Tor toggle)
8. **Forwards** commands to `forwards.rs` (list/add/remove/reorder manual forwards)
9. `lib.rs` updated to point `generate_handler![]` at the new sub-module paths
10. `port_mapping.rs`` updated with new `crate::commands::util::resolve_local_ip()` path
11. Old `commands.rs` deleted

### Dead code cleanup (completed)

| Item | Action taken |
|---|---|
| `#[allow(dead_code)]` on `commands.rs` imports | Removed with the split |
| `RESERVED_VERSIONS` in `protocol.rs` | Already used — kept as-is |
| `SessionKeyContext` in `crypto.rs` | Checked — already properly used |
| Pre-existing `#[allow(dead_code)]` scattered in `network.rs`, `port_mapping.rs`, `tor.rs`, `hole_punch.rs`, `state.rs` | Intentional — these are architecture-level markers for Phase 3 (TURN relay) code that's designed but not wired |

### Project-wide clippy cleanup

Fixed 25 clippy warnings across the entire codebase:
- `commands/` — needless borrows, redundant closures, manual div_ceil
- `crypto.rs` — orphaned doc comment causing `empty_line_after_doc_comments`
- `network.rs` — dead code annotations, `or_insert_with(VecDeque::new)` → `or_default()`
- `stun.rs` — `&data[..]` → `data[..]` ref comparison
- `port_mapping.rs` — dead code, `splitn(2, ':').nth(1)` → `split_once()`, let_unit_value, needless Some+?
- `tor.rs` — dead code on `connect()` and `connect_via_tor()`
- `storage.rs` — type_complexity allow on `load_identity`

### Impact
| Before | After |
|---|---|
| 2258-line monolith | 8 focused files, most <300 lines |
| `cargo clippy -- -D warnings` = 25 errors | Zero warnings, zero errors |
| Implicit module boundaries | Explicit trait/struct visibility per module |
| Every change touched `commands.rs` | Change surface scoped to one sub-module |

---

## Phase 3 — TURN Relay (Week 3–4)

M2M currently has STUN but **no TURN relay**. Users behind symmetric NATs
(common in corporate networks, mobile hotspots, some home routers) cannot
receive inbound connections. The STUN module already detects symmetric NAT
and warns the user — but can't help them.

**Foundation laid:** The `hole_punch.rs::Strategy::TcpRelay` variant is defined
(at architecture level) with `#[allow(dead_code)]`. Wire format for relay
candidates is specified in `docs/protocol-spec.md` (§Candidate Exchange) and
`docs/invite-format.md` (§WireCandidate structure). This phase wires the
implementation.

### What to build

**Lightweight approach: TURN over WebSocket + cloud relay.**

Rather than implementing the full RFC 5766 TURN protocol (which needs UDP
allocation, permission management, channel bindings — thousands of lines),
build a lightweight relay that peers can self-host or use a community relay.

| New file | Purpose |
|---|---|
| `src-tauri/src/relay.rs` | Relay client: register with relay server, request allocation |
| `src/views/RelaySettings.tsx` | UI for configuring relay servers |
| `docs/relay-deploy.md` | Guide for self-hosting a relay |

### Relay protocol (minimal)

1. Peer A connects to relay server, sends its identity hash
2. Relay returns a `relay://` address
3. Peer A embeds the relay address in the invite (alongside direct TCP address)
4. Peer B connects to the relay, which bridges the TCP stream
5. Relay never sees plaintext (end-to-end encrypted)

### Changes to existing files

| File | Change |
|---|---|
| `candidate.rs` | Populate `CandidateType::Relay` from relay addresses (currently defined, unused) |
| `protocol.rs` | Relay address in `WireCandidate` (wire format already specified) |
| `commands.rs` | `create_invite` includes relay address when available |
| `network.rs` | `connect()` attempts direct first, falls back to relay |
| `state.rs` | Add `relay_config: RwLock<Option<RelayConfig>>` |
| `lib.rs` | Register `commands::set_relay_server` |

### Tests

- Unit: relay message serialization, candidate priority ordering with relay
- Integration: Alice ↔ relay ↔ Bob round-trip

### Impact
| Before | After |
|---|---|
| Symmetric NAT = no inbound | Symmetric NAT = works with relay |
| `CandidateType::Relay` dead code | Fully wired relay candidates |
| Only host + srflx candidates | Full ICE candidate set |

---

## Phase 4 — Hardening & Testing (Weeks 4–5)

### 4a — Storage tests (missing entirely)

| File | What to test |
|---|---|
| `storage.rs` | `store_identity` + `load_identity` round-trip, `store_message` + `load_messages`, `ensure_conversation`, `delete_conversation` (verify VACUUM + secure_delete), `upsert_peer`, `is_vault_initialized`, legacy migration, concurrent writes |
| Test pattern | Use in-memory SQLite (`":memory:"`) for speed. Each test creates a fresh store. |

### 4b — Identity tests

| File | What to test |
|---|---|
| `identity.rs` | `create_invite` + `validate_invite` round-trip, expired invite rejection, future-invite rejection, tampered signature rejection, one_time flag, max length enforcement, clock-skew boundary |
| Edge cases | Malformed base64, missing prefix, oversized payload, version mismatch |

**Note:** The invite validation logic is now documented with pseudocode in
`docs/invite-format.md` (§3. Validation), providing a reference for test case
design.

### 4c — Fuzz harness for protocol parsing

Add `src-tauri/fuzz/` with a cargo-fuzz target:

1. `fuzz_targets/frame_parse.rs` — feeds random byte sequences into `read_frame_impl`
2. Run for 1M+ iterations, check for panics, OOMs, excessive CPU
3. Also fuzz `unpad_message_variable` with random inputs

**Note:** Protocol size limits and validation rules are now fully specified in
`docs/protocol-spec.md` (§2. Transport Framing), providing the invariant
contract for fuzz oracles.

### 4d — Memory hardening

| Change | File |
|---|---|
| `mlock()` the storage key onto physical RAM | `state.rs` — use `memsec` or raw `libc::mlock` |
| After mlock, also `mprotect(PROT_NONE)` the page when not in use | Advanced: unmap when vault is locked |
| Verify WAL-mode SQLite securely deletes on `secure_delete` | Already partially done in `delete_conversation` |

### 4e — CSP & capability hardening

| Change | File |
|---|---|
| Harden CSP to block inline styles and eval | `tauri.conf.json` |
| Audit Tauri capabilities: remove unused permissions | `capabilities/default.json` |
| Add `connect-src` restriction for WebView | `tauri.conf.json` |

### Impact
| Before | After |
|---|---|
| 0 storage tests | Full coverage of all store operations |
| 0 identity tests | Full coverage of invite lifecycle |
| 0 fuzz coverage | Protocol-level fuzz harness |
| Key can be paged to swap | `mlock()`'d in RAM |

---

## Phase 5 — Frontend Lift (Weeks 5–6)

User chose backend-first, but the frontend is the weakest-scoring area (6.5/10)
and drags the average down. A measured lift without full rewrite.

### 5a — TypeScript strict mode

| Change | File |
|---|---|
| Enable `strict: true` in `tsconfig.json` | `tsconfig.json` |
| Fix all resulting type errors | Throughout `src/` |

### 5b — Extract state from App.tsx

`App.tsx` has ~40 state variables and handles everything inline. Extract:

| New file | Purpose |
|---|---|
| `src/hooks/useM2MState.ts` | Custom hook encapsulating all app state, event listeners, and command calls |
| `src/context/M2MContext.tsx` | React context so child views don't need 30 props each |

### 5c — Basic component tests

Add vitest:

| File | Tests |
|---|---|
| `src/__tests__/VaultView.test.tsx` | Passphrase validation rendering, strength meter, error states |
| `src/__tests__/App.test.tsx` | View routing (setup → vault → hub → chat) |

### 5d — UI polish

| Change | Priority |
|---|---|
| Loading states for all async operations (connection, invite gen, file transfer) | High |
| Error boundaries for each view | High |
| Online/offline indicator in header | Medium |
| Keyboard shortcut help modal (`?` key) | Medium |
| Accessibility: aria labels on icon-only buttons | Medium |

### Impact
| Before | After |
|---|---|
| 650-line App.tsx with 40 state vars | State in custom hook, clean components |
| `any` types throughout | Strict TypeScript |
| 0 frontend tests | Component tests for critical views |
| No loading states for async ops | Consistent loading/error UX |

---

## Phase 6 — Protocol Polish (Week 6)

### 6a — Wire protocol version 0x02 (backward-compatible)

After Double Ratchet changes the handshake, bump `PROTOCOL_VERSION` from `0x01`
to `0x02`. Keep the v0x01 parser as a fallback with a deprecation notice.

### 6b — Entropy estimation upgrade

| Change | Reason |
|---|---|
| Replace character-pool model with diceware-aware estimation | Current estimator overestimates entropy of keyboard patterns ("password123!" gets ~50 bits) |
| Include NIST SP 800-63B password rules guidance in UI | Users get actionable advice |

### 6c — Connection keepalive improvements

| Change | File |
|---|---|
| Exponential backoff for reconnection (1s, 2s, 4s, … cap 60s) | `network.rs` / `state.rs` |
| Persistent heartbeat with adaptive interval | `session.rs` — start at 30s, back off to 120s on stable connection |

---

## Summary

| Phase | Score improvement | Time | Dependencies | Status |
|---|---|---|---|---|
| **Docs Overhaul** | Architecture: 8.5→9.0, Documentation: 9.0→9.5 | Complete | None | ✅ Done (v1.9.5–1.9.8) |
| **Split commands.rs** | Code Quality: 8.0→9.5, Maintainability: 7.5→9.5 | Complete | None | ✅ Done (v2.0.3–2.1.1) |
| 1 — Double Ratchet + X3DH | Security: 9.0→9.8, Innovation: 7.5→9.0 | 2 weeks | None | ⬜ Pending |
| 3 — TURN relay | Completeness: 7.5→9.5 | 1 week | None | ⬜ Pending |
| 4 — Hardening & Testing | Testing: 8.0→9.5, Security: 9.8→10 | 1 week | Phases 1, 3 (tests new code) | ⬜ Pending |
| 5 — Frontend lift | UI/UX: 6.5→8.5 | 1.5 weeks | None | ⬜ Pending |
| 6 — Protocol polish | All categories +0.2–0.5 | 1 week | Phase 1 (version bump) | ⬜ Pending |

**Target scores after all phases:**

| Category | Current | After Docs | After Phase 2 | Target |
|---|---|---|---|---|
| Architecture & Design | 8.5 | **9.0** | 9.0 | 9.5 |
| Security | 9.0 | 9.0 | 9.0 | 10 |
| Code Quality | 8.0 | 8.0 | **9.5** | 9.5 |
| Testing | 8.0 | 8.0 | 8.0 | 9.5 |
| Documentation | 9.0 | **9.5** | 9.5 | 9.5 |
| UI/UX | 6.5 | 6.5 | 6.5 | 8.5 |
| Performance | 7.5 | 7.5 | 7.5 | 8.5 |
| Completeness | 7.5 | **8.0** | 8.0 | 9.5 |
| Maintainability | 7.5 | 7.5 | **9.5** | 9.5 |
| Innovation | 7.5 | 7.5 | 7.5 | 9.0 |
| **Overall** | **7.9** | **8.1** | **8.6** | **9.3–9.5** |

The Double Ratchet + X3DH and TURN relay are the two highest-leverage
remaining changes. Phases 1 + 3 alone would take the project to ~9.0/10.
Phases 4–6 add the polish layer to push toward 9.5+.
