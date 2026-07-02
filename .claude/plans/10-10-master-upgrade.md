# M2M → 10/10: Master Upgrade Roadmap

**Current Score: 8.7/10**
- Architecture: 10 | Security/Crypto: 10 | Networking/Privacy: 10
- Test Coverage: 9.5 | UI/UX: 7.0 | Performance: 10

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

## ✅ Phase 2: Multi-Device & Identity Sync — COMPLETE

**Problem**: Identity is locked to one device. No way to use the same key on multiple machines or recover from device loss.

### 2.1 — Identity Export/Import with Encrypted Backup — ✅ DONE

**Implemented**: `src-tauri/src/commands/vault.rs` (lines ~523–640+)
- `export_identity` with passphrase + Argon2id wrapping key (min 12 chars, 40+ bits entropy)
- `import_identity` to restore from encrypted JSON
- Family contacts: `list_family`, `add_family_member`, `remove_family_member`, `set_family_nickname`, `connect_family_member`, `update_family_member`

### 2.2 — Encrypted Sync Layer (P2P Message Sync) — ✅ DONE

**Implemented**: `src-tauri/src/sync.rs` (~440 lines)
- `SyncManager` with device ID, device name, pending invites, paired device list (max 8)
- `generate_sync_invite` — one-time token (24 random bytes, 15-min expiry), `m2m-sync://` prefix
- `pair_sync_device` — authorize an already-connected peer as a sync device
- `handle_sync_device_info` (packet 0x45) — registers paired device, responds with own info, broadcasts conversation metadata
- `handle_sync_payload` (packet 0x46) — upserts received conversation metadata into local store
- `broadcast_sync_data` — sends conversation list as `SyncPayload` over DR session
- Messages are NOT synced by default — only conversation list metadata and peer info
- All sync data travels over existing X3DH+DR encrypted session (no new crypto)

### 2.3 — Read-Only Web Companion (STRETCH) — ❌ NOT STARTED

---

## ⚠️ Phase 3: Group Chat & Multi-Peer Sessions — BACKEND DONE (Frontend Pending)

**Problem**: Currently strictly 1:1 sessions. No group conversations.

### 3.1 — Sender Keys for Group E2EE — ✅ BACKEND DONE

**Implemented**: `src-tauri/src/group.rs` (~987 lines) + `src-tauri/src/crypto.rs` (+160 lines)
- `SenderKeyChain` — HKDF-based message key derivation with 2000-entry skipped-key cache for out-of-order messages
- `Group` struct with sending chain, receiver chains per member, Ed25519 signing/verification
- `GroupManager` with create/add/remove/leave/rotate/list operations
- `Group::encrypt_message()` — derives key from Sender Key chain, encrypts with XChaCha20-Poly1305, signs with Ed25519
- `Group::decrypt_message()` — verifies signature, derives key from receiver chain, decrypts
- `handle_sender_key()` — detects whether a bundle is our own sending chain or another member's receiver chain
- `rotate_own_sender_key()` — generates new chain + signing keypair after member removal
- **15 unit tests** all passing

**Wire protocol**: 7 new PacketType variants (0x50–0x56) + 8 data structs in `protocol.rs`
- GroupCreate, GroupInvite, GroupRemove, GroupSenderKey, GroupEncryptedMessage, GroupInfo, GroupLeave
- All sent over existing X3DH+DR encrypted sessions (standard `send_encrypted_typed` pattern)

**Storage**: 3 new SQLite tables in `messages.db` — `groups`, `group_members`, `group_messages` with indexes. 12 CRUD methods.

**Tauri Commands**: `commands/groups.rs` (~610 lines) — 9 commands: create_group, send_group_message, list_groups, get_group_info, invite_to_group, remove_from_group, leave_group, load_group_messages, update_group_name

**Network dispatch**: 7 new match arms in `spawn_receive_loop` — handles all group packet types with proper lock scoping and inner message decryption

### 3.2 — Frontend: Group Chat UI — ❌ NOT STARTED

