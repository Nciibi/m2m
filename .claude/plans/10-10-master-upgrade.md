# M2M → 10/10: Master Upgrade Roadmap

**Current Score: 8.0/10**
- Architecture: 10 | Security/Crypto: 10 | Networking/Privacy: 10
- Test Coverage: 9.5 | UI/UX: 6.0 | Performance: 9.5

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

**Planned file**: `src-tauri/src/crypto.rs` (add ~250 lines)
...
...
...

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

## Phase 6: Performance & Reliability (Performance: 8.5 → 10)

### 6.1 — Connection Reconnection Logic

**Modified**: `src-tauri/src/commands/network.rs` — auto-reconnect
- When connection drops: store peer info, attempt reconnect with exponential backoff
  - 1s, 2s, 4s, 8s, 16s, 30s cap
  - Max 5 attempts before giving up (user can click "Retry")
- Re-establish X3DH session on reconnect (new ephemeral keys)
- Resume file transfers from last ACKed chunk
- Frontend: "Reconnecting…" badge during retry, "Retry" button after exhaustion

### 6.2 — Message De-duplication & Ordering

**Modified**: `src-tauri/src/session.rs` — robust message ordering
- Messages carry a logical timestamp (monotonic clock per device)
- On reconnect: request missed messages by sequence number
- De-duplication: dedupe by message_id (idempotent delivery)
- Sender-side queue: pending messages queued locally if peer offline, sent on reconnect

### 6.3 — Database Performance

**Modified**: `src-tauri/src/storage.rs`
- WAL mode for SQLite (concurrent reads while writing)
- Periodic `PRAGMA optimize` on idle
- Indexed queries: add composite index on `(conversation_id, timestamp)` for message loading
- Message pagination: cursor-based instead of offset-based for large convos
- Storage encryption: move to per-page encryption for large databases

### 6.4 — Memory & CPU Profiling

**New**: Benchmarks and profiling
- `cargo bench` benchmarks for crypto operations (DR encrypt/decrypt latency)
- Memory profiling for large file transfers (ensure <1 chunk in memory)
- CPU profiling for DHT operations
- Connection memory overhead: measure per-connection struct size

### 6.5 — Startup Time Optimization

