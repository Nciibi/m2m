# M2M → 10/10: Master Upgrade Roadmap

**Current Score: 8.0/10**
- Architecture: 10 | Security/Crypto: 10 | Networking/Privacy: 10
- Test Coverage: 9.5 | UI/UX: 6.0 | Performance: 10

**Target: True 10/10 — a production-ready, fully decentralized P2P messenger**

---

## Architecture Overview (Target State)

```
                  ┌──────────────────────────────────────────────┐
                  │              M2M Node (you)                  │
                  │                                                │
                  │  ┌─────────┐  ┌──────────┐  ┌─────────────┐  │
                  │  │ Identity │  │ DHT Peer │  │ Connection  │  │
                  │  │ Manager │  │ Discovery│  │  Manager    │  │
                  │  └────┬────┘  └────┬─────┘  └──────┬──────┘  │
                  │       │            │               │         │
                  │  ┌────▼────────────▼───────────────▼──────┐  │
                  │  │           Session Layer                 │  │
                  │  │  (X3DH + Double Ratchet + Messaging)    │  │
                  │  └───────────────────┬─────────────────────┘  │
                  │                      │                       │
                  │  ┌───────────────────▼─────────────────────┐  │
                  │  │         Transport Layer                  │  │
                  │  │  TCP/Tor/Relay  •  LAN Broadcast         │  │
                  │  │  NAT Traversal  •  Port Mapping          │  │
                  │  └───────────────────┬─────────────────────┘  │
                  └──────────────────────┼───────────────────────┘
                                         │
          ┌──────────────┬───────────────┼───────────────┬───────────────┐
          │              │               │               │               │
     ┌────▼────┐   ┌────▼────┐    ┌─────▼─────┐   ┌────▼────┐   ┌────▼────┐
     │  Peer A │   │  Peer B │    │  Peer C   │   │  DHT    │   │  LAN    │
     │ (device)│   │(device) │    │ (device)  │   │ Bootstrap│  │Broadcast│
     └─────────┘   └─────────┘    └───────────┘   └──────────┘   └─────────┘
```

---

## ✅ Phase 1: Fully Decentralized Peer Discovery — COMPLETE

| Sub-phase | File | Status |
|-----------|------|--------|
| 1.1 Kademlia DHT | `src-tauri/src/dht.rs` | ✅ Custom Kademlia-style DHT — ephemeral peer IDs, announce/lookup/bootstrap, NAT awareness |
| 1.2 LAN Discovery | `src-tauri/src/lan_discovery.rs` | ✅ UDP multicast on `239.255.27.3:38553`, 30s announce interval, ephemeral session tokens |
| 1.3 Relay Server | `src-tauri/examples/relay-server.rs` | ✅ Standalone relay with `docker-compose.yml` |
| Off by default | — | ✅ Both `dht_enabled` and `lan_enabled` default to `false` per privacy-first principle |

---

## ⚠️ Phase 2: Multi-Device & Identity Sync — PARTIAL (1/3 done)

**Problem**: Identity is locked to one device. No way to use the same key on multiple machines or recover from device loss.

### 2.1 — Identity Export/Import with Encrypted Backup — ✅ DONE

**Implemented**: `src-tauri/src/commands/vault.rs` (lines ~523–640+)
- `export_identity` with passphrase + Argon2id wrapping key (min 12 chars, 40+ bits entropy)
- `import_identity` to restore from encrypted JSON
- Family contacts: `list_family`, `add_family_member`, `remove_family_member`, `set_family_nickname`, `connect_family_member`, `update_family_member`

### 2.2 — Encrypted Sync Layer (P2P Message Sync) — ❌ NOT STARTED

