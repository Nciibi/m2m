# M2M UI/UX Upgrade Implementation Plan

## Overview
This plan details the gaps that prevent the app from achieving a **10/10** UI/UX rating and provides a concrete, prioritized roadmap to close those gaps. It follows the senior‑engineer approach outlined earlier.

---

### 1. Current UI Gaps (High‑Level Summary)

| View | **✅ Implemented** | **❌ Missing / Partial** |
|------|-------------------|---------------------------|
| **ChatView** | Message list, reactions (6 hardcoded emojis), markdown rendering, self‑destruct timer, file‑transfer UI, read‑receipt badge, context menu (edit/delete), connection banner, reconnect button, infinite scroll pagination, retention policy UI | Typing‑indicator UI (state exists but unused), dark‑theme styling (partial — system auto-detect only), full‑text per‑conversation search, extensive keyboard shortcuts (Ctrl+N, Ctrl+F, Ctrl+K, etc.), message search highlighting |
| **HubView** | Conversation list, basic filter/search, navigation to ChatView, mute/unmute conversations, delete conversation, family tab, nearby tab (LAN/DHT discovery) | Favorites, archive, drag‑and‑drop file send, keyboard navigation shortcuts (arrow keys, Delete to archive), conversation folders/tags |
| **SetupView** | Loading spinner with key generation status | Interactive onboarding tutorial (step‑by‑step wizard) |
| **VaultView** | Vault lock/unlock UI, passphrase strength meter, show/hide passphrase, passphrase tips, paste button, fingerprint hint | UX polish for dark mode (works via system), copy‑to‑clipboard feedback |
| **SettingsView** | Basic toggles (idle lock, clipboard clear, screen capture, private mode, Tor, STUN config, LAN/DHT discovery), theme selector (light/dark/system), copy IP button, STUN health indicators, Test Tor button | Accent color picker, system‑tray & background settings, auto‑update UI |
| **Global** | Notification dependency present, Tauri notification plugin configured | Native desktop notifications integration (no action buttons), system‑tray icon with menu, background keep‑alive, auto‑update UI |
| **Styling** | Light‑theme CSS (`[data-theme="light"]`), dark tokens as `:root` default, CSS variables, system preference auto-detect, manual theme toggle in Settings | WCAG‑AA contrast‑checked colors, accent‑color customization |
| **Accessibility** | Minimal ARIA labels on buttons, some `role="alert"` on toasts, `tabIndex` on conversation items | Keyboard focus order, visible focus ring (FocusRing component exists but not universally applied), ARIA live regions for typing indicator & notifications, high‑contrast mode toggle |

---

### 2. Prioritized Feature Groups

| Priority | Feature Set | Rationale |
|----------|-------------|-----------|
| **P1 – Core Messaging UX** ✅ ~70% | Emoji picker ✅, message status indicators ✅, file transfer progress bars ✅, sender labels ✅, invite countdown/ history ✅, conversation sorting ✅, last‑seen ✅. **Still missing**: Typing indicator, per‑conversation search, keyboard shortcuts (Ctrl+N, Ctrl+F, Ctrl+K), message search highlighting | Directly impacts daily chat experience; lifts UI score from 6 → 9+. |
| **P2 – Conversation Management** | Favorites / mute / archive (mute ✅), drag‑and‑drop file send, conversation folders/tags, keyboard navigation in Hub | Improves scalability for power users; pushes UI score to 10. |
| **P3 – System Integration** | Native notifications (with actions), system‑tray icon, background keep‑alive, auto‑update UI | Gives native desktop feel; required for production‑ready perception. |
| **P4 – Onboarding & Accessibility** | Interactive onboarding flow, WCAG‑AA contrast audit, full keyboard navigation, ARIA live regions | Lowers learning curve, meets accessibility standards — critical for polish. |
| **P5 – Theming & Customization** ✅ ~70% | Light/dark/system toggle in Settings ✅, manual theme toggle ✅, dark as default ✅, light theme complete with `[data-theme="light"]` ✅. **Still missing**: Accent‑color picker, user‑defined presets, export/import theme | Personalization, final polish. |

---

### 3. Implementation Roadmap (12‑Week Timeline)

#### 3.1. Foundations (Weeks 1–2)

