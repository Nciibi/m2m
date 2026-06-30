# M2M — Project Guide

## Architecture Overview

See `docs/architecture.md` for the full module map. Key modules:

| Module | Purpose |
|--------|---------|
| `src-tauri/src/crypto.rs` | Ed25519, X25519, X3DH, Double Ratchet, HKDF, AEAD |
| `src-tauri/src/session.rs` | Encrypted session lifecycle, send/receive with ratchet |
| `src-tauri/src/protocol.rs` | Wire format: packet types, framing, MessagePack serialization |
| `src-tauri/src/commands/` | Tauri IPC bridge — 8 modules (chat, vault, network, files, discovery, security, relay, settings) |
| `src-tauri/src/storage.rs` | SQLite-backed MessageStore, KeyStore, TransferStore (app-level AEAD) |
| `src-tauri/src/state.rs` | Central AppState with all runtime config + connection state |
| `src-tauri/src/dht.rs` | Custom lightweight Kademlia DHT for peer discovery |
| `src-tauri/src/lan_discovery.rs` | UDP multicast LAN peer discovery |

## What's Implemented

- ✅ X3DH + Double Ratchet (Signal-protocol E2EE)
- ✅ DHT peer discovery + LAN multicast discovery (OFF by default)
- ✅ TURN relay server (self-hosted)
- ✅ Identity export/import + family contacts
- ✅ File transfer with streaming, chunk hashing, ACK/cancel/resume
- ✅ Conversation retention policies (auto-delete/export)
- ✅ Private mode + Tor SOCKS5 support
- ✅ STUN + Happy Eyeballs connection strategies
- ✅ Reactions (0x41), Message Edit (0x42), Message Delete (0x43)
- ✅ Self-destruct timer on messages
- ✅ Read receipts (local)
- ✅ Markdown rendering (bold, italic, code, links)
- ✅ Clipboard auto-clear + screen capture protection + idle vault lock
- ✅ 95 frontend tests, pre-existing backend borrow-checker issues

## Key Patterns

### Typed encrypted frames
Use `session.send_encrypted_typed(write_half, PacketType::Xxx, &serialized)` for any feature that needs to send structured data over the encrypted session. The handler in `network.rs` receives it and dispatches by `PacketType`.

### Lock scoping
Always scope `state.message_store.lock()` narrowly. The `rusqlite::Connection` uses `RefCell` internally and makes the future `!Send` if held across `.await`.

### Borrow checker workaround for send_encrypted_typed
```rust
let PeerConnection { session, write_half, .. } = &mut *conn;
session.send_encrypted_typed(write_half, packet_type, &data).await?;
```
This destructures `conn` first so both borrows are from the destructured fields, not from `conn`.

### Adding a new ChatMessage field
Update ALL construction sites:
1. `src-tauri/src/commands/mod.rs` — ChatMessage struct
2. `src-tauri/src/commands/chat.rs` — load_messages, send_message, send_message_with_timer
3. `src-tauri/src/commands/network.rs` — incoming message construction
4. `src-tauri/src/storage.rs` — StoredMessage struct + both query_map closures
5. `src/types.ts` — ChatMessage interface
6. `src/views/ChatView.tsx` — rendering (if visible)

### Privacy-first defaults
All discovery and security features are OFF by default. Users must explicitly enable them. This follows the principle that convenience is the enemy of privacy.

## Remaining pre-existing errors
- `dht.rs:51` — unused import `EphemeralPeerId` (warning → error in CI)
- `chat.rs:363,400` — borrow conflict in `send_encrypted_typed` (needs the destructure pattern above)