**Planned file**: `src-tauri/src/sync.rs` (350 lines)
- When two devices share the same identity, they can sync via encrypted P2P channel
- **Sync protocol**:
  1. Bootstrap device creates a "sync invite" (one-time, high-entropy token)
  2. Secondary device connects using direct TCP or relay
  3. X3DH handshake between the two devices (not identity-based, session-based)
  4. Bi-directional sync of:
     - Conversation list (metadata only, not messages — messages stay on original device)
     - Peer keys (so second device knows how to connect)
     - Unread message count
  5. Messages are NOT synced by default — they stay on the device that received them
     - Optional: "sync messages" toggle that mirrors encrypted blobs

### 2.3 — Read-Only Web Companion (STRETCH) — ❌ NOT STARTED

---

## ❌ Phase 3: Group Chat & Multi-Peer Sessions — NOT STARTED

**Problem**: Currently strictly 1:1 sessions. No group conversations.

### 3.1 — Sender Keys for Group E2EE — ❌ NOT STARTED

**Planned**: Sender Key distribution via existing 1:1 DR sessions, group key ratchet, member add/remove protocol.

### 3.2 — Frontend: Group Chat UI — ❌ NOT STARTED

### 3.3 — Frontend: Group Management — ❌ NOT STARTED

---

## ✅ Phase 4: Message Features — Reactions, Edit, Delete, Self-Destruct, Markdown (COMPLETE)

| Sub-phase | What | Status |
|-----------|------|--------|
| 4.1 | Reactions | ✅ Packet 0x41, emoji picker, reaction badges |
| 4.2 | Read receipts | ✅ `read_at` column, ✓✓ indicator, auto-mark-read |
| 4.3 | Self-destruct timer | ✅ `disappear_after` in MessageBody, countdown UI, cleanup |
| 4.4 | Message edit & delete | ✅ Packets 0x42/0x43, context menu, inline edit, soft-delete |
| 4.5 | Rich text / Markdown | ✅ Bold, italic, code, link detection in ChatView |

---

## ❌ Phase 5: Frontend Overhaul — MOSTLY MISSING (1/8 done)

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 5.1 Typing indicators | ❌ | No packet type 0x45, no frontend UI |
| 5.2 Local message search | ❌ | Conversation-list filter exists in HubView, but no per-conversation full-text search |
| 5.3 Drag-and-drop file transfer | ❌ | No drag/drop handlers in ChatView |
| 5.4 Voice messages | ❌ | No audio capture, no playback |
| 5.5 Conversation organization | ❌ | No favorites, mute, archive, or folders — no fields in `types.ts` |
| 5.6 Theme & color customization | ⚠️ | Light theme CSS exists. AppContext sets `data-theme="dark"` but **no `[data-theme="dark"]` CSS rules**. No accent picker. |
| 5.7 Keyboard navigation | ⚠️ | Partial: `Esc` (back to hub), `Ctrl+,` (settings), `?` (help). Missing: Ctrl+N, Ctrl+K, Ctrl+F, etc. |
| 5.8 Dark mode refinements | ❌ | No dark theme CSS at all |

---

## ✅ Phase 6: Performance & Reliability — COMPLETE

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 6.1 Connection reconnection | ✅ | `reconnect.rs` — exponential backoff (1s→30s cap), 5 max attempts, frontend "Reconnecting…" badge |
| 6.2 Message de-duplication & ordering | ✅ | DB-level idempotent store by message_id. Sender-side offline queue + reconnect missed-message request implemented |
| 6.3 Database performance | ✅ | WAL mode on all stores, composite indexes (`idx_messages_conversation`, `idx_messages_expires_at`, etc.) |
| 6.4 Benchmarks | ✅ | `crypto_bench.rs` with criterion for DR encrypt/decrypt |
| 6.5 Startup time optimization | ✅ | Lazy vault init, lazy candidate gathering (no STUN scan unless listening), deferred DHT bootstrap |

---

