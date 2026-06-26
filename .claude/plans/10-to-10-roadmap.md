# M2M: 7.9 → 10/10 Roadmap

**Strategy**: Backend-first, high-impact changes. ~4-6 weeks total.
Each phase is independently shippable. Within a phase, order matters.

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

## Phase 2 — Split `commands.rs` (Week 3)

`commands.rs` is 2048 lines — the single biggest maintainability problem.
It handles identity, vault, chat, files, STUN, Tor, settings, and conversation
management. Every change to any command touches this file.

### New file structure

```
src-tauri/src/
  commands/
    mod.rs        — re-exports all command modules, defines shared response types
    vault.rs      — unlock_vault, get_vault_status, init_identity
    chat.rs       — send_message, load_messages, list_conversations
    files.rs      — send_file, accept_file_transfer, reject_file_transfer
    network.rs    — start_listening, connect_to_peer, disconnect_peer
    settings.rs   — set_stun_servers, set_tor_enabled, set_private_mode
    diagnostics.rs— discover_public_ip, check_connectivity, get_network_diagnostics
```

### Changes

1. Move `IdentityInfo`, `ConnectionInfo`, `ChatMessage`, `InviteInfo`,
   `FileTransferInfo` to `commands/mod.rs` (shared types)
2. Move event types (`MessageEvent`, `ConnectionEvent`, `FileRequestEvent`)
   to `commands/mod.rs` or `src/events.rs`
3. Each command module gets its own `decode_peer_key`, `resolve_local_ip` helpers
   (or extract to `src/util.rs`)
4. `lib.rs` imports from `commands::*` instead of `commands`

### Dead code cleanup

While splitting, fix:
- Unused imports (`PathBuf` in `commands.rs`, `ConnectionState` import not used)
- `candidate.rs` and `stun.rs` both have `gather_host_candidates()` — deduplicate
- Remove `#[allow(dead_code)]` annotations that no longer apply
- Remove unused `SessionKeyContext` constant in `crypto.rs`
- Remove unused `RESERVED_VERSIONS` in `protocol.rs`

### Tests

- Verify every command module compiles independently
- Ensure `lib.rs` compiles without warnings (clippy -D)
- CI: `cargo clippy -- -D warnings` added to workflow

### Impact
| Before | After |
|---|---|
| 2048-line monolith | 7 focused files, each <400 lines |
| `#[allow(dead_code)]` littered | Clean clippy pass |
| Duplicate candidate gathering | Single source of truth |

---

## Phase 3 — TURN Relay (Week 3–4)

M2M currently has STUN but **no TURN relay**. Users behind symmetric NATs
(common in corporate networks, mobile hotspots, some home routers) cannot
receive inbound connections. The STUN module already detects symmetric NAT
and warns the user — but can't help them.

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
| `candidate.rs` | Add `CandidateType::Relay` (already defined, currently unused) — populate from relay addresses |
| `protocol.rs` | Add relay address to `WireCandidate` |
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

### 4c — Fuzz harness for protocol parsing

Add `src-tauri/fuzz/` with a cargo-fuzz target:

1. `fuzz_targets/frame_parse.rs` — feeds random byte sequences into `read_frame_impl`
2. Run for 1M+ iterations, check for panics, OOMs, excessive CPU
3. Also fuzz `unpad_message_variable` with random inputs

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

| Phase | Score improvement | Time | Dependencies |
|---|---|---|---|
| 1 — Double Ratchet + X3DH | Security: 9.0→9.8, Innovation: 7.5→9.0 | 2 weeks | None |
| 2 — Split commands.rs | Code Quality: 8.0→9.5, Maintainability: 7.5→9.5 | 1 week | None |
| 3 — TURN relay | Completeness: 7.5→9.5 | 1 week | Phase 2 (cleaner surface) |
| 4 — Hardening & Testing | Testing: 8.0→9.5, Security: 9.8→10 | 1 week | Phases 1, 3 (tests new code) |
| 5 — Frontend lift | UI/UX: 6.5→8.5 | 1.5 weeks | None |
| 6 — Protocol polish | All categories +0.2–0.5 | 1 week | Phase 1 (version bump) |

**Projected final scores after all phases:**

| Category | Current | Target |
|---|---|---|
| Architecture & Design | 8.5 | 9.5 |
| Security | 9.0 | 10 |
| Code Quality | 8.0 | 9.5 |
| Testing | 8.0 | 9.5 |
| Documentation | 9.0 | 9.5 |
| UI/UX | 6.5 | 8.5 |
| Performance | 7.5 | 8.5 |
| Completeness | 7.5 | 9.5 |
| Maintainability | 7.5 | 9.5 |
| Innovation | 7.5 | 9.0 |
| **Overall** | **7.9** | **9.3–9.5** |

The Double Ratchet + X3DH, commands.rs refactor, and TURN relay are the
three highest-leverage changes. Phases 1–3 alone take the project to
~9.0/10. Phases 4–6 add the polish layer to push toward 9.5+.
