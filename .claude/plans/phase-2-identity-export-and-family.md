# Phase 2 — Identity Export/Import + Family System

## Overview

Two connected features that replace the old multi-device-sync plan:

1. **Family** — an explicit, user-curated contact list. Peers are ephemeral by default (delete conversation = peer gone). Adding someone to Family makes them a persistent contact: you give them a name, set an optional expiry, and can message them without generating a new invite link.

2. **Identity Export/Import** — one encrypted file that carries your identity keypair + your entire Family list. Move to a new PC, import, and all your contacts are there.

---

## Part A — Family System

### What "Family" is

- **Your outbound contact list.** You add someone you've already connected with to your family.
- **You name them.** Not their self-chosen display name — *your* label for *them*.
- **Configurable duration.** Forever, or auto-expire after N days.
- **Bypasses invites.** If someone's in your family, you can message them directly — your app knows their key and tries to connect. No invite generation, no copy-paste.
- **One-directional.** You adding Bob to your family does NOT add you to Bob's family. Bob still needs an invite to message you, unless he also adds you.

### What happens when you message a family member

Your app knows their public key and last-known address. It tries:
1. Direct connect using saved address
2. If that fails, they get a notification: *"Alice (someone you've contacted before) is trying to reach you."*

The recipient can accept or ignore. It's not an automatic connection — it's a persistent outbound shortcut on your side.

### Database changes

**`keys.db` — new `family` table:**
```sql
CREATE TABLE IF NOT EXISTS family (
    public_key BLOB NOT NULL UNIQUE,
    nickname TEXT NOT NULL,          -- your label for them
    added_at INTEGER NOT NULL,       -- unix seconds
    expires_at INTEGER,              -- NULL = forever, otherwise unix seconds
    last_address TEXT                -- last known IP:port (best-effort)
);
```

**Migration:** The existing `peers` table stays (`upsert_peer` is still called on every connection). Family is a separate opt-in table. Peers can exist in `peers` without being in `family`.

### New Tauri commands

| Command | Input | Output | What it does |
|---|---|---|---|
| `list_family` | — | `Vec<FamilyMember>` | Returns all non-expired family members |
| `add_family_member` | `peer_key_hex`, `nickname`, `expires_in_days: Option<u64>` | `FamilyMember` | Add a peer to family. Must have had a prior connection. |
| `remove_family_member` | `peer_key_hex` | — | Remove from family |
| `set_family_nickname` | `peer_key_hex`, `nickname` | — | Rename a family member |
| `connect_family_member` | `peer_key_hex` | `ConnectionInfo` | Direct connect without invite — uses saved peer info |

### Frontend — HubView

Add a **Family** tab next to Connect and Chats:

```
┌─ [Connect] [Chats] [Family] ─────────────────┐
│                                               │
│ Family Members (3)                   [+ Add]  │
│ ───────────────────────────────────────────── │
│ 🔵 Alice    (Laptop)     forever    [Msg] [X] │
│ 🔵 Bob      (Home PC)    23d left   [Msg] [X] │
│ 🔵 Charlie  (Phone)      expired    [Renew]   │
│                                               │
│ (empty state if no family members)            │
│   "Add people you trust to message them       │
│    without generating an invite each time."   │
```

**Add family member flow:**
1. Modal: text input for peer key (auto-filled from recent conversations)
2. Nickname input
3. Duration selector: "Forever" / "7 days" / "30 days" / "Custom"
4. Confirm button

**Message flow:**
1. Tap [Msg] → calls `connect_family_member`
2. If connect succeeds → enter chat view
3. If connect fails → show "Peer offline" with option to retry later

### Privacy note

Family is stored locally and encrypted at rest (same as everything else). It is NEVER shared with the peer. Your nickname for them stays on your device. The export file is passphrase-encrypted — if someone gets your export file without the passphrase, they get nothing.

---

## Part B — Identity Export/Import

### Export format

Single encrypted JSON file (`.m2m-backup`):

