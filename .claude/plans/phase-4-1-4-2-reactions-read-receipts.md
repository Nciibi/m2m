# Phase 4 — Message Features ✅ COMPLETE

This plan has been completed and expanded to include 4.3 (self-destruct), 4.4 (edit/delete), and 4.5 (markdown).
See `CLAUDE.md` and `docs/protocol-spec.md` for current implementation status.

---

# Phase 4.1 + 4.2 — Message Reactions + Read Receipts

## Overview

Two additive message features that work within the existing EncryptedMessage → decrypt → dispatch pattern. No protocol changes needed for read receipts (local-only), minimal protocol changes for reactions.

---

## 4.1 — Message Reactions

### New packet type

Add to `protocol.rs`:

```rust
PacketType::MessageReaction = 0x41,
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReactionData {
    pub message_id: String,
    pub reaction: String,   // emoji: "👍", "❤️", "😂", "😮", "😢", "🙏"
    // peer_key_hex is implicit — comes from the session
}
```

### Storage

Add `reactions` table to `messages.db`:

```sql
CREATE TABLE IF NOT EXISTS reactions (
    message_id TEXT NOT NULL,
    reaction TEXT NOT NULL,
    peer_key_hex TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (message_id, peer_key_hex, reaction)
);
```

Key design: `peer_key_hex` is the reactor. You can react to your own messages. Each peer can only react once per emoji per message. Stored locally + sent as a packet to the peer.

### MessageStore methods

```rust
pub fn store_reaction(&self, message_id: &str, reaction: &str, peer_key_hex: &str) -> Result<(), StorageError>
pub fn remove_reaction(&self, message_id: &str, reaction: &str, peer_key_hex: &str) -> Result<(), StorageError>
pub fn get_reactions(&self, message_id: &str) -> Result<Vec<StoredReaction>, StorageError>
pub fn get_all_reactions(&self, message_ids: &[&str]) -> Result<HashMap<String, Vec<StoredReaction>>, StorageError>
```

### Sending flow

1. User taps emoji on a message
2. Frontend calls `send_reaction(peer_key_hex, message_id, reaction)`
3. Backend:
   - Stores reaction locally in `reactions` table
   - Encrypts `MessageReactionData` and sends as typed frame (same path as file transfer accept/reject)
4. Peer receives it in receive loop via `PacketType::MessageReaction` → stores locally → emits event

### Tauri commands

| Command | Args | Returns | What it does |
|---|---|---|---|
| `send_reaction` | `peer_key_hex`, `message_id`, `reaction` | `StoredReaction` | Store + send reaction |
| `remove_reaction` | `peer_key_hex`, `message_id`, `reaction` | — | Remove + send reaction removal (or send with empty string) |
| `load_reactions` | `peer_key_hex` | `HashMap<String, Vec<StoredReaction>>` | Load all reactions for a conversation |

### Frontend — ChatView

**On message hover:**
```
┌────────────────────────────────────┐
│ Message text here                  │
│                           👍 😂 ❤️ │  ← reaction bar (appears on hover)
│                           2:30 PM  │
└────────────────────────────────────┘
```

**Reaction bar** — 6 emojis: 👍 ❤️ 😂 😮 😢 🙏
- Tapping an emoji toggles it for that message
- Active reactions are highlighted
- Count shown next to each emoji that has reactions

**Rendered reactions** below the message:
```
┌────────────────────────────────────┐
│ Message text here                  │
│                           2:30 PM  │
│ 👍 2  ❤️ 1                         │  ← inline reaction chips
└────────────────────────────────────┘
```

**Events:**

```typescript
// m2m://reaction — received a new reaction
{ message_id: string, reaction: string, peer_key_hex: string }
```

### React types

```typescript
export interface MessageReaction {
  message_id: string;
  reaction: string;
  peer_key_hex: string;
  created_at: number;
}
```

---

## 4.2 — Read Receipts

### Design decision

