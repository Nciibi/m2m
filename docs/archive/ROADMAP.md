# M2M: 9.3/10 → 10/10 Roadmap

**Strategy**: Everything below is completed. The project is at 9.3/10 overall.
See `docs/full_analysis.md` for the detailed status of every component.

---

## ✅ Completed: Documentation Overhaul (v1.9.5–1.9.8)

- `README.md` — Happy Eyeballs diagram, strategy priority table, NAT-PMP/PCP/UPnP ordering
- `docs/architecture.md` — 154→505 lines, full module map with dependency graph
- `docs/protocol-spec.md` — 82→314 lines, slowloris protection, handshake state machine
- `docs/threat-model.md` — v1.1, ICE-Lite, TCP hole punch, port mapping
- `docs/full_analysis.md` — structural refresh
- `docs/invite-format.md` — WireCandidate structure, ICE-Lite population order, Tor guard

---

## ✅ Completed: Split commands.rs (v2.0.3–2.1.1)

2258-line `commands.rs` → 8 focused modules:
- `mod.rs` (110) — shared types
- `util.rs` (169) — helpers, Argon2id, storage crypto
- `vault.rs` (265) — identity init, vault lock/unlock
- `chat.rs` (310) — send/load messages, conversations CRUD
- `files.rs` (169) — file transfer init, accept, reject, chunk send
- `network.rs` (1008) — invites, connect, receive loop
- `settings.rs` (198) — STUN, Tor, diagnostics
- `forwards.rs` (91) — manual port forwarding CRUD

**25 clippy warnings fixed** — `cargo clippy -- -D warnings` passes clean.

---

## ✅ Completed: Double Ratchet + X3DH (v2.5.0)

Signal-standard **Double Ratchet** with **X3DH** key agreement:

| Component | Location | Status |
|-----------|----------|--------|
| HKDF-SHA256 (RFC 5869) | `crypto.rs` — extract/expand/full | Verified |
| X25519IdentityKeypair | `crypto.rs` | Long-term X25519 for X3DH DH |
| X3DH engine | `crypto.rs` — initiate/respond | **Bug fixed** (was using IK_A instead of EK_A) |
| Double Ratchet | `crypto.rs` — ~200 lines | Chain derivation, DH ratchet |
| PrekeyBundle | `crypto.rs` — extracted from invite | IK + SPK + Sig + OPK |

New packet types: `X3DHHandshakeInit/Response/Complete` (0x04–0x06).
Backward-compatible: legacy EncryptedEnvelope path preserved via `#[serde(default)]`.

---

## ✅ Completed: TURN Relay (v2.3.4)

Lightweight TCP relay protocol for symmetric NAT fallback:

| File | Lines | Purpose |
|------|-------|---------|
| `relay.rs` | ~570 | Relay client: register, connect_via_relay, wait_for_bridge, frame I/O |
| `commands/relay.rs` | ~80 | Tauri commands for relay config |
| `examples/relay-server.rs` | ~400 | Standalone TCP relay server |

Integration: `WireCandidate` carries `relay_id`, Happy Eyeballs races relay as lowest-priority strategy.

---

## ✅ Completed: Phase 4 — Hardening & Testing

### 4a — Storage tests (22 tests)
`storage.rs`: KeyStore + MessageStore round-trips, errors, cascade delete, retention, limits.

### 4b — Identity tests (16 tests)
`identity.rs`: Invite creation, validation, expiry, tampered signature, version mismatch, base64 errors.

### 4c — Fuzz harness
`fuzz/` with 2 targets: `frame_parse` (protocol parsing) and `padding` (unpadding invariants).

### 4d — Memory hardening
`secure_key.rs` (`StorageKey`): mlock/VirtualLock on storage encryption key, zeroized on drop.
Integrated into `state.rs`, `vault.rs`, `chat.rs`, `network.rs`, `util.rs`.

### 4e — CSP hardening
✅ Already done — `'self'` only, no Google Fonts, no `'unsafe-inline'`.

---

## ✅ Completed: Phase 5 — Frontend Lift

### 5a — TypeScript strict mode
✅ Already done — `"strict": true` in `tsconfig.json`.

### 5b — M2MContext
`src/context/M2MContext.tsx` — React context eliminates prop drilling.
All 5 views use `useM2M()` hook directly.

### 5c — Component tests (vitest)
Infrastructure set up: `vite.config.ts` test config, `src/__tests__/setup.ts`, 7 VaultView tests.
Needs `pnpm install` to run.