```json
{
    "version": 1,
    "created_at": 1719446400,
    "identity": {
        "public_key": "<base64, 32 bytes>",
        "encrypted_private_key": "<base64, encrypted with export passphrase>",
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
- Derive wrapping key via Argon2id from an **export passphrase** (separate from vault passphrase)
- Encrypt the entire JSON payload with XChaCha20-Poly1305 (same AAD pattern: `b"m2m-export-v2"`)
- Final file: `<nonce(24B)><ciphertext>` — same pattern as storage encryption

### New Tauri commands

| Command | Input | Output | What it does |
|---|---|---|---|
| `export_identity` | `path: String`, `passphrase: String` | — | Export identity + family to encrypted file |
| `import_identity` | `path: String`, `passphrase: String` | `IdentityInfo` | Import from backup file. Writes to vault. |

### Export flow

1. User enters export passphrase (strength-checked, same 40-bit minimum)
2. Derive Argon2id key from passphrase + public_key as salt
3. Serialize identity + family list to JSON
4. Encrypt with XChaCha20-Poly1305 + AAD `b"m2m-export-v2"`
5. Write `nonce || ciphertext` to file path chosen by user (OS save dialog)
6. (optional) Show warning: "Keep this file safe. Anyone with the passphrase can access your identity."

### Import flow

1. User selects backup file (OS open dialog)
2. User enters export passphrase
3. Read `nonce(24B) || ciphertext`
4. Derive Argon2id key from passphrase + stored public_key as salt
5. Decrypt and verify AAD
6. Deserialize JSON
7. Write identity to keys.db (same `store_identity` path as vault unlock)
8. Write all family members to `family` table
9. Set vault as initialized
10. Reload identity into state
11. Return IdentityInfo

### Edge cases

- **Already has identity on new PC**: Import overwrites the existing identity. Warn: "This will replace your current identity. Are you sure?"
- **Corrupted file**: Decryption fails → "Invalid passphrase or corrupted backup file."
- **No family members**: Export still works — just identity with empty family list.
- **Expired family members**: Export includes them (expiry timestamps preserved). On import, they'll show as expired until renewed.

---

## Files to create

| File | Purpose |
|---|---|
| `src/views/FamilyView.tsx` | New tab component in Hub |
| `src/components/FamilyAddModal.tsx` | Modal for add-family flow |
| `src/components/ui/TimerSelect.tsx` | Duration selector component |

## Files to modify

| File | Change |
|---|---|
| **Backend:** | |
| `src-tauri/src/storage.rs` | Add `family` table CRUD to KeyStore |
| `src-tauri/src/commands/vault.rs` | Add `export_identity`, `import_identity` commands |
| `src-tauri/src/commands/mod.rs` | Export types: `FamilyMember` |
| `src-tauri/src/lib.rs` | Register 7 new commands |
| `src-tauri/src/commands/util.rs` | Add `AAD_EXPORT_V2` constant |
| **Frontend:** | |
| `src/types.ts` | Add `FamilyMember` |
| `src/context/ChatContext.tsx` | Add family state + handlers |
| `src/views/HubView.tsx` | Add Family tab with member list |
| `src/views/SettingsView.tsx` | Add "Export Identity" button |
| `src/context/AppContext.tsx` | Add "Import Identity" flow |
| `src/__tests__/HubView.test.tsx` | Update for Family tab |

## What we DON'T change

- ❌ No new protocol packets (family is local-only)
- ❌ No sync protocol (no P2P family syncing — import/export only)
- ❌ No automatic peer discovery for family
- ❌ No invite changes (invites remain the same for non-family connections)

## Migration path

Existing users: `family` table is empty. Nothing changes. They keep connecting via invites as before. Family is purely additive — opt-in.

## Test plan

| Test | What it covers |
|---|---|
| `storage::tests::test_family_add_list_remove` | Family CRUD roundtrip |
| `storage::tests::test_family_expiry` | Expired members filtered out |
| `storage::tests::test_family_duplicate` | Can't add same peer twice |
| `vault::test_export_import_roundtrip` | Export → import → identity matches |
| `vault::test_export_import_with_family` | Family list survives roundtrip |
| `vault::test_import_wrong_passphrase` | Rejects wrong passphrase |
| `vault::test_import_corrupted_file` | Rejects corrupted data |
| Frontend: Family tab renders | Empty state, populated list |
| Frontend: Add family modal | Form validation, submit flow |

---

## Summary

- **Family** = your private contact list. You name them, set expiry, message freely.
- **Export/Import** = one encrypted file carries your identity + family to a new PC.
- No invites needed for family members (outbound only).
- Peers still ephemeral by default — family is opt-in.
- Everything encrypted at rest and in transit, nothing shared without consent.
