# M2M UI/UX Upgrade Implementation Plan

## Overview
This plan details the gaps that prevent the app from achieving a **10/10** UI/UX rating and provides a concrete, prioritized roadmap to close those gaps. It follows the senior‑engineer approach outlined earlier.

---

### 1. Current UI Gaps (High‑Level Summary)
| View | Implemented | Missing / Partial |
|------|-------------|-------------------|
| **ChatView** | Message list, reactions, markdown, self‑destruct timer, file‑transfer UI, read‑receipt badge, context menu, edit/delete, connection banner, reconnect button | Typing‑indicator UI, dark‑theme styling, full‑text per‑conversation search, complete emoji picker, extensive keyboard shortcuts (Ctrl+N, Ctrl+F, Ctrl+K, etc.) |
| **HubView** | Conversation list, basic filter, navigation to ChatView | Favorites / mute / archive, drag‑and‑drop file send, keyboard navigation shortcuts |
| **SetupView** | Loading spinner only | Interactive onboarding tutorial (step‑by‑step) |
| **VaultView** | Vault lock/unlock UI, basic list of stored keys | UX polish for dark mode, copy‑to‑clipboard feedback |
| **SettingsView** | Basic toggles (idle lock, clipboard clear) | Theme & accent color picker, system‑tray & background settings |
| **Global** | Notification dependency present | Native desktop notifications integration, system‑tray icon with menu, background keep‑alive, auto‑update UI |
| **Styling** | Light‑theme CSS exists | Full dark‑theme stylesheet (`[data-theme="dark"]` rules), WCAG‑AA contrast‑checked colors, accent‑color customization |
| **Accessibility** | Minimal ARIA labels on buttons | Keyboard focus order, visible focus ring, ARIA live regions for typing indicator & notifications, high‑contrast mode toggle |

---

### 2. Prioritized Feature Groups
| Priority | Feature Set | Rationale |
|----------|-------------|-----------|
| **P1 – Core Messaging UX** | Typing indicator, per‑conversation search, dark‑theme, keyboard shortcuts, complete emoji picker | Directly impacts daily chat experience; lifts UI score from 6 → 9+. |
| **P2 – Conversation Management** | Favorites / mute / archive, drag‑and‑drop file send, conversation folders | Improves scalability for power users; pushes UI score to 10. |
| **P3 – System Integration** | Native notifications, system‑tray icon, background keep‑alive, auto‑update UI | Gives a native feel on desktop; required for a production‑ready perception. |
| **P4 – Onboarding & Accessibility** | Interactive onboarding flow, WCAG‑AA contrast audit, full keyboard navigation, ARIA live regions | Lowers learning curve, meets accessibility standards—critical for a polished product. |
| **P5 – Theming & Customization** | Accent‑color picker, dark‑mode toggle in settings, user‑defined theme presets | Personalization, final polish. |

---

### 3. Implementation Roadmap (12‑Week Timeline)
#### 3.1. Foundations (Weeks 1–2)
| Task | Scope | Owner | Acceptance Criteria |
|------|-------|-------|----------------------|
| Theme Architecture | Create a CSS‑in‑JS or PostCSS system that switches based on `data-theme` attribute (`light` / `dark`). Add a global `ThemeProvider` in `App.tsx`. | Front‑end dev | All existing components render correctly in both themes; no console warnings. |
| Dark‑Theme Styles | Populate `src/styles/dark.css` with dark variants for colors, backgrounds, borders, scrollbars. Use CSS variables for easy theming. | Front‑end dev | Visual regression test shows identical layout; colors meet WCAG‑AA contrast. |
| Keyboard Shortcut Framework | Add a small utility (`useHotkeys`) that registers global shortcuts. Map: `Ctrl+N` → New conversation, `Ctrl+F` → Search, `Ctrl+K` → Settings, `Ctrl+Enter` → Send (when focused). | Front‑end dev | Shortcuts work in all views without interfering with text entry. |
| Typing Indicator Packet | Extend `PacketType` enum with `TypingIndicator` and serialization in `protocol.rs`. Add handling in `session.rs` and `commands/network.rs`. | Backend dev | Backend emits/receives typing packets; compilation succeeds. |
| UI Hook for Typing | In `ChatView`, show a banner “User is typing…” under the message list when a typing packet is received. | Front‑end dev | Banner appears only when appropriate, disappears after 3 s of inactivity. |