Needed: Group chat view (or extended ChatView), group creation modal, group info panel. Frontend types (`GroupInfo`, `GroupMember`, `GroupDetail`, `sender_peer_key_hex` on `ChatMessage`) already defined in `src/types.ts`. No group rendering in ChatView, no GroupContext hook, no group list in HubView yet.

### 3.3 — Frontend: Group Management — ❌ NOT STARTED

Needed: Member list with roles, add/remove controls, GroupContext hook with event listeners

### Review Findings (all fixed)
- 🔴 **Critical**: `handle_sender_key` wasn't setting up the recipient's sending chain — fixed to accept `our_peer_key_hex` param and properly route bundles with signing keys
- 🟡 `sign_group_message` silently returned bogus signatures on bad keys — changed to `Result<Vec<u8>, CryptoError>`
- 🟡 `load_group_messages` never decrypted stored content — added `load_group_messages_with_content()` that returns encrypted blobs for caller-side decryption

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

## ❌ Phase 5: Frontend Overhaul — PARTIAL (3/8 done)

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 5.1 Typing indicators | ❌ | No packet type 0x45, no frontend UI |
| 5.2 Local message search | ❌ | Conversation-list filter exists in HubView, but no per-conversation full-text search |
| 5.3 Drag-and-drop file transfer | ❌ | No drag/drop handlers in ChatView |
| 5.4 Voice messages | ❌ | No audio capture, no playback |
| 5.5 Conversation organization | ❌ | No favorites, mute, archive, or folders — no fields in `types.ts` |
| 5.6 Theme & color customization | ✅ | `ThemeContext.tsx` — light/dark/system modes. `theme.css` — full light theme with all CSS token overrides. `SettingsView.tsx` — theme selector (Monitor/Sun/Moon icons). Backend: `get_theme_preference`, `set_theme_preference`. Default mode is dark through `:root` tokens. No accent picker yet. |
| 5.7 Keyboard navigation | ⚠️ | Partial: `Esc` (back to hub), `Ctrl+,` (settings), `?` (help). Missing: Ctrl+N, Ctrl+K, Ctrl+F, etc. |
| 5.8 Dark mode refinements | ✅ | Dark is the default mode. `:root` in `tokens.css` holds all dark-mode values (canvas gradient, glass effects, edge lights). Full shadow scale for dark backgrounds. No separate `[data-theme="dark"]` block needed — `:root` IS the dark theme. |

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

## ✅ Phase 7: Notifications & Background Mode — COMPLETE

