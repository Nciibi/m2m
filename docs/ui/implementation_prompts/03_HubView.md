# HubView — Implementation Prompt

## Mission

Implement the HubView, the central navigation hub of M2M. This view contains the app header, tab bar with 4 tabs (Connect, Chats, Nearby, Family), and the content area that switches between tab views. The HubView provides the main interface for connecting to peers, managing conversations, discovering nearby users, and accessing family contacts.

## Scope

Covers the HubView shell including:
- App header: M2M logo, connection badge, settings gear
- Tab bar: 4 tabs with active indicator and badge counts
- Tab content switching between ConnectTab, ChatsTab, NearbyTab, FamilyTab
- Responsive layout at desktop, tablet, and mobile breakpoints

Does NOT cover: Individual tab content (each tab has its own prompt), settings navigation (SettingsView has its own prompt), app shell container (handled by AppShell component).

## Files Expected to Be Modified

- `src/views/HubView.tsx` — Main component
- `src/styles/layout.css` — Header and tab bar styles
- `src/components/ui/icons/MessageIcon.tsx` — Conversations icon
- `src/components/ui/icons/GlobeIcon.tsx` — DHT/nearby icon
- `src/components/ui/icons/HomeIcon.tsx` — Family icon
- `src/components/ui/icons/LinkIcon.tsx` — Connect icon
- `src/components/ui/icons/GearIcon.tsx` — Settings icon
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **Badge** (Section 2.5) — Connection status badge in header, notification counts on tabs
- **Button** (Section 2.1) — Icon button for settings gear, tab buttons
- **OnlineDot/OfflineDot** (Section 15.2) — Connection status indicator

## Components to Create

- **TabBar** — 4-tab navigation bar with active indicator and badge counts
- **AppHeader** — M2M logo, title, connection status, settings gear
- **ConnectTab** — (separate prompt 04)
- **ChatsTab** — (separate prompt 05)
- **NearbyTab** — (separate prompt 06)
- **FamilyTab** — (separate prompt 07)

## Layout Hierarchy

```
<HubView>
  <AppHeader>
    <div class="header">
      <div class="header__logo">
        <img class="header__logo-img" />      <!-- 20×20px rounded -->
        <span class="header__title">M2M</span> <!-- --text-lg, bold -->
      </div>
      <div class="header__right">
        <Badge variant="success" dot>Online</Badge>  <!-- connection status -->
        <Button variant="icon" aria-label="Settings">
          <GearIcon />
        </Button>
      </div>
    </div>
  </AppHeader>

  <TabBar>
    <div class="tab-bar" role="tablist">
      <button class="tab-bar__tab tab-bar__tab--active" role="tab" aria-selected="true">
        <LinkIcon /><span>Connect</span>
      </button>
      <button class="tab-bar__tab" role="tab" aria-selected="false">
        <MessageIcon /><span>Chats</span><Badge count={3} />
      </button>
      <button class="tab-bar__tab" role="tab" aria-selected="false">
        <WifiIcon /><span>Nearby</span>
      </button>
      <button class="tab-bar__tab" role="tab" aria-selected="false">
        <HomeIcon /><span>Family</span>
      </button>
    </div>
  </TabBar>

  <div class="tab-content">
    {activeTab === 'connect' && <ConnectTab />}
    {activeTab === 'chats' && <ChatsTab />}
    {activeTab === 'nearby' && <NearbyTab />}
    {activeTab === 'family' && <FamilyTab />}
  </div>
</HubView>
```

## Design Implementation Requirements

### Exact Spacing

From Design Bible Sections 10.2 & 12.4-12.5:

**Header (52px height):**
- Left padding: 24px (from app-shell edge)
- Right padding: 16px
- Logo area: 32×32px icon container, `--radius-sm`, accent gradient
- Title text: 24px from logo, `--text-lg` (15.2px), `--font-weight-bold`
- Connection badge: 22px height, right-aligned, 8px from settings gear
- Settings gear: 32×32px icon button, rightmost
- Bottom border: 1px solid `--color-border-default`

**Tab bar (44px height):**
- Tab padding: 12px horizontal, 10px vertical
- Tab gap: 4px between tabs
- Active indicator: 2px bottom border, `--color-accent`, full tab width
- Badge: 18px height, `--radius-full`, `--color-accent` bg, white text `--text-xs`
- Badge position: 6px right of tab text, vertically centered