Read receipts are **local-only**. They are NOT sent over the wire as packets. This avoids:
- Protocol complexity (batching, dedup, ordering)
- Privacy implications (peer doesn't know when you read)
- Additional encrypted frame overhead

Instead: when the user opens a conversation (ChatView mounts), all messages from the peer that are `direction: "received"` are marked as read. This is purely a UX indicator on the sender's side — it shows a checkmark on sent messages.

### Storage

Add `read_at` column to the `messages` table:

```sql
-- Migration
ALTER TABLE messages ADD COLUMN read_at INTEGER;
```

On load: if any received messages have `read_at IS NULL`, set `read_at = now` for all of them.

### Frontend — ChatView

**Sent message indicators:**
```
┌────────────────────────────────────┐
│ Message text                       │
│                           ✓ 2:30PM │  ← sent (no read receipt yet)
└────────────────────────────────────┘

┌────────────────────────────────────┐
│ Message text                       │
│                          ✓✓ 2:30PM │  ← read (read_at is set)
└────────────────────────────────────┘
```

**Load-time marking:** When `load_messages` is called, after mounting:
1. Frontend reads messages
2. Calls `mark_messages_read(peer_key_hex)`
3. Backend finds all `received` messages with `read_at IS NULL` → sets `read_at = now`
4. Frontend refreshes to show updated read status

### Tauri commands

| Command | Args | Returns | What it does |
|---|---|---|---|
| `mark_messages_read` | `peer_key_hex` | `count: u32` | Marks all unread received messages as read |

### Loading reactions alongside messages

Modify `load_messages` to include reactions:

```typescript
export interface StoredMessage {
  id: string;
  content: string;
  direction: string;
  timestamp: number;
  read_at?: number | null;   // new
  reactions: MessageReaction[]; // new
}
```

On the backend, `load_messages` now:
1. Loads messages (as before)
2. Loads all reactions for the conversation's message IDs
3. Attaches reactions to each message

---

## Messages DB migration

The `MessageStore::open()` migration function needs to add:

```sql
-- reactions table
CREATE TABLE IF NOT EXISTS reactions (
    message_id TEXT NOT NULL,
    reaction TEXT NOT NULL,
    peer_key_hex TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (message_id, peer_key_hex, reaction)
);

-- ALTER TABLE for read_at
ALTER TABLE messages ADD COLUMN read_at INTEGER;
```

Using the same migration pattern as `migrate_conversations_table`.

---

## Files to modify

| File | Change |
|---|---|
| `src-tauri/src/protocol.rs` | Add `MessageReaction = 0x41`, `MessageReactionData` struct, `PacketType::from_byte` and test |
| `src-tauri/src/storage.rs` | Add `reactions` table + CRUD, `read_at` migration, update `load_messages` to include `read_at`, add `get_all_reactions` |
| `src-tauri/src/commands/chat.rs` | Add `send_reaction`, `remove_reaction`, `load_reactions`, `mark_messages_read` commands. Add reactions to `ChatMessage` response. |
| `src-tauri/src/commands/mod.rs` | Add `ChatMessage.read_at`, `ChatMessage.reactions` fields |
| `src-tauri/src/lib.rs` | Register 4 new commands |
| `src-tauri/src/network.rs` / receive loop | Add `PacketType::MessageReaction` handler |
| `src/types.ts` | Add `MessageReaction`, update `ChatMessage` |
| `src/views/ChatView.tsx` | Reaction bar on hover, reaction chips below messages, read receipt indicators |
| `src/context/ChatContext.tsx` | Handle `m2m://reaction` event |

## What we DON'T change

- ❌ No new encryption paths (reactions use existing `decrypt_typed_frame` / `send_encrypted_typed`)
- ❌ No protocol version bump (backward compatible — unknown packet types are ignored by older peers)
- ❌ No read receipt packets over the wire (local-only)
- ❌ No changes to handshake, invites, or discovery

## Test plan

| Test | What it covers |
|---|---|
| `protocol_tests::test_message_reaction_roundtrip` | ReactionData serialize/deserialize |
| `session_tests::test_reaction_send_receive` | Reaction sent via DR path, received + decrypted |
| `storage_tests::test_store_and_load_reactions` | Reaction CRUD roundtrip |
| `storage_tests::test_mark_messages_read` | read_at set correctly |
| `storage_tests::test_reactions_per_message` | Multiple reactions on same message |
