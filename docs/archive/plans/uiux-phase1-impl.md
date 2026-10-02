# UI/UX Phase 1–2 Implementation Plan (Tier 1)

**Goal**: Execute Phases 1 & 2 of the UI/UX upgrade plan — state management extraction + view rewrites.

**Phase 0 is DONE**: Design tokens, theme system, 8 UI primitives (Button, Input, Select, Card, Modal, Badge, Toast, LoadingSpinner, ProgressBar), ThemeContext, light/dark/system theme, SettingsView theme selector.

---

## Phase 1 — State Management Organization (what exists vs what to fix)

### Current state
- `AppContext.tsx` (88 lines) — view routing, identity, toast, keyboard shortcuts, theme detection
- `ChatContext.tsx` (524 lines) — all messaging state, file transfers, reactions, reconnection, event listeners
- `SettingsContext.tsx` (320 lines) — all network, discovery, security state + handlers
- `VaultContext.tsx` — vault unlock/create state
- `ThemeContext.tsx` (85 lines) — theme management

### What's already good
- Context separation is already clean (no 650-line App.tsx)
- Views already take few props (HubView uses context hooks, not 30+ props)
- Event listeners are properly bound in contexts
- No monolithic hook needed — the context pattern works

### What needs fixing
1. `ChatContext.tsx` is too large (524 lines) — extract event listener setup into a helper hook
2. Most "M2M://" event handler names are hardcoded magic strings inline
3. File request event listeners could be cleaner

**Recommendation**: Skip major refactoring — current state is already close to the target. Focus effort on Phase 2 view rewrites instead.

---

## Phase 2 — View Rewrites + Polish (THE MAIN WORK)

### 2a — SetupView (loading splash) ⚡

Current state: Icon + "Initializing Secure Enclave" + loading dots + crypto badge. Already decent.

**Changes**:
- ✅ Already has pulsing lock icon (setup-icon with glow)
- ✅ Already has loading dots with animation
- ✅ Already shows crypto algorithms used
- **Minor polish**: Add step indicator-like progressive text ("Generating Ed25519 identity keys…" → "Creating X25519 key exchange pair…")

### 2b — VaultView (passphrase entry) ⚡

Current state: Already has:
- ✅ Animated vault icon (lock/unlock)
- ✅ Eye toggle for visibility
- ✅ Strength bar with real-time entropy bits
- ✅ "What is a strong passphrase?" info tooltip
- ✅ Shake animation on error
- ✅ Min 12 chars + entropy validation

**Small additions**:
- Add "Paste" button for long passphrases
- Add cached-fingerprint hint: "This vault belongs to [fingerprint]" when re-opening

### 2c — HubView (Connect + Chats) 🔥 — MOST WORK

Current state has tabs: Connect, Chats, Nearby, Family. Good structure, needs polish.

**Connect tab**:
- Add invite expiry countdown timer (uses `generatedInviteExpiry`)
- Add recent invites history (last 5, stored in state)
- Show listening indicator (green pulsing dot when hosting)
- Better empty states with illustrations

**Chats tab**:
- ✅ Search bar already exists
- ✅ Conversation avatars with dynamic color via `hashToColor`
- ✅ Online/offline indicators
- ✅ Delete button per conversation
- ✅ Mute button per conversation
- ✅ Empty state with icon + message
- **Add**: Last-seen relative time display
- **Add**: Conversation sorting (most recent first — ensure it's working)

### 2d — ChatView (messaging) 🔥 — MOST WORK

Current state: Solid foundation — markdown rendering, reactions, editing, deletion, self-destruct timer, scroll-to-bottom FAB, message grouping by date, inline edit.

**What's missing vs plan**:

| Feature | Status |
|---------|--------|
| Typing indicator | ✅ "Peer is typing…" UI exists (typingPeers state), but no wire protocol |
| Message grouping by sender | ❌ Not implemented |
| Date separators | ✅ Already done with `groupByDate` |
| Scroll-to-bottom button | ✅ Already done |
| Emoji picker in input | ❌ Missing — need emoji grid in message input toolbar |
| Message status (sending/sent/delivered) | ❌ Missing — only basic send |
| Inline images | ❌ Not implemented |
| Code blocks | ✅ Inline code renders, multi-line code blocks not styled |
| File transfer progress bar | ❌ Missing — need ProgressBar component integration |
| Multi-line textarea | ✅ Auto-grows to 120px |
| Toolbar (emoji/attach/clear/send) | ⚠️ Partial — attach button exists, no emoji button |
| Ctrl+Enter | ✅ Already done |
| Drag-and-drop file attachment | ❌ Missing |

**New items (not in original plan but needed)**:
- Add **sender labels** in group chat when `sender_peer_key_hex` is set
- Add emoji picker button next to the chat input

### 2e — SettingsView

Current state: Already has all sections — Identity, Network, Discovery, Security, STUN servers, About.

**Minor polish**:
- Add "Copy" button to Public IP row
- Add health-check indicators on STUN servers
- Add "Test Tor" button

---

## Phase 3 — Animations (Low Priority)

Skip for now — existing CSS animations are adequate:
- ✅ Loading dots
- ✅ Vault icon pulse
- ✅ Shake on error
- ✅ Date separators fade in
- ✅ Message bubble slide-up (via `animation: msgSlide`)
- ✅ Modal fade-in + zoom
- ✅ Button hover shine
- ✅ Reduced-motion media query exists

---

## Implementation Order (Tier 1)

### Sprint A — ChatView enhancements (highest impact)
1. Add emoji picker to message input toolbar
2. Add message status indicators (sending/sent/delivered)
3. Integrate file transfer progress bars (existing ProgressBar component)
4. Add sender labels for group messages

### Sprint B — HubView polish
5. Invite expiry countdown + recent invites
6. Last-seen relative time
7. Better empty states

### Sprint C — VaultView polish
8. Paste button + fingerprint hint

### Sprint D — SettingsView polish
9. Copy IP, STUN health, Test Tor

---

## File Changes

| File | Action | Est. lines |
|------|--------|:----------:|
| `src/components/ui/icons/MonitorIcon.tsx` | **NEW** | ~15 |
| `src/components/ui/icons/SunIcon.tsx` | **NEW** | ~15 |
| `src/components/ui/icons/MoonIcon.tsx` | **NEW** | ~15 |
| `src/components/ui/Icons.tsx` | Modify (+3 exports) | +3 |
| `src/views/ChatView.tsx` | Modify (emoji picker, progress bars, status indicators, sender labels) | +150 |
| `src/views/HubView.tsx` | Modify (invite countdown, last-seen, recent invites) | +100 |
| `src/views/VaultView.tsx` | Modify (paste button, fingerprint hint) | +30 |
| `src/views/SettingsView.tsx` | Modify (copy IP, stun health) | +30 |
| `src/context/ChatContext.tsx` | Modify (+ emoji state, + message status tracking) | +30 |
| `src/styles/components/chat.css` | Modify (sender labels, emoji picker, progress bar styles) | +60 |
| `src/styles/components/hub.css` | Modify (invite countdown, last-seen styles) | +30 |