## ❌ Phase 7: Notifications & Background Mode — MOSTLY MISSING

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 7.1 Native notifications | ⚠️ | `tauri-plugin-notification` is a dependency but no action buttons, notification grouping, or content preferences implemented |
| 7.2 Background keep-alive | ❌ | No system tray icon, no minimize-to-tray, no `run_on_close` behavior |
| 7.3 System tray integration | ❌ | No `TrayIcon` or tray menu anywhere |

---

## ✅ Phase 8: Security Hardening — COMPLETE

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 8.1 Screen capture protection | ✅ | `window_security.rs` — Windows FFI `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`, macOS/Linux stubs. OFF by default. |
| 8.2 Clipboard auto-clear | ✅ | `commands/security.rs` — `clear_clipboard` command, configurable timeout (default 0 = disabled). |
| 8.3 Lock on idle/sleep | ✅ | `useIdleDetection` hook, `SecurityConfig.idle_lock_secs` (default 0 = disabled), auto-lock vault. |

### Ongoing security maintenance:
- Regular cargo-audit + pnpm-audit in CI (already done)
- Fuzz testing: expand existing fuzz targets (`frame_parse`, `padding`)
- Add fuzz target for DR message handling
- Add fuzz target for X3DH handshake with malformed bundles
- Formal verification of KDF ratchet (property-based testing with `proptest`)
- Security audit document update: keep `docs/threat-model.md` current

---

## ⚠️ Phase 9: Documentation & Onboarding — PARTIAL (1/4 done)

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 9.1 In-app onboarding tutorial | ❌ | `SetupView.tsx` is just a loading spinner — no interactive walkthrough |
| 9.2 User-facing documentation | ⚠️ | `docs/user-guide.md` not created, but `docs/beginners-guide.md`, `docs/architecture.md`, and related docs exist |
| 9.3 Threat model | ✅ | `docs/threat-model.md` exists and is maintained |
| 9.4 API documentation | ⚠️ | Partial inline docs on public functions; module-level docs in key files |

---

## ❌ Phase 10: Platform Polish & Distribution — NOT STARTED

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 10.1 Code signing & notarization | ❌ | Not configured |
| 10.2 Auto-update infrastructure | ❌ | `tauri-plugin-updater` not in dependencies, no updater config |
| 10.3 Installer polish | ❌ | No MSI/DMG/AppImage configs beyond defaults |

---

## Summary: Scores After Each Phase

| Phase | Content | Architecture | Security | Networking | Tests | UI/UX | Perf | Overall |
|-------|---------|:-----------:|:--------:|:----------:|:----:|:----:|:----:|:-------:|
| Now | Current | 10 | 10 | 10 | 9.5 | 6.0 | 10 | **8.0** |
| 1 | DHT + LAN discovery | 10 | 10 | 10 | 9.5 | 8.5 | 8.5 | 9.4 |
| 2 | Multi-device sync | 10 | 10 | 10 | 9.5 | 8.5 | 8.5 | 9.4 |
| 3 | Group chat | 10 | 10 | 10 | 9.5 | 8.5 | 8.5 | 9.4 |
| 4 | Message features | 10 | 10 | 10 | 9.5 | 9.0 | 8.5 | 9.5 |
| 5 | Frontend overhaul | 10 | 10 | 10 | 9.5 | 10 | 8.5 | 9.7 |
| 6 | Performance | 10 | 10 | 10 | 9.5 | 10 | 10 | 9.8 |
| 7 | Notifications | 10 | 10 | 10 | 9.5 | 10 | 10 | 9.8 |
| 8 | Security hardening | 10 | 10 | 10 | 9.5 | 10 | 10 | 9.9 |
| 9 | Documentation | 10 | 10 | 10 | 10 | 10 | 10 | **10** |
| 10 | Distribution | 10 | 10 | 10 | 10 | 10 | 10 | **10** |

---

## File Change Summary