**Content area:**
- Desktop (>1000px): 32px horizontal padding
- Tablet (600-1000px): 24px horizontal padding
- Mobile (<600px): 16px horizontal padding

### Typography

- Header title "M2M": `--text-lg` (0.95rem / 15.2px), `--font-weight-bold` (700)
- Tab text: `--text-sm` (0.72rem / 11.5px), `--font-weight-medium` (500)
- Active tab text: `--color-text-primary`, inactive: `--color-text-secondary`
- Badge count: `--text-xs` (0.65rem / 10.4px), `--font-weight-semibold` (600)

### Colors

- Header bg: transparent (uses app-shell glass surface)
- Header text: `--color-text-primary`
- Header border: 1px `--color-border-default` at bottom
- Tab text default: `--color-text-secondary`
- Tab text active: `--color-text-primary`
- Tab active indicator: `--color-accent` (2px)
- Tab hover: `--color-bg-hover`
- Badge bg: `--color-accent`
- Badge text: white
- Settings gear: `--color-text-secondary`

### Glass Effects

- Header inherits from app-shell: `background: var(--color-bg-surface)`, `backdrop-filter: var(--glass-blur)`
- Edge light: `linear-gradient(90deg, transparent, rgba(255,255,255,0.08), transparent)` at top of header

### Shadows

- App shell: `--shadow-app-shell`

### Icons

From Icon Catalog (Section 15):
- Logo: M2M branded icon (20×20px)
- Tab icons: LinkIcon (Connect), MessageIcon (Chats), WifiIcon (Nearby), HomeIcon (Family) — 16px each
- Settings gear: GearIcon (24px → icon button 32×32px container)

## States

### Hover States

- Settings gear icon: `rotate(30deg)` over 200ms
- Tab: background tint on hover (`--color-bg-hover`)
- Connection badge (if clickable): subtle scale

### Focus States

- Settings gear: focus ring via `:focus-visible`
- Tab buttons: focus ring via `:focus-visible`

### Active States

- Settings gear: scale(0.95)
- Tab: pressed state (scale 0.98)

### Disabled States

Not applicable (all elements always enabled — connection badge may show offline state).

### Loading States

- Connection badge shows "Connecting…" during initial connection
- Tab content shows individual loading states per tab

### Empty States

HubView itself doesn't have an empty state (the shell is always visible). Individual tabs handle their own empty states.

### Error States

- Connection state error: badge shows "Disconnected" in red with `--color-danger` dot
- Network diagnostic failures handled by individual tabs

## Animations

| Animation | Duration | Easing | Property | Trigger |
|-----------|----------|--------|----------|---------|
| `appEntrance` | 800ms | ease-out-expo | translateY + opacity | App mount (once) |
| `slideInRight` | 500ms | ease-out-expo | translateX | Tab switch |
| `btnHover` | 150ms | ease-out-expo | transform + box-shadow | Settings gear hover |
| `pulseRing` | 2s | ease-in-out | scale + opacity | Online dot pulse |

**Tab switch animation**: Content slides in from right when switching tabs (500ms, ease-out-expo). Previous content slides out to left.

## Keyboard Shortcuts

From Design Bible Sections 6.4 & 13.2:

| Key | Context | Action |
|-----|---------|--------|
| Tab | HubView | Move through header → tabs → content |
| Left/Right Arrow | Tab bar | Switch between tabs |
| Enter/Space | Tab bar | Activate selected tab |
| Ctrl+N | Global | Switch to Connect tab |
| Ctrl+K | Global | Open Settings |
| Ctrl+, | Global | Open Settings |
| ? | Global | Toggle shortcut help |

## Mouse Interactions

From Design Bible Section 13.1:

| Element | Hover | Click |
|---------|-------|-------|
| Settings gear | rotate(30deg) 200ms | Navigate to SettingsView |
| Tab | Bottom border transition | Switch tab |
| Connection badge | Subtle scale | (no action — informational) |

## Interactions

- **Tab switching**: Clicking a tab switches content immediately with slide animation
- **Settings navigation**: Click gear icon → navigate to SettingsView
- **Connection badge**: Reflects real-time connection state via `m2m://connection` event
- **Badge counts**: Updated via `m2m://message` and `m2m://connection` events
- **Horizontal scroll on mobile**: Tab bar scrolls horizontally on small screens (<600px)

## Accessibility

From Design Bible Sections 6 & 16:

- Tab bar: `role="tablist"`
- Each tab: `role="tab"`, `aria-selected="true/false"`, `aria-controls` pointing to tab panel
- Tab panel: `role="tabpanel"`, `aria-labelledby` pointing to tab
- Settings gear (icon-only): `aria-label="Settings"`
- Connection badge: `role="status"` (live region for connection changes)
- Logo: `role="presentation"` or `aria-hidden="true"` (decorative)
- Focus order: Settings gear → Connect tab → Chats tab → Nearby tab → Family tab → content
- Focus ring visible via `:focus-visible` on all interactive elements

**Focus order (HubView):**
1. Settings gear (top-right)
2. Connect tab button
3. Chats tab button
4. Nearby tab button
5. Family tab button
6. First content item in active tab

## Responsive Behavior

From Design Bible Sections 7 & 17:

**Desktop (>1000px):**
- App shell: 1000px max-width, centered, 94vh height (max 800px), border-radius 32px
- Header padding: 24px left, 16px right
- Content padding: 32px horizontal

**Tablet (600-1000px):**
- App shell: 100% width, 100vh, no border-radius, no shadow
- Header padding: 14px 20px
- Content padding: 24px horizontal

**Mobile (<600px):**
- App shell: 100dvh, no border-radius
- Header: 44px height, padding 10px 16px
- Tab bar: 40px height, overflow-x auto, white-space nowrap
- Tab padding: 8px 12px, font-size 12px
- Content padding: 16px horizontal

## Performance Considerations

From Design Bible Section 20:

- Conversation list load (50 items): < 300ms
- Tab switch: < 100ms (animation 500ms but content should be ready immediately)
- DOM nodes: < 2000 (for full loaded state)
- Event listeners: < 100 active at any time
- No layout shifts on tab switch (fixed height container)

## Security Considerations

From Design Bible Sections 8 & 38:

- Connection badge shows only connected/disconnected — no granular status
- Conversation previews truncated to prevent shoulder surfing
- Archived conversations hidden from main list
- Settings gear is behind authentication (vault must be unlocked)
- Mute suppresses notifications

## Edge Cases

From Design Bible Sections 9 & 32:

- **All tabs show empty states**: Possible on first launch — each tab handles its own empty state
- **Rapid tab switching**: Animation queue should handle rapid clicks (use cancellation)
- **Settings navigation while in tab content**: Safe — settings is a separate view
- **Connection badge flickering**: Debounce connection state changes (100ms)
- **No conversations + no discovery + no family**: Show CTAs on each tab to guide user
- **Tab bar overflow on mobile**: Horizontal scroll with -webkit-overflow-scrolling: touch
- **Badge count overflow**: Badge showing "99+" for large counts (cap at 99+)

## Acceptance Criteria

- [ ] Header shows M2M logo (20×20px), title, connection badge, settings gear
- [ ] Header height is exactly 52px with correct padding
- [ ] Connection badge shows Online/Offline/Connecting states with appropriate colors
- [ ] Settings gear animates rotate(30deg) on hover
- [ ] Tab bar shows 4 tabs with correct icons and labels
- [ ] Tab bar height is exactly 44px
- [ ] Active tab has 2px accent bottom border indicator
- [ ] Badge counts shown on Chats tab (and update in real-time)
- [ ] Tab switching animates with slideInRight
- [ ] Tab content area shows correct component for active tab
- [ ] tablist/tab/tabpanel ARIA roles correctly applied
- [ ] aria-selected updates on tab switch
- [ ] Keyboard navigation works (arrows, Tab, Enter)
- [ ] Responsive at desktop (1000px max), tablet (full-bleed), mobile (full-bleed + scroll tabs)
- [ ] All animations respect prefers-reduced-motion
- [ ] Settings gear has aria-label="Settings"
- [ ] i18n strings match string catalog

## Self-Review Checklist

- [ ] Does the layout match the pixel specs in Design Bible Sections 10.2 & 12.4-12.5?
- [ ] Are all spacing values multiples of the 4px grid?
- [ ] Are all CSS custom properties from the token system used?
- [ ] Are animations using only transform and opacity?
- [ ] Is prefers-reduced-motion respected?
- [ ] Are all ARIA roles/attributes correctly applied (tablist, tab, tabpanel)?
- [ ] Does focus order follow the spec?
- [ ] Does the component handle tab switching correctly?
- [ ] Are keyboard interactions implemented?
- [ ] No redesign, no improvisation, no invented layouts — follows Design Bible exactly