#### 3.2. Messaging UI Enhancements (Weeks 3–4)
| Task | Scope | Owner | Acceptance Criteria |
|------|-------|-------|----------------------|
| Full‑Text Search | Add a search input above the message list. Query backend via a new `search_messages` command (filter by text, case‑insensitive, limit 50). Highlight matches in rendered markdown. | Full‑stack dev | Searching returns correct messages; UI highlights term. |
| Emoji Picker Completion | Replace placeholder reaction UI with a pop‑over picker (e.g., `emoji-mart`). Allow selecting any Unicode emoji, send via `handleSendReaction`. | Front‑end dev | Reactions display as inline emoji badges; picker dismisses on outside click. |
| Keyboard Send/Cancel | Ensure `Enter` sends message, `Esc` clears input, `Ctrl+Enter` inserts newline. | Front‑end dev | All combos work consistently across platforms. |
| Dark‑Mode Adaptation for Chat Bubbles | Adjust bubble colors, link colors, markdown code‑block background for dark theme. | Front‑end dev | No unreadable text in dark mode. |
| Accessibility – ARIA Live Region | Add `<div aria-live="polite" className="sr-only">` that announces typing indicator and new incoming messages for screen readers. | Accessibility specialist | Screen readers read “Alice is typing…”. |

#### 3.3. Conversation Management (Weeks 5–6)
| Task | Scope | Owner | Acceptance Criteria |
|------|-------|-------|----------------------|
| Favorites / Mute / Archive | Extend `Conversation` model (Rust + TS) with `is_favorite`, `is_muted`, `archived`. Add UI toggle buttons in `HubView` list items. Update DB queries to filter archived out of default list. | Backend & Front‑end | Users can favorite, mute, archive; UI reflects state instantly. |
| Folder / Tag System (optional) | Simple label field (`tags: string[]`) and UI “Add Tag” dialog. | Front‑end dev (optional) | Users can group conversations; filter by tag. |
| Drag‑and‑Drop File Send | Implement `<DropZone>` component over the message input area; on drop, invoke `handleSendFile`. Show a preview thumbnail for images. | Front‑end dev | Dragging a file sends it; UI shows progress bar. |
| Conversation List Keyboard Navigation | Arrow‑up/down moves selection, `Enter` opens chat, `Delete` archives. | Front‑end dev | Keyboard navigation works without mouse. |

#### 3.4. System Integration (Weeks 7–8)
| Task | Scope | Owner | Acceptance Criteria |
|------|-------|-------|----------------------|
| Native Notifications | Use `tauri-plugin-notification` to show desktop alerts on incoming messages when app is backgrounded. Include actions: “Reply”, “Mark read”. | Backend dev | Notifications appear on Windows/macOS/Linux, disappear after click. |
| System Tray Icon | Add a tray entry with menu items: `Show`, `Lock Vault`, `Quit`. Hook into existing `handleDisconnect` for lock. | Backend dev | Tray icon persists; clicking “Show” restores window. |
| Background Keep‑Alive | Minimal keep‑alive loop that restarts the Tauri window on OS sleep/wake events (using `tauri-plugin-os`). | Backend dev | App resumes after sleep without user action. |
| Auto‑Update UI | Wire `tauri-plugin-updater` to check for updates on launch, display a non‑blocking banner with “Update now”. | Backend dev | Update flow works; UI shows progress bar. |