| File | Action | Est. Lines | Status |
|------|--------|:----------:|:------:|
| `src-tauri/src/dht.rs` | **NEW** | 400 | ✅ Done |
| `src-tauri/src/lan_discovery.rs` | **NEW** | 200 | ✅ Done |
| `src-tauri/src/sync.rs` | **NEW** | 350 | ❌ Pending |
| `src-tauri/src/group.rs` | **NEW** | 300 | ❌ Pending |
| `src-tauri/src/crypto.rs` | Modify (+ sender keys) | +250 | ❌ Pending |
| `src-tauri/src/protocol.rs` | Modify (+ SyncRequest 0x44, + typing indicator) | +80 | ⚠️ Partial |
| `src-tauri/src/session.rs` | Modify (+ reconnect) | +100 | ✅ Done |
| `src-tauri/src/state.rs` | Modify (+ groups, sync) | +50 | ❌ Pending |
| `src-tauri/src/storage.rs` | Modify (+ WAL, indexes, offline queue, sync queries) | +120 | ✅ Done |
| `src-tauri/src/commands/chat.rs` | Modify (+ search, reactions, offline queue, flush) | +160 | ⚠️ Partially done |
| `src-tauri/src/commands/network.rs` | Modify (+ reconnect) | +60 | ✅ Done |
| `src-tauri/src/commands/vault.rs` | Modify (+ export/import) | +150 | ✅ Done |
| `src-tauri/src/main.rs` | Modify (+ tray, background) | +100 | ❌ Pending |
| `src-tauri/src/reconnect.rs` | **NEW** | ~80 | ✅ Done |
| `src-tauri/src/window_security.rs` | **NEW** | ~100 | ✅ Done |
| `src-tauri/Cargo.toml` | Modify (+ deps) | +15 | ⚠️ Partial |
| `src/views/ChatView.tsx` | Modify (major) | +400 | ⚠️ Partial |
| `src/views/HubView.tsx` | Modify (major) | +200 | ❌ Pending |
| `src/views/SetupView.tsx` | Modify (onboarding) | +100 | ❌ Pending |
| `src/views/VaultView.tsx` | Modify (UX polish) | +50 | ❌ Pending |
| `src/App.tsx` | Modify | +30 | ❌ Pending |
| `src/styles/` | Modify (themes) | +200 | ❌ Pending |
| `src/types.ts` | Modify | +30 | ❌ Pending |
| `docs/` | Various updates | +200 | ⚠️ Partial |

---

## Dependency Additions

```toml
# DHT-based peer discovery — ✅ Implemented custom Kademlia (no external crate needed)
# Multi-device sync (QR encoding)
# qrcode = "0.14"                ❌ Pending
# Benchmarking — ✅ Already added
criterion = { version = "0.5", optional = true }
# Audio capture (voice messages)
# cpal = "0.15"                  ❌ Pending
# System tray
# tauri-plugin-tray = "2"        ❌ Pending
# Updater
# tauri-plugin-updater = "2"     ❌ Pending
# Notifications — ✅ Already added
tauri-plugin-notification = "2"
```

---

## Execution Priority (What to Build Next)

**Tier 1 — Core missing features (highest user impact)**:
1. Group chat (Phase 3) — biggest missing feature vs. Signal/WhatsApp
2. Frontend overhaul (Phase 5) — biggest visible quality gap

**Tier 2 — Feature complete**:
4. Multi-device sync (Phase 2) — significant engineering effort, moderate user demand
5. Notifications + system tray (Phase 7) — background UX
6. Typing indicators + message search (Phase 5.1–5.2) — medium effort, high UX value

**Tier 3 — Power user features**:
7. Voice messages (Phase 5.4) — moderate effort, high polish value
8. Conversation organization (Phase 5.5) — favorites, mute, archive, folders
9. Dark theme + keyboard shortcuts (Phase 5.6–5.8)

**Tier 4 — Platform & maintenance**:
10. Auto-update + code signing (Phase 10)
11. Onboarding tutorial (Phase 9.1)
12. API documentation pass (Phase 9.4)
