# Tier B Testing Plan

## Goal
Close the three Tier B gaps identified in review_part2.md:
1. **(B.4) MEDIUM — Integration tests**: Full handshake → message exchange through layer boundaries
2. **(B.5) MEDIUM — Frontend tests**: HubView, ChatView, SettingsView + contexts
3. **(B.6) LOW — Typed-frame DR tests**: DR encryption/decryption for file transfers and metadata

---

## 1. Rust Backend — Integration Tests (session.rs)

### 1.1 Full X3DH Handshake → Text Message Exchange
- Full duplex handshake via `handshake_as_initiator_x3dh` / `handshake_as_responder_x3dh`
- After established, `send_text` + `decrypt_message` through the DR path
- Verify plaintext round-trips correctly
- Verify counter/state advances

### 1.2 Full X3DH Handshake → File Transfer (DR Path)
- Full handshake → `send_file_request` → `decrypt_typed_frame` (DR path)
- Verify `FileTransferRequestData` fields preserved
- Test file accept/reject through DR path

### 1.3 Full X3DH Handshake → Conversation Metadata (DR Path)
- Full handshake → `send_conversation_meta` → `decrypt_typed_frame` (DR path)
- Verify display names round-trip

### 1.4 DH Ratchet Trigger During Message Exchange
- Send 101+ messages in a full X3DH session (triggers `should_ratchet(100)`)
- Verify DH ratchet key changes appear in DR headers
- Verify all messages decrypt correctly after multiple ratchets

### 1.5 Session Expiry During Active Session
- Set a short `established_at`, send a message
- `check_expiry()` should reject after `MAX_SESSION_DURATION_SECS`

### 1.6 Replay Protection via DR Path in Full Session
- Full handshake → send messages → capture frame → verify old frame rejected

### 1.7 Heartbeat Roundtrip
- Test `send_heartbeat` + `handle_heartbeat` for both Heartbeat and HeartbeatAck

---

## 2. Rust Backend — Typed-Frame DR Tests (session.rs)

### 2.1 DR encrypt_file_request / decrypt_typed_frame
- Manually set up Session with `ratchet = Some(...)` (bypass full handshake)
- Send file request through `send_encrypted_typed` → verify DR header present
- Decrypt with `decrypt_typed_frame` → verify `FileTransferRequestData`

### 2.2 DR encrypt_file_accept / decrypt_typed_frame
- Same pattern for accept

### 2.3 DR encrypt_file_reject / decrypt_typed_frame
- Same pattern for reject

### 2.4 DR encrypt_file_chunk / decrypt_typed_frame
- Same pattern for file chunks

### 2.5 DR encrypt_file_complete / decrypt_typed_frame
- Same pattern for complete

### 2.6 DR encrypt_conversation_meta / decrypt_typed_frame
- Same pattern for conversation metadata

### 2.7 DR decrypt_typed_frame without ratchet fails
- Legacy session without ratchet, DR envelope → `Err(SessionError::InvalidState)`

---

## 3. Frontend Tests

### 3.1 HubView Tests (`src/__tests__/HubView.test.tsx`)
- Renders connect tab by default
- Renders chats tab with conversations
- Shows empty state when no conversations
- Generate invite button triggers handleGenerateInvite
- Connect button disabled when no invite text
- Connect button enabled with invite text
- Copy invite button copies link
- Searches/filters conversations
- Settings button navigates to settings
- Shows Tor warning when tor + no private mode + invite generated
- Fingerprint display shows identity fingerprint
- Badge shows "Offline" status

### 3.2 ChatView Tests (`src/__tests__/ChatView.test.tsx`)
- Renders encrypted session header
- Shows messages list
- Shows empty state when no messages
- Sends message on form submit
- Disables send button when input empty
- Shows file transfer requests with accept/reject
- Shows disconnect button when established
- Shows verified icon when peer verified
- Grouped messages by date
- Scroll-to-bottom button appears when scrolled up
- Back to hub button navigates back
- Retention policy selector works
- Export conversation button works

### 3.3 SettingsView Tests (`src/__tests__/SettingsView.test.tsx`)
- Renders identity section with fingerprint and public key
- Copies fingerprint to clipboard
- Network section shows STUN discover button
- NAT type badge displayed
- Private mode toggle works
- Tor toggle works
- Connectivity check button works
- STUN server list rendered
- Add STUN server works
- Remove STUN server works
- Reset STUN defaults works
- Version info displayed
- Back to hub navigates

### 3.4 AppContext Tests (`src/__tests__/AppContext.test.tsx`)
- Provides default values
- setView changes current view
- addToast adds notification
- removeToast removes notification
- Initializes with vault status from Tauri invoke

### 3.5 ChatContext Tests (`src/__tests__/ChatContext.test.tsx`)
- Provides default connection state
- handleSendMessage calls Tauri invoke
- handleConnect validates invite and connects
- handleDisconnect calls Tauri invoke
- Listens for message events
- Listens for connection events
- Listens for file request events

### 3.6 SettingsContext Tests (`src/__tests__/SettingsContext.test.tsx`)
- Provides default STUN config
- handleStunDiscover fetches STUN info
- handleTorToggle toggles Tor setting
- handlePrivateModeToggle toggles private mode
- handleAddStunServer validates and adds server
- handleRemoveStunServer removes server
- handleResetStunDefaults resets defaults

---

## File List

### New Files
1. `src-tauri/src/session.rs` — Add ~20 new tests (integration + typed-frame DR)
2. `src/__tests__/HubView.test.tsx` — ~15 test cases
3. `src/__tests__/ChatView.test.tsx` — ~15 test cases
4. `src/__tests__/SettingsView.test.tsx` — ~15 test cases
5. `src/__tests__/AppContext.test.tsx` — ~6 test cases
6. `src/__tests__/ChatContext.test.tsx` — ~10 test cases
7. `src/__tests__/SettingsContext.test.tsx` — ~10 test cases

### Modified Files
8. `src/__tests__/setup.ts` — Add shared mocks
9. `package.json` — Add vitest config if needed (check existing)