### 5d — UI polish
- **ErrorBoundary** per-view component
- **ShortcutHelp** modal (? key)
- **LoadingSpinner** with overlay mode
- **Toasts** for all async operations
- **Online/offline indicator** in HubView
- **Aria labels** on icon-only buttons
- **Dark/light theme** auto-detection
- **Strength meter** on vault passphrase

---

## ✅ Completed: Phase 6 — Protocol Polish

### 6a — Protocol v0x02
`PROTOCOL_VERSION = 0x02`. Legacy `0x01` accepted with deprecation log.
All version checks use `validate_version()` instead of strict equality.

### 6b — Entropy estimation upgrade
Pattern-based penalties (sequential, repeating, keyboard, substitution, short length)
+ NIST SP 800-63B floor. Both backend (`commands/util.rs`) and frontend (`src/utils.ts`).

### 6c — Connection keepalive
Background heartbeat task per connection (30s interval). Heartbeat failure → disconnect.
No `#[allow(dead_code)]` on heartbeat constants.

---

## ✅ Completed: Phase 4 — Message Features (Reactions, Edit, Delete, Self-Destruct, Markdown)

### 4.1 — Message Reactions
Emoji reactions on messages with encrypted typed frames (0x41):
- Backend: `send_reaction`, `remove_reaction` commands, `reactions` table, `m2m://reaction` events
- Frontend: Emoji picker on hover, reaction badges under messages, toggling on/off
- Wire format: `MessageReactionData { message_id, reaction (emoji), remove (bool) }`

### 4.2 — Read Receipts
Track when received messages have been viewed:
- Backend: `mark_messages_read` command, `read_at` column on messages table
- Frontend: "✓✓" indicator on read messages, auto-mark-read after 1s delay

### 4.3 — Self-Destruct Timer
Messages with optional auto-delete countdown:
- Backend: `disappear_after` field in `MessageBody::Text`, `expires_at` column, `cleanup_expired_messages`, periodic pruner
- Frontend: Timer selector (5s–24h), 🔥 countdown display, auto-removal when expired
- Wire format: `Text { id, content, disappear_after: Option<u64> }`

### 4.4 — Message Edit & Delete
Full edit/delete lifecycle:
- Packet types 0x42 (MessageEdit) and 0x43 (MessageDelete)
- Backend: `edit_message`, `delete_message` commands + network handlers + `m2m://edit`/`m2m://delete` events
- Frontend: Right-click context menu, inline edit mode, "edited" badge, "Message deleted" placeholder

### 4.5 — Rich Text / Markdown
Message rendering in ChatView:
- Bold (`**text**` / `__text__`), italic (`*text*` / `_text_`), inline code (`` `code` ``), clickable link detection
- Content inside backticks is rendered raw (no markdown parsing)

---

## ✅ Completed: Phase 7 — Docs, Tests & Polish

### X3DH Bug Fix (2026-06-28)
**Critical fix**: `x3dh_initiate()` was using `our_identity` (IK_A) for all DH operations
instead of `our_ephemeral` (EK_A) for DH2 and DH3. The ephemeral key was completely unused.
Clippy warning `unused variable: our_ephemeral` was the only sign. Now fixed.

### 22 new crypto tests
HKDF unit tests, X3DH same-output + wrong-key tests, Double Ratchet encrypt/decrypt
round-trip with gap and ratchet scenarios.

### Production `.unwrap()` fix
`stun.rs:262` — `max_by_key().unwrap()` → safe default.

### Documentation refresh
`docs/full_analysis.md` and `docs/threat-model.md` fully updated.

### CI/CD
Added `pnpm test`, switched `cargo audit` to official `rustsec/audit-check@v2`.

---

## Current Scores

| Category | Score |
|----------|-------|
| Architecture & Design | 9.5 / 10 |
| Security & Cryptography | 10 / 10 |
| Networking & Privacy | 10 / 10 |
| Test Coverage | 9.5 / 10 |
| Documentation | 9.0 / 10 |
| UI/UX | 8.5 / 10 |
| Performance | 8.5 / 10 |
| Code Quality | 9.5 / 10 |
| Maintainability | 9.5 / 10 |
| **Overall** | **9.3 / 10** |

## Remaining Ideas (Future)

| Idea | Impact | Effort |
|------|--------|--------|
| Component tests: run `pnpm test` (needs dep install) | Medium | Low |
| System tray icon | Low | Medium |
| Multi-device / account sync | High | Very High |
| Mobile app (Tauri mobile) | High | Very High |
| Push notifications for disconnected peers | Medium | High |
| Binary size optimization | Low | Low |
| Integration/E2E tests with full handshake | Medium | Medium |