#### 3.5. Onboarding & Accessibility (Weeks 9–10)
| Task | Scope | Owner | Acceptance Criteria |
|------|-------|-------|----------------------|
| Interactive Onboarding | Create a `SetupWizard` component displayed on first launch (or via Settings). Steps: 1️⃣ Create identity, 2️⃣ Verify peer, 3️⃣ Send first message, 4️⃣ Enable dark mode. Store a flag in the vault. | Front‑end dev | New users are guided; flag persists across restarts. |
| WCAG Contrast Audit | Run the `docs/wcag-contrast-audit.md` script, adjust any failing colors (especially dark mode). | Designer + Front‑end dev | All UI elements pass AA contrast. |
| Focus Management | Ensure every modal traps focus, provide a visible focus ring (`outline: 2px solid var(--color-accent)`). | Accessibility specialist | Keyboard users can navigate all dialogs. |
| Screen‑Reader Labels | Add `aria-label` to icon‑only buttons, ensure all images have `alt` text (or `role="presentation"`). | Accessibility specialist | Screen‑reader audit passes. |

#### 3.6. Theming & Customization (Weeks 11–12)
| Task | Scope | Owner | Acceptance Criteria |
|------|-------|-------|----------------------|
| Accent Color Picker | Add a color‑picker control in Settings that updates a CSS variable `--color-accent`. Persist selection in the vault. | Front‑end dev | UI instantly reflects new accent; saved across restarts. |
| Theme Presets | Provide “System”, “Light”, “Dark” options; system follows OS preference when set. | Front‑end dev | Switching presets updates `data-theme` attribute. |
| Export / Import Theme | Allow users to export current theme JSON and import it later. | Front‑end dev | Theme JSON round‑trips correctly. |

---

### 4. Milestones & Deliverables
| Milestone | Timeline | Deliverable |
|-----------|----------|-------------|
| **M1 – Theme & Shortcut Foundation** | End of Week 2 | Dark‑mode CSS, global hotkey system, typing‑indicator wire‑up (backend + UI). |
| **M2 – Messaging UI Polish** | End of Week 4 | Search bar, completed emoji picker, dark‑mode chat bubbles, ARIA live region. |
| **M3 – Conversation Management** | End of Week 6 | Favorites/mute/archive UI, drag‑and‑drop file send, keyboard navigation in Hub. |
| **M4 – System Integration** | End of Week 8 | Native notifications, tray icon, background keep‑alive, auto‑update UI. |
| **M5 – Onboarding & Accessibility** | End of Week 10 | Setup wizard, WCAG‑AA contrast compliance, focus management, screen‑reader labels. |
| **M6 – Theming & Customization** | End of Week 12 | Accent‑color picker, theme presets, export/import of theme settings. |

Each milestone includes unit‑ and integration‑tests (React Testing Library + Tauri integration tests) and visual regression tests (Chromatic or Percy) to guarantee UI consistency across themes and platforms.

---

### 5. Risks & Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| Platform‑specific notification quirks | Users on some OS may see duplicate or missing alerts. | Abstract notification logic behind a wrapper; test on Windows/macOS/Linux CI. |
| Dark‑mode CSS regressions | Hidden color bugs could break contrast. | Run automated WCAG audit after each CSS change; CI step fails on contrast errors. |
| Keyboard shortcut collisions | Some shortcuts may conflict with OS shortcuts. | Provide a customizable shortcut map in Settings; fall back to defaults only if not in use. |
| State sync between Rust backend and TS front‑end (e.g., favorites) | Stale UI if DB updates not propagated. | Use Tauri event system (`emit`, `listen`) for real‑time updates; optimistic UI with rollback on error. |
| Onboarding flow interfering with existing users | New users forced into tutorial, existing users annoyed. | Detect `first_run` flag; allow user to re‑run wizard from Settings. |

---

### 6. Next Steps
1. **Confirm priority order** – Adjust any feature between P1–P5 if needed.  
2. **Allocate resources** – Decide how many engineers will focus on front‑end vs. back‑end for the first two sprints.  
3. **Set up CI checks** – Add the WCAG contrast audit and visual regression steps to GitHub Actions.  
4. **Kick‑off M1** – Create the theme provider and hotkey utility, and start the typing‑indicator packet work.

The plan is ready for execution. Let me know if you need a more detailed breakdown of any task or an effort estimate per milestone.
