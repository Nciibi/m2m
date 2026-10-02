# Phase 2 — Identity Export/Import + Family System

## Overview

Two features that together replace the old "multi-device sync" plan:

**Family** — an explicit, user-curated contact list. Peers are ephemeral by default (delete conversation = peer gone). Adding someone to Family makes them a persistent contact: you give them a nickname, set an optional expiry, and can message them without generating a new invite link.

**Identity Export/Import** — one encrypted file that carries your identity keypair + your entire Family list. Move to a new PC, import, and all your contacts are back. No need to re-add anyone or re-prove who you are.

---

## Part A — Family System

### What "Family" is

- **Your outbound contact list.** You add someone you've already connected with.
- **You name them.** The nickname is your label for them, stored locally, never shared.
- **Configurable duration.** Forever, or auto-expire after N days.
- **Bypasses invites for you.** If someone's in your family, you can message them directly — your app knows their key and last address. No invite generation, no copy-paste.
- **One-directional.** You adding Bob doesn't add you to Bob's family. Bob still needs to invite you or add you to message you freely.

### How connecting to a family member works

1. Tap family member → backend calls `connect_family_member`
2. Backend tries direct TCP to saved address
3. **If connect succeeds** → enter chat view
4. **If connect fails** → frontend shows:
   > *"Can't reach Alice. Paste a new invite for Alice, or remove them."*
   >
   > `[Paste invite link...] [Remove from Family]`
5. User pastes an `m2m://` link for the **same person with their new key/address**
6. Backend validates the invite and **replaces** everything for that family member: public key, address, all of it. No key-matching — the user decides "this invite IS Alice now."
7. Connect retries with the fresh invite data.

### Database

**New `family` table in keys.db:**
```sql
CREATE TABLE IF NOT EXISTS family (
    public_key BLOB NOT NULL UNIQUE,  -- current public key (replaced on update)
    nickname TEXT NOT NULL,            -- your label for them
    added_at INTEGER NOT NULL,         -- unix seconds
    expires_at INTEGER,                -- NULL = forever, otherwise unix seconds
    last_address TEXT                  -- last known address (best-effort)
);
```

The existing `peers` table stays. `upsert_peer` is still called on every connection. Family is a separate opt-in layer on top.

### New Tauri commands (backend)

| Command | Args | Returns | Behavior |
|---|---|---|---|
| `list_family` | — | `Vec<FamilyMember>` | Returns non-expired members. Expired ones filtered out. |
| `add_family_member` | `peer_key_hex`, `nickname`, `expires_in_days: Option<u64>` | `FamilyMember` | Adds peer to family. Must have had a prior connection (peer exists in `peers` table). |
| `remove_family_member` | `peer_key_hex` | — | Removes from family table. Peer still exists in `peers`. |
| `set_family_nickname` | `peer_key_hex`, `nickname` | — | Rename. |
| `connect_family_member` | `peer_key_hex` | `ConnectionInfo` or `ConnectionFailed` | Try direct connect to saved address. If fails, return error with `"needs_update"` flag. |
| `update_family_member` | `peer_key_hex`, `invite_str` | `FamilyMember` | Validates invite. **Replaces** public_key, address, fingerprint. Nickname and expiry stay unchanged. |
| `family_member_expired` | `peer_key_hex` | `bool` | Check if a family member has expired (frontend polling / periodic refresh). |

### FamilyMember type (Rust → TypeScript)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyMember {
    pub public_key_hex: String,
    pub nickname: String,
    pub added_at: i64,
    pub expires_at: Option<i64>,     // null = forever
    pub last_address: Option<String>,
    pub is_expired: bool,
    pub is_reachable: Option<bool>,  // from last check
}
```

### Frontend — HubView Family tab

```
┌─ [Connect] [Chats] [Family] ─────────────────────────┐
│                                                       │
│  + Add to Family                                      │
│                                                       │
│  Alice     (Laptop)          forever     [Msg] [×]    │
│  Bob       (Home PC)         23d left   [Warning] [×] │
│  Charlie   (Phone)           [Offline]  [Update] [×]  │
│                                                       │
│  (empty state):                                       │
│    "Add people you trust to message them without      │
│     generating an invite each time."                  │
└───────────────────────────────────────────────────────┘
```

**States per member row:**

| State | Indicator | Action |
|---|---|---|
| Reachable (connect succeeded) | Green dot, [Msg] | Tap → enter chat |
| Unreachable (connect failed) | Warning icon, address stale | [Update] → paste new invite, or [×] to remove |
| Expired (timer ran out) | Red "Expired" badge | [Renew] → confirm new duration, or [×] to remove |
| Forever, reachable | Green dot | Normal, no expiry shown |

**Add Family Member flow:**
1. Modal opens
2. Dropdown/typeahead of recent conversation peers (from `list_conversations`)
3. Nickname input (required)
4. Duration: `Forever` / `7 days` / `30 days` / `90 days` / `Custom (N days)`
5. Confirm → calls `add_family_member`

**Update (reconnect) flow:**
1. Member shows as unreachable
2. User taps [Update]
3. Input appears inline: "Paste a new invite for Alice"
4. User pastes `m2m://` link
5. Calls `update_family_member` which validates and replaces
6. On success → retry `connect_family_member`
7. On fail → "This invite doesn't seem to work. Try another?"