| Task | Scope | Status | Acceptance Criteria |
|------|-------|--------|----------------------|
| Theme Architecture | Create `ThemeProvider` in `App.tsx`, manual theme toggle (light/dark/system), persist to vault/settings | ✅ Complete (`ThemeContext.tsx`, `resolvedTheme`, light/dark/system, persisted via `set_theme_preference`/`get_theme_preference`) | All components render correctly in both themes; no console warnings; toggle persists across restarts |
| Dark‑Theme Styles | Populate `src/styles/dark.css` (or extend tokens) with dark variants for colors, backgrounds, borders, scrollbars. Use CSS variables. Audit all components. | ✅ Complete (`:root` IS dark theme in `tokens.css`, `[data-theme="light"]` in `theme.css` overrides) | Visual regression test shows identical layout; colors meet WCAG‑AA contrast |
| Keyboard Shortcut Framework | Add `useHotkeys` utility. Map: `Ctrl+N` → New conversation, `Ctrl+F` → Search, `Ctrl+K` → Settings, `Ctrl+Enter` → Send (when focused), `Esc` → Back/Close. | 🟡 Partial (Esc, Ctrl+,, Ctrl+Enter exist in AppContext/ChatView) | Shortcuts work in all views without interfering with text entry |
| Typing Indicator Packet (Backend) | Extend `PacketType` enum in `protocol.rs` with `TypingIndicator`. Add serialization. Handle in `session.rs` and `commands/network.rs`. | ❌ Not started | Backend emits/receives typing packets; compilation succeeds |
| UI Hook for Typing | In `ChatView`, show banner "User is typing…" under message list when typing packet received. Auto‑hide after 3 s inactivity. | ❌ Not started (`typingPeers` state exists but unused) | Banner appears only when appropriate, disappears after 3 s |

#### 3.2. Messaging UI Enhancements (Weeks 3–4)

| Task | Scope | Status | Acceptance Criteria |
|------|-------|--------|----------------------|
| Full‑Text Search | Add search input above message list. Query backend via new `search_messages` command (filter by text, case‑insensitive, limit 50). Highlight matches in rendered markdown. | ❌ Not started (no backend command, no UI) | Searching returns correct messages; UI highlights term |
| Emoji Picker Completion | Replace placeholder reaction UI with pop‑over picker (60-emoji grid in input toolbar). Allow selecting any Unicode emoji to insert into message text. | ✅ Complete (60 emoji grid in `ChatView`, emoji picker button in input toolbar) | Emojis insert into message text; picker dismisses on outside click |
| Message Status Indicators | Show "sending" (clock icon) and "sent" (checkmark) per sent message. | ✅ Complete (msg-status system tracking in ChatView) | Status indicators shown on sent messages |
| File Transfer Progress Bars | Animated progress bar during file send/receive (% complete, speed, ETA) using existing ProgressBar component. | ✅ Complete (listens to `m2m://transfer-progress`) | Live progress updates with speed + ETA shown |
| Sender Labels (Group Chat) | Show abbreviated peer key above message when `sender_peer_key_hex` is set. | ✅ Complete (msg-sender-label CSS in utilities.css) | Group messages show sender identity |
| Keyboard Send/Cancel | Ensure `Enter` sends message, `Esc` clears input, `Ctrl+Enter` inserts newline. | 🟡 Partial (Ctrl+Enter sends, Esc goes back to hub) | All combos work consistently across platforms |
| Dark‑Mode Adaptation for Chat Bubbles | Adjust bubble colors, link colors, markdown code‑block background for dark theme. | 🟡 Partial (CSS variables exist, needs audit) | No unreadable text in dark mode |
| Accessibility – ARIA Live Region | Add `<div aria-live="polite" className="sr-only">` that announces typing indicator and new incoming messages for screen readers. | ❌ Not started | Screen readers read "Alice is typing…" and new messages |

#### 3.3. Conversation Management (Weeks 5–6)

| Task | Scope | Status | Acceptance Criteria |
|------|-------|--------|----------------------|
| Favorites / Mute / Archive | Extend `Conversation` model (Rust + TS) with `is_favorite`, `is_muted`, `archived`. Add UI toggle buttons in `HubView` list items. Update DB queries to filter archived out of default list. | 🟡 Partial (mute ✅, favorite/archive ❌) | Users can favorite, mute, archive; UI reflects state instantly |
| Folder / Tag System (optional) | Simple label field (`tags: string[]`) and UI "Add Tag" dialog. | ❌ Not started | Users can group conversations; filter by tag |
| Drag‑and‑Drop File Send | Implement `<DropZone>` component over message input area; on drop, invoke `handleSendFile`. Show preview thumbnail for images. | ❌ Not started (file send via dialog only) | Dragging a file sends it; UI shows progress bar |
| Conversation List Keyboard Navigation | Arrow‑up/down moves selection, `Enter` opens chat, `Delete` archives. | 🟡 Partial (Enter works, no arrow nav) | Keyboard navigation works without mouse |

#### 3.4. System Integration (Weeks 7–8)

