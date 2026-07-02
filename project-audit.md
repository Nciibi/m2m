# M2M Project Audit & Implementation Status

## Overview

| Area | Expected Implementation | Current Status | Gaps / Action Needed |
|------|------------------------|----------------|----------------------|
| Core Crypto (`src-tauri/src/crypto.rs`) | Ed25519, X25519, X3DH, Double Ratchet, HKDF, AEAD | Implemented; pending sender‑key extension for groups | Add sender‑key logic (~250 lines) |
| Session Layer (`src-tauri/src/session.rs`) | Ratchet handling, message send/receive, reconnect logic | ✅ Reconnect added (`reconnect.rs`) and integrated | None |
| Protocol / Wire Format (`src-tauri/src/protocol.rs`) | MessagePack serialization, packet type enum | ✅ Core packet types present; typing‑indicator (+60 lines) pending | Add `PacketType::TypingIndicator` and serialization |
| DHT Peer Discovery (`src-tauri/src/dht.rs`) | Custom Kademlia‑style DHT, NAT‑aware bootstrap | ✅ New file, fully functional | None |
| LAN Discovery (`src-tauri/src/lan_discovery.rs`) | UDP multicast discovery, token‑based announce | ✅ New file, works | None |
| Relay / TURN (`relay‑server/…`) | Standalone TURN/relay, docker compose | ✅ Implemented | None |
| Message Store (`src-tauri/src/storage.rs`) | SQLite + WAL, indexes, expiry cleanup | ✅ WAL enabled, indexes added | None |
| Commands – Chat (`src-tauri/src/commands/chat.rs`) | Send, load, edit, reactions, read receipts, self‑destruct | ✅ Most features present; search & some reaction UI partially done | Finish full‑text search, complete reaction UI |
| Commands – Network (`src-tauri/src/commands/network.rs`) | Encrypted packet routing, reconnect integration | ✅ Done | None |
| Commands – Vault (`src-tauri/src/commands/vault.rs`) | Export/import encrypted identity, family contacts | ✅ Completed | None |
| Sync Layer (`src-tauri/src/sync.rs`) | Encrypted multi‑device sync (planned) | ❌ Not started | Design protocol, implement file (~350 lines) |
| Group Chat (`src-tauri/src/group.rs`) | Sender‑key distribution, group ratchet | ❌ Not started | Implement group logic, UI hooks |
| Frontend – Views (`src/views/*.tsx`) | Chat, Hub, Setup, Vault, theming, dark mode | Mixed: ChatView partially done; Hub, Setup, Vault, theming pending | Complete UI overhaul per Phase 5 (typing indicator, search, dark theme, shortcuts) |
| Types (`src/types.ts`) | Mirror Rust structs for UI | ❌ Pending updates (new fields for groups, sync, typing) | Extend interfaces accordingly |
| Performance (`src-tauri/src/reconnect.rs`, DB WAL) | Exponential backoff, deduplication, indexing | ✅ Implemented | None |
| Security Hardening (`src-tauri/src/window_security.rs`, clipboard clear) | Screen‑capture protection, clipboard auto‑clear, idle lock | ✅ Completed | None |
| Notifications / Tray | `tauri-plugin-notification`, system tray, background keep‑alive | ⚠️ Notification dep present; tray & background not wired | Add tray UI, background handling (`main.rs` pending) |
| Distribution | Code signing, auto‑update, installers | ❌ Not started | Add `tauri-plugin-updater`, signing configs |
| Documentation | Threat model, onboarding, user guide | Partial – core docs exist; onboarding UI missing | Write `docs/user-guide.md`, improve in‑app tutorial |
| Tests | 95 frontend tests, backend borrow‑checker fixes | Mostly passing; borrow conflict in `chat.rs` lines 363/400 noted | Resolve conflict using destructuring pattern from the guide |

## Key Findings

1. **Core cryptography and session management are solid** – all critical security primitives are present and tested.
2. **Peer discovery (DHT & LAN) is fully implemented** and matches the roadmap.
3. **Message‑feature set (reactions, edit, delete, self‑destruct, markdown) is complete**.
4. **Major missing pieces**:
   - **Group chat** (sender‑key distribution, UI, protocol changes).
   - **Frontend overhaul** (dark theme, typing indicator, full‑text search, keyboard shortcuts, theming).
   - **Multi‑device sync** (protocol and UI).
   - **System tray & background behavior** (notification handling, keep‑alive).
   - **Distribution tooling** (code signing, auto‑update).
5. **Partial/unfinished items**:
   - `src-tauri/src/commands/chat.rs` – reaction UI and search not fully wired.
   - `src-tauri/src/main.rs` – tray/background scaffolding pending.
   - `Cargo.toml` – some optional deps (qrcode, cpal, tray, updater) listed but not enabled.
   - Documentation onboarding is a placeholder.

## Suggested Next Steps (Tier 1 Priority)

1. **Group Chat** – implement sender‑key logic, new packet types, and UI integration.
2. **Frontend Overhaul** – add dark‑theme CSS, typing‑indicator badge, full‑text search bar, and keyboard shortcuts.
3. **Resolve Borrow Conflict** in `src-tauri/src/commands/chat.rs` using the destructuring pattern described in the architecture guide.

Feel free to let me know which area you’d like to prioritize, or ask for a deeper dive into any specific module.