| Sub-phase | Status | Details |
|-----------|--------|---------|
| 7.1 Native notifications | ✅ | `tauri-plugin-notification` integrated — OS notifications on incoming messages from non-active peers. Mute per-conversation via bell icon toggle. |
| 7.2 Background keep-alive | ✅ | `on_window_event` intercepts close → hides to tray. App stays running. |
| 7.3 System tray integration | ✅ | `TrayIconBuilder` with Show/Hide, New Conversation, Settings, Quit menu. Left-click toggles window visibility. |

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
| Now | Current | 10 | 10 | 10 | 9.5 | 6.5 | 10 | **8.5** |
| 1 | DHT + LAN discovery | 10 | 10 | 10 | 9.5 | 8.5 | 8.5 | 9.4 |
| 2 | Multi-device sync | 10 | 10 | 10 | 9.5 | 8.5 | 8.5 | 9.4 |
| 3 | Group chat (backend) | 10 | 10 | 10 | 9.5 | 8.5 | 8.5 | 9.4 |
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
| `src-tauri/src/sync.rs` | **NEW** | 440 | ✅ Done |
| `src-tauri/src/group.rs` | **NEW** | 987 | ✅ Done (incl. 15 unit tests) |
| `src-tauri/src/crypto.rs` | Modify (+ sender keys) | +160 | ✅ Done (SenderKeyChain, signing) |
| `src-tauri/src/protocol.rs` | Modify (+ SyncRequest 0x44, SyncDeviceInfo 0x45, SyncPayload 0x46, + Group 0x50-0x56) | +190 | ✅ Done |
| `src-tauri/src/session.rs` | Modify (+ reconnect) | +100 | ✅ Done |
| `src-tauri/src/state.rs` | Modify (+ groups, sync_manager) | +60 | ✅ Done |
| `src-tauri/src/storage.rs` | Modify (+ WAL, indexes, offline queue, sync queries, group storage + `load_group_messages_with_content` for caller-side decryption) | +120 | ✅ Done |
| `src-tauri/src/commands/chat.rs` | Modify (+ search, reactions, offline queue, flush, `sender_peer_key_hex` field) | +160 | ✅ Done |
| `src-tauri/src/commands/network.rs` | Modify (+ reconnect, sync handlers, 7 group packet handlers — GroupCreate/Invite/SenderKey/EncryptedMessage/Info/Remove, GroupEvent/GroupMessageEvent emits) | +270 | ✅ Done |
| `src-tauri/src/commands/vault.rs` | Modify (+ export/import) | +150 | ✅ Done |
| `src-tauri/src/commands/groups.rs` | **NEW** | 610 | ✅ Done (9 commands: create, send, list, get_info, invite, remove, leave, load, update_name) |
| `src-tauri/src/commands/settings.rs` | Modify (+ `get_theme_preference`, `set_theme_preference`) | +30 | ✅ Done |
| `src-tauri/src/main.rs` | Modify (+ tray, background) | +100 | ✅ Done |
| `src-tauri/src/lib.rs` | Modify (+ register 9 group commands) | +10 | ✅ Done |
| `src-tauri/src/reconnect.rs` | **NEW** | ~80 | ✅ Done |
| `src-tauri/src/window_security.rs` | **NEW** | ~100 | ✅ Done |
| `src-tauri/Cargo.toml` | Modify (+ deps) | +15 | ✅ Done |
| `src/views/ChatView.tsx` | Modify (major) | +400 | ⚠️ Partial |
| `src/views/HubView.tsx` | Modify (major) | +200 | ❌ Pending |
| `src/views/SetupView.tsx` | Modify (onboarding) | +100 | ❌ Pending |
| `src/views/VaultView.tsx` | Modify (UX polish) | +50 | ❌ Pending |
| `src/views/SettingsView.tsx` | Modify (+ theme selector UI with Monitor/Sun/Moon icons) | +30 | ✅ Done |
| `src/context/ThemeContext.tsx` | **NEW** | 85 | ✅ Done (light/dark/system with media query listener) |
| `src/styles/theme.css` | **NEW** | 87 | ✅ Done (full light theme, all CSS token overrides) |
| `src/styles/tokens.css` | Modify (+ dark mode :root values, glass effects, edge light) | +10 | ✅ Done |
| `src/App.tsx` | Modify | +30 | ❌ Pending |
| `src/types.ts` | Modify (+ sender_peer_key_hex, GroupInfo, GroupMember, GroupDetail) | +30 | ✅ Done |
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
# Updater
# tauri-plugin-updater = "2"     ❌ Pending
# Notifications — ✅ Already added
tauri-plugin-notification = "2"
```

---

## Execution Priority (What to Build Next)

**Tier 1 — Core missing features (highest user impact)**:
1. Group Chat frontend UI (Phase 3.2–3.3) — biggest remaining gap. Backend fully done, frontend types defined, just needs the UI.
2. Frontend overhaul (Phase 5) — biggest visible quality gap; typing indicators + message search would be highest-ROI sub-items.

**Tier 2 — Feature complete**:
3. Typing indicators + message search (Phase 5.1–5.2) — medium effort, high UX value

**Tier 3 — Power user features**:
4. Voice messages (Phase 5.4) — moderate effort, high polish value
5. Conversation organization (Phase 5.5) — favorites, mute, archive, folders
6. Dark theme refinements + accent picker (Phase 5.6–5.8) — mostly done, needs accent customization

**Tier 4 — Platform & maintenance**:
7. Auto-update + code signing (Phase 10)
8. Onboarding tutorial (Phase 9.1)
9. API documentation pass (Phase 9.4)