| Task | Scope | Status | Acceptance Criteria |
|------|-------|--------|----------------------|
| Native Notifications | Use `tauri-plugin-notification` to show desktop alerts on incoming messages when app is backgrounded. Include actions: "Reply", "Mark read". | 🟡 Partial (basic notification works, no actions) | Notifications appear on Windows/macOS/Linux, disappear after click |
| System Tray Icon | Add tray entry with menu items: `Show`, `Lock Vault`, `Quit`. Hook into existing `handleDisconnect` for lock. | ❌ Not started (tauri.conf has tray-icon feature but no code) | Tray icon persists; clicking "Show" restores window |
| Background Keep‑Alive | Minimal keep‑alive loop that restarts Tauri window on OS sleep/wake events (using `tauri-plugin-os`). | ❌ Not started | App resumes after sleep without user action |
| Auto‑Update UI | Wire `tauri-plugin-updater` to check for updates on launch, display non‑blocking banner with "Update now". | ❌ Not started (updater not in Cargo.toml) | Update flow works; UI shows progress bar |

#### 3.5. Onboarding & Accessibility (Weeks 9–10)

| Task | Scope | Status | Acceptance Criteria |
|------|-------|--------|----------------------|
| Interactive Onboarding | Create `SetupWizard` component displayed on first launch (or via Settings). Steps: 1️⃣ Create identity, 2️⃣ Verify peer, 3️⃣ Send first message, 4️⃣ Enable dark mode. Store flag in vault. | ❌ Not started (SetupView is loading spinner only) | New users are guided; flag persists across restarts |
| WCAG Contrast Audit | Run contrast audit script, adjust any failing colors (especially dark mode). | ❌ Not started | All UI elements pass AA contrast |
| Focus Management | Ensure every modal traps focus, provide visible focus ring (`outline: 2px solid var(--color-accent)`). | 🟡 Partial (FocusRing component exists) | Keyboard users can navigate all dialogs |
| Screen‑Reader Labels | Add `aria-label` to icon‑only buttons, ensure all images have `alt` text (or `role="presentation"`). | 🟡 Partial (some labels exist) | Screen‑reader audit passes |

#### 3.6. Theming & Customization (Weeks 11–12)

| Task | Scope | Status | Acceptance Criteria |
|------|-------|--------|----------------------|
| Accent Color Picker | Add color‑picker control in Settings that updates CSS variable `--color-accent`. Persist selection in vault. | ❌ Not started | UI instantly reflects new accent; saved across restarts |
| Theme Presets | Provide "System", "Light", "Dark" options; system follows OS preference when set. | 🟡 Partial (system auto-detect only) | Switching presets updates `data-theme` attribute |
| Export / Import Theme | Allow users to export current theme JSON and import it later. | ❌ Not started | Theme JSON round‑trips correctly |

---

### 4. Milestones & Deliverables

| Milestone | Timeline | Deliverable |
|-----------|----------|-------------|
| **M1 – Theme & Shortcut Foundation** | End of Week 2 | Dark‑mode CSS, global hotkey system, typing‑indicator wire‑up (backend + UI) |
| **M2 – Messaging UI Polish** | End of Week 4 | Search bar, completed emoji picker, dark‑mode chat bubbles, ARIA live region |
| **M3 – Conversation Management** | End of Week 6 | Favorites/mute/archive UI, drag‑and‑drop file send, keyboard navigation in Hub |
| **M4 – System Integration** | End of Week 8 | Native notifications (with actions), tray icon, background keep‑alive, auto‑update UI |
| **M5 – Onboarding & Accessibility** | End of Week 10 | Setup wizard, WCAG‑AA contrast compliance, focus management, screen‑reader labels |
| **M6 – Theming & Customization** | End of Week 12 | Accent‑color picker, theme presets, export/import of theme settings |

Each milestone includes unit‑ and integration‑tests (React Testing Library + Tauri integration tests) and visual regression tests (Chromatic or Percy) to guarantee UI consistency across themes and platforms.

---

### 5. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Platform‑specific notification quirks | Users on some OS may see duplicate or missing alerts | Abstract notification logic behind a wrapper; test on Windows/macOS/Linux CI |
| Dark‑mode CSS regressions | Hidden color bugs could break contrast | Run automated WCAG audit after each CSS change; CI step fails on contrast errors |
| Keyboard shortcut collisions | Some shortcuts may conflict with OS shortcuts | Provide customizable shortcut map in Settings; fall back to defaults only if not in use |
| State sync between Rust backend and TS front‑end (e.g., favorites) | Stale UI if DB updates not propagated | Use Tauri event system (`emit`, `listen`) for real‑time updates; optimistic UI with rollback on error |
| Onboarding flow interfering with existing users | New users forced into tutorial, existing users annoyed | Detect `first_run` flag; allow user to re‑run wizard from Settings |

---

### 6. Next Steps

1. **Confirm priority order** – Adjust any feature between P1–P5 if needed.
2. **Allocate resources** – Decide how many engineers focus on front‑end vs. back‑end for the first two sprints.
3. **Set up CI checks** – Add WCAG contrast audit and visual regression steps to GitHub Actions.
4. **Kick‑off M1** – Create the theme provider and hotkey utility, and start the typing‑indicator packet work.

The plan is ready for execution. Let me know if you need a more detailed breakdown of any task or an effort estimate per milestone.