---

## Part B — Identity Export/Import

### Export format

Single encrypted file (recommend `.m2m-backup` extension):

```json
{
    "version": 1,
    "created_at": 1719446400,
    "identity": {
        "ed25519_public_key": "<base64, 32 bytes>",
        "x25519_public_key": "<base64, 32 bytes>",
        "encrypted_ed25519_secret": "<base64, encrypted>",
        "encrypted_x25519_secret": "<base64, encrypted>",
        "nonce": "<base64, 24 bytes>"
    },
    "family": [
        {
            "public_key": "<base64, 32 bytes>",
            "nickname": "Alice",
            "added_at": 1719446000,
            "expires_at": null,
            "last_address": "203.0.113.5:9000"
        }
    ]
}
```

Encryption:
1. Serialize JSON payload
2. Derive wrapping key via **Argon2id** from export passphrase (separate from vault passphrase)
   - Salt = public key (same pattern as vault)
   - Parameters: 64 MiB, 3 iterations, 4 lanes
3. Encrypt with XChaCha20-Poly1305 + AAD `b"m2m-export-v2"`
4. Write: `<nonce(24B)><ciphertext>` to file

File on disk is binary — not readable without decryption.

### New Tauri commands (backend)

| Command | Args | Returns | Behavior |
|---|---|---|---|
| `export_identity` | `path: String`, `passphrase: String` | — | Exports identity keypair + full family list to encrypted file at path. Strength checks passphrase (40-bit min). |
| `import_identity` | `path: String`, `passphrase: String` | `IdentityInfo` | Reads encrypted file. Derives key from passphrase. Decrypts. Writes identity + family to vault. Reloads state. |

### Export flow (end to end)

1. User clicks "Export Identity" in Settings
2. Native save dialog opens (`.m2m-backup`)
3. Modal: "Create an export passphrase" (strength meter, confirm)
4. Backend:
   - Loads identity from state (Ed25519 + X25519 keypairs)
   - Loads all family members from `family` table
   - Serializes to JSON payload
   - Derives Argon2id key from passphrase + public key salt
   - Encrypts with XChaCha20-Poly1305 + AAD `b"m2m-export-v2"`
   - Writes `nonce || ciphertext` to file
5. Frontend shows: "Exported successfully. Keep this file safe!"

### Import flow (end to end)

1. User clicks "Import Identity" on vault lock screen or settings
2. Native open dialog for `.m2m-backup` files
3. Modal: "Enter export passphrase"
4. Backend:
   - Reads `nonce(24B) || ciphertext` from file
   - Derives Argon2id key from passphrase + stored public key
   - Decrypts with AAD `b"m2m-export-v2"` (fails → "wrong passphrase or corrupted file")
   - Deserializes JSON
   - Writes identity to keys.db (`store_identity`)
   - Writes all family members to `family` table (clear old ones first)
   - Loads identity into state
   - Unlocks vault
5. Frontend: "Identity imported successfully" → redirects to Hub with family intact

### Key management detail

The export passphrase is **separate** from the vault passphrase. This means:
- You can share an export with someone without giving them your vault key
- The export passphrase only protects the *export file*, not your local database
- On import, the identity is re-encrypted with the **new device's vault passphrase**

But this also means import requires the user to go through the vault unlock flow afterward. Alternatively: import creates the vault passphrase inline using the export passphrase. Let me think...

**Decision**: Import sets a **new vault passphrase**. The user enters:
1. The export passphrase (to decrypt the file)
2. A new vault passphrase (to lock the identity on this device)

