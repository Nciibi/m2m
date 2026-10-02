# EmptyStates — Implementation Prompt

## Mission

Implement all empty state components for every screen in M2M. Each empty state provides clear, contextual guidance when there is no content to display.

## Scope

Covers all empty state components including:
- ChatView (no messages): ✉️ "Start the conversation"
- Chats tab (no conversations): 💬 "No conversations yet" + CTA
- Chats tab (search no results): "No conversations found"
- Nearby tab (discovery off): "Discovery Not Active" + "Open Settings"
- Nearby tab (no peers): "No Peers Found"
- Family tab (no family): "No family members yet"

Does NOT cover: Loading states, error states (separate prompts).

## Files Expected to Be Modified

- `src/components/EmptyStates.tsx` — All empty state components
- `src/styles/components/utilities.css` — Component styles
- `src/components/ui/icons/MessageIcon.tsx` — Conversations icon
- `src/components/ui/icons/WifiIcon.tsx` — Nearby icon
- `src/components/ui/icons/HomeIcon.tsx` — Family icon

## Components to Reuse

- **Button** (Section 2.1) — CTA buttons (Get Started, Open Settings)

## Components to Create

- **ChatEmptyState** — ✉️ icon, title, description
- **ChatsEmptyState** — 💬 icon, title, description, CTA button
- **SearchEmptyState** — "No conversations found" text
- **NearbyOffState** — "Discovery Not Active" + Settings CTA
- **NearbyEmptyState** — "No Peers Found" + Refresh button
- **FamilyEmptyState** — "No family members yet"

## Design Specs (Common to All)

Each empty state follows this pattern:

```
┌──────────────────────────────────────────┐
│                                          │
│             💬 / ✉️ / 📡                │  ← 48px icon, muted color
│                                          │
│        No conversations yet               │  ← --text-lg, 600 weight
│                                          │
│   Generate an invite link to host a      │  ← --text-md, muted
│   connection, or paste an invite from    │
│   a peer to join.                        │
│                                          │
│         [Get Started]                     │  ← optional Button
│                                          │
└──────────────────────────────────────────┘
```

### Typography

- Title: --text-lg (15.2px / 0.95rem), --font-weight-semibold (600), --color-text-primary
- Description: --text-md (13.6px / 0.85rem), --color-text-muted, centered, line-height 1.5
- Icon: 48px, --color-text-muted opacity

### Spacing

- Icon to title: --space-lg (20px)
- Title to description: --space-sm (12px)
- Description to button: --space-lg (20px)
- Content max-width: 360px, centered

### Specific States

| Component | Icon | Title | Description | Action |
|-----------|------|-------|-------------|--------|
| ChatEmptyState | ✉️ (MessageIcon) | "Start the conversation" | "Send a message below to begin your encrypted conversation." | None |
| ChatsEmptyState | 💬 | "No conversations yet" | "Generate an invite link to host a connection, or paste an invite from a peer to join." | [Get Started] → Connect tab |
| SearchEmptyState | 🔍 (SearchIcon) | "No conversations found" | "Try adjusting your search terms or clear the filter." | None |
| NearbyOffState | 📡 (WifiIcon) | "Discovery Not Active" | "Enable LAN or DHT discovery in Settings to find nearby peers." | [Open Settings] |
| NearbyEmptyState | 📡 | "No Peers Found" | "No LAN peers detected. Make sure other M2M users are on the same network." | [Refresh] |
| FamilyEmptyState | 🏠 (HomeIcon) | "No family members yet" | "Add trusted peers as family members for quick access." | None |

### Colors

- Icon: --color-text-muted at 0.5 opacity (or CSS opacity)
- Title: --color-text-primary
- Description: --color-text-muted

### Accessibility

- Each empty state: role="status"
- Icons: aria-hidden="true" (decorative)
- Buttons: descriptive aria-label

### Animations

- Fade in on mount: fadeIn 300ms ease-out-expo

## Acceptance Criteria

- [ ] ChatView empty state shows ✉️ 48px icon + "Start the conversation" text
- [ ] Chats tab empty shows 💬 + "No conversations yet" + [Get Started]
- [ ] Search empty shows "No conversations found" + hint
- [ ] Nearby (discovery off) shows "Discovery Not Active" + [Open Settings]
- [ ] Nearby (no peers) shows "No Peers Found" + [Refresh]
- [ ] Family tab empty shows "No family members yet"
- [ ] All icons 48px, muted color
- [ ] All titles --text-lg 600 weight
- [ ] All descriptions --text-md muted
- [ ] Fade-in animation on mount (300ms)
- [ ] role="status" on each container
- [ ] aria-hidden="true" on decorative icons

## Self-Review Checklist

- [ ] Follows Design Bible Section 9.1 exactly
- [ ] Each empty state matches spec
- [ ] CSS custom properties used
- [ ] i18n strings match catalog