**Modified**: `src-tauri/src/state.rs`, `src-tauri/src/storage.rs`
- Lazy vault initialization (don't load message store until unlocked)
- Lazy candidate gathering (don't STUN scan on startup unless listening)
- Deferred DHT bootstrap (start after UI is responsive)
- SQLite connection pool for concurrent access

---

## Phase 7: Notifications & Background Mode (UI/UX: +0.3)

### 7.1 — Native Notifications (Existing + Enhancement)

**Already has**: `tauri-plugin-notification`
**Enhancements**:
- Action buttons in notifications: "Reply" (opens quick compose), "Mark Read"
- Notification grouping: per-conversation summary notifications
- Silent notifications for muted conversations (still show in notification center, no sound/badge)
- Notification content preference: always show, show on unlock, never show content

### 7.2 — Background Keep-Alive (Desktop)

**Modified**: `src-tauri/src/main.rs`
- System tray icon with context menu (Show/Hide, Quit)
- Minimize to tray option (app stays running, receives messages)
- Flash tray icon on new message from minimized state
- Tauri `run_on_close` behavior: minimize, don't quit

### 7.3 — System Tray Integration

**New**: Tray menu with:
- Connection status indicator
- Recent conversations (click to open)
- Quick actions: "New conversation", "Settings", "Quit"
- Unread badge on tray icon (platform-specific)

---

## Phase 8: Security Hardening (Already 10/10 — maintain)

### Ongoing security maintenance:
- Regular cargo-audit + pnpm-audit in CI (already done)
- Fuzz testing: expand existing fuzz targets (`frame_parse`, `padding`)
- Add fuzz target for DR message handling
- Add fuzz target for X3DH handshake with malformed bundles
- Formal verification of KDF ratchet (property-based testing with `proptest`)
- Security audit document update: keep `docs/threat-model.md` current

### 8.1 — Screen Capture Protection

**Modified**: `src-tauri/src/state.rs` — window security
- On Windows: call `SetWindowDisplayAffinity` to prevent screen capture of sensitive windows
- On macOS: `NSWindow.sharingType = .none` for chat window
- On Linux: `XDG_SESSION_TYPE` detection, apply `_NET_WM_STATE_ABOVE` + `_NET_WM_WINDOW_TYPE_DIALOG`
- Toggle: user can enable/disable (useful for screen sharing)

### 8.2 — Clipboard Management

**Modified**: `src-tauri/src/commands/mod.rs`
- Auto-clear clipboard after copying sensitive content (fingerprint, invite link)
- Configurable timeout (5s default, 0 = never clear)
- On paste: sanitize clipboard content (strip hidden characters, limit length)

### 8.3 — Lock on Idle/Sleep

**Modified**: Frontend — detect system idle
- Lock vault after configurable idle timeout (5 min default)
- On system sleep/lock: auto-lock vault on resume
- Biometric unlock fallback (platform keychain) — stretch goal

---

## Phase 9: Documentation & Onboarding (Documentation: 9.5 → 10)

### 9.1 — In-App Onboarding Tutorial

**Modified**: `src/views/SetupView.tsx` — interactive walkthrough
- First-run: 3-step onboarding
  1. "Create your identity" — explains keys, fingerprints
  2. "Share your invite" — walk through generating and sending invite
  3. "First connection" — guide to paste invite and connect
- Progressive disclosure: advanced features shown later (Tor, manual forwards, relay)

### 9.2 — User-Facing Documentation

**New**: `docs/user-guide.md` — plain English guide
- What is P2P? Why no servers?
- How to verify fingerprints
- What is NAT traversal?
- Troubleshooting connection issues

### 9.3 — Threat Model Updates

**Modified**: `docs/threat-model.md`
- Update for group chat
- Update for multi-device
- Update for DHT-based peer discovery
- Add side-channel analysis for new features

### 9.4 — API Documentation

**Modified**: Rust code — inline doc improvements
- Every public function has a doc comment with example usage
- Add `# Panics` sections where applicable
- Module-level docs explain architecture decisions

---

## Phase 10: Platform Polish & Distribution

### 10.1 — Code Signing & Notarization
- macOS: Developer ID + notarization via `tauri ci` pipeline
- Windows: Authenticode signing via Azure Key Vault or similar
- Linux: AppImage + Flatpak packaging with signature verification

### 10.2 — Auto-Update Infrastructure
- Use `tauri-plugin-updater` (already in Tauri 2 ecosystem)
- Build update server or use GitHub Releases as update source
- Delta updates for binary size efficiency (future)

### 10.3 — Installer Polish
- Windows: MSI installer with custom icon, Start menu shortcut
- macOS: DMG with background image, Applications shortcut
- Linux: `.deb` + `.rpm` + AppImage via CI matrix

---

## Summary: Scores After Each Phase

| Phase | Content | Architecture | Security | Networking | Tests | UI/UX | Perf | Overall |
|-------|---------|:-----------:|:--------:|:----------:|:----:|:----:|:----:|:-------:|
| Now | Current | 9.5 | 10 | 10 | 9.5 | 8.5 | 8.5 | **9.3** |
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

| File | Action | Est. Lines |
|------|--------|:----------:|
| `src-tauri/src/dht.rs` | **NEW** | 400 |
| `src-tauri/src/lan_discovery.rs` | **NEW** | 200 |
| `src-tauri/src/sync.rs` | **NEW** | 350 |
| `src-tauri/src/group.rs` | **NEW** | 300 |
| `src-tauri/src/crypto.rs` | Modify (+ sender keys) | +250 |
| `src-tauri/src/protocol.rs` | Modify (+ new packet types) | +60 |
| `src-tauri/src/session.rs` | Modify (+ reconnect logic) | +100 |
| `src-tauri/src/state.rs` | Modify (+ groups, sync, DHT) | +50 |
| `src-tauri/src/storage.rs` | Modify (+ WAL, indexes) | +40 |
| `src-tauri/src/commands/chat.rs` | Modify (+ search, reactions) | +100 |
| `src-tauri/src/commands/network.rs` | Modify (+ reconnect) | +60 |
| `src-tauri/src/commands/vault.rs` | Modify (+ export/import) | +150 |
| `src-tauri/src/main.rs` | Modify (+ tray, background) | +100 |
| `src-tauri/Cargo.toml` | Modify (+ deps) | +15 |
| `src/views/ChatView.tsx` | Modify (major) | +400 |
| `src/views/HubView.tsx` | Modify (major) | +200 |
| `src/views/SetupView.tsx` | Modify (onboarding) | +100 |
| `src/views/VaultView.tsx` | Modify (UX polish) | +50 |
| `src/App.tsx` | Modify | +30 |
| `src/styles/` | Modify (themes) | +200 |
| `src/types.ts` | Modify | +30 |
| `docs/` | Various updates | +200 |
| **Total** | | **~3,385** |

---

## Dependency Additions

```toml
# DHT-based peer discovery
kademlia-dht = "0.8"        # or minimal custom Kademlia
# Multi-device sync (QR encoding)
qrcode = "0.14"
# Benchmarking
criterion = { version = "0.5", optional = true }
# Audio capture (voice messages)
# cpal = "0.15"              # cross-platform audio capture
# System tray
# tauri-plugin-tray = "2"    # if not built-in in Tauri v2
# Updater
tauri-plugin-updater = "2"
```

---

## Execution Priority (What to Build First)

**Tier 1 — Core missing features (highest user impact)**:
1. Group chat (Phase 3) — biggest missing feature vs. Signal/WhatsApp
2. Frontend overhaul (Phase 5) — biggest visible quality gap
3. Message reactions + self-destruct (Phase 4) — high UX value

**Tier 2 — P2P completeness**:
4. DHT peer discovery (Phase 1) — removes dependency on out-of-band invite sharing
5. LAN discovery (Phase 1) — zero-config local connections
6. Reconnection logic (Phase 6) — reliability improvement

**Tier 3 — Power user features**:
7. Multi-device sync (Phase 2) — significant engineering effort, moderate user demand
8. Voice messages (Phase 5) — moderate effort, high polish value
9. Keyboard shortcuts (Phase 5) — small effort, high power-user value

**Tier 4 — Platform & maintenance**:
10. Performance optimization (Phase 6)
11. Auto-update + code signing (Phase 10)
12. Idle lock + clipboard (Phase 8)