Or simpler: import just restores the identity, then the user goes through the normal vault setup to set a passphrase. The import doesn't create a vault — it just populates the database. The next time the app starts, `init_identity` finds the identity and the user unlocks normally.

This is cleaner. Import = data restore. Vault passphrase = separate concern.

### Edge cases

| Scenario | Behavior |
|---|---|
| Already has identity on new PC | Import overwrites. Warn: "This will replace your current identity." |
| Corrupted file | Decryption fails → "Invalid passphrase or corrupted backup" |
| No family members | Export still works — empty family array |
| Expired family members | Export includes them with their expiry timestamps. On import, they'll show as expired until renewed or removed. |
| Wrong passphrase | Argon2id + AEAD: decryption fails cleanly, no partial data leaked |
| Export while vault locked | Refuse: "Unlock vault first" |
| Import into empty data dir | Works — creates keys.db, writes identity + family |

---

## Files to Create

| File | Purpose |
|---|---|
| **Backend:** | |
| — | No new files. All changes in existing files. |
| **Frontend:** | |
| `src/components/FamilyTab.tsx` | Family tab component rendered inside HubView |
| `src/components/AddFamilyModal.tsx` | Modal for add-to-family flow with peer selector, nickname, duration |

## Files to Modify

| File | Change |
|---|---|
| **Backend:** | |
| `src-tauri/src/storage.rs` | Add `family` table CRUD to `KeyStore` (5 methods) |
| `src-tauri/src/commands/vault.rs` | Add `export_identity`, `import_identity` commands |
| `src-tauri/src/commands/mod.rs` | Add `FamilyMember` type definition |
| `src-tauri/src/lib.rs` | Register 9 new Tauri commands |
| `src-tauri/src/commands/util.rs` | Add `AAD_EXPORT_V2` constant |
| **Frontend:** | |
| `src/types.ts` | Add `FamilyMember` interface |
| `src/views/HubView.tsx` | Add Family tab next to Connect/Chats |
| `src/components/FamilyTab.tsx` | Full family list with all states |
| `src/components/AddFamilyModal.tsx` | Add modal with peer selector + duration picker |
| `src/views/SettingsView.tsx` | Add "Export Identity" button |
| `src/context/AppContext.tsx` | Add "Import Identity" flow on vault screen |
| `src/__tests__/HubView.test.tsx` | Update mocks for new Family tab |

## What we DON'T change

- ❌ No new protocol packets (family is local-only data)
- ❌ No sync protocol between devices
- ❌ No changes to invite format or handshake
- ❌ No changes to existing peer/conversation lifecycle
- ❌ No changes to discovery (DHT/LAN) — they stay optional

## Migration

Existing users: `family` table is empty. No behavior change. Everything works exactly as before. Family is purely additive — opt-in.

## Test Plan

### Rust tests to add to `storage.rs`

| Test | What it covers |
|---|---|
| `test_family_add_and_list` | Add a member, list returns it |
| `test_family_add_duplicate` | Adding same peer key twice fails gracefully |
| `test_family_remove` | Remove works, list empty after |
| `test_family_expiry_filter` | Expired member excluded from list, still in DB |
| `test_family_update` | Replace public_key + address for existing member |
| `test_family_nickname` | Set/update nickname |

### Rust tests for export/import

| Test | What it covers |
|---|---|
| `test_export_import_roundtrip` | Export → import → identity matches |
| `test_export_import_with_family` | Family list survives roundtrip |
| `test_import_wrong_passphrase` | Wrong passphrase → decryption fails |
| `test_import_corrupted_file` | Truncated/tampered file → clean error |

### Frontend tests

| Test | What it covers |
|---|---|
| HubView: family tab renders | Tab exists, switch to it |
| FamilyTab: empty state | Shows "no family members" message |
| FamilyTab: member list | Shows nickname, status, actions |
| AddFamilyModal: form validation | Empty nickname rejected, valid duration accepted |
| SettingsView: export button | Calls `export_identity` |

---

## Summary

| Concept | One-liner |
|---|---|
| Family | Your private phonebook. You name them, set expiry, message freely. |
| Family reconnect fails | "Need a new invite for this person?" → paste it → everything updated |
| Export | One encrypted file = your identity + all your family contacts |
| Import | Move to new PC → import → identity + family restored |
| Family is local-only | Never shared, never synced, encrypted at rest |
| Export passphrase | Separate from vault passphrase, Argon2id protected |
