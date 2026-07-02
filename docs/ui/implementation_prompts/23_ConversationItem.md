# ConversationItem — Implementation Prompt

## Mission

Implement the ConversationItem component for displaying a single conversation row in the Chats tab conversation list. This component handles display states, hover-reveal actions, and sorting metadata.

## Scope

Covers the ConversationItem component including:
- Layout: avatar, name, time, preview, online dot
- Hover-reveal action buttons (favorite, archive, mute, delete)
- Sorting-related display (favorite star state)
- States: default, hover, active, selected, archived

Does NOT cover: The ChatsTab conversation list container, backend operations for actions.

## Files Expected to Be Modified

- `src/components/ConversationItem.tsx` — Main component
- `src/styles/components/utilities.css` — Component styles

## Components to Reuse

- **Badge** (Section 2.5) — Online dot, notification count
- **Button** (Section 2.1) — Action buttons (icon variant, 28×28px)

## Components to Create

- **Avatar** — Initials avatar with gradient background from hashToColor()

## Layout Hierarchy

```
<div class="conv-item" role="listitem">
  <!-- Avatar -->
  <div class="conv-item__avatar">
    <div class="conv-avatar" style={{ background: hashToColor(name) }}>
      AB
    </div>
    <OnlineDot visible={online} />          <!-- 8px dot, top-right -->
  </div>

  <!-- Body -->
  <div class="conv-item__body">
    <div class="conv-item__top">
      <span class="conv-item__name">Alice</span>
      <span class="conv-item__time">2m ago</span>
    </div>
    <div class="conv-item__preview">
      Hey, are you there?
    </div>
  </div>

  <!-- Action Buttons (hover-reveal) -->
  <div class="conv-item__actions">
    <Button icon size="xs" aria-label={favorite ? "Remove from favorites" : "Add to favorites"}>
      {favorite ? <StarFilledIcon /> : <StarIcon />}
    </Button>
    <Button icon size="xs" aria-label={archived ? "Unarchive" : "Archive"}>
      {archived ? <FolderOpenIcon /> : <FolderIcon />}
    </Button>
    <Button icon size="xs" aria-label={muted ? "Unmute notifications" : "Mute notifications"}>
      {muted ? <BellOffIcon /> : <BellIcon />}
    </Button>
    <Button icon size="xs" aria-label="Delete conversation">
      <TrashIcon />
    </Button>
  </div>
</div>
```

## Design Implementation Requirements

### Exact Pixel Layout

From Design Bible Sections 2.10 & 12.5:

```
┌──────────────────────────────────────────────────────────┐
│  ← 20px padding → ┌──────┐ ← 16px gap → ┌────────────┐  │
│                   │  AB  │               │ Alice       │  │
│                   │ 48px │               │             │  │
│                   │      │               │ 2m ago      │  │
│                   └──────┘               │             │  │
│                                          │ Hey, are... │  │
│  ● = 8px green dot at top-right of avatar               │
│                                                          │
│  height: 64px (8 × 8px grid)                             │
│  Internal padding: 16px top/bottom, 20px left/right      │
└──────────────────────────────────────────────────────────┘
```

### Avatar Spec

- Size: 48×48px
- Border-radius: --radius-lg (14px)
- Font: 20px, 700 weight, white, uppercase initials
- Background: dynamic gradient from hashToColor(name)
- Online dot: 8px, 2px white border, --radius-full, positioned top-right of avatar
  - Online: --color-success (#10b981)
  - Offline: --color-text-muted (hidden by default)

### Typography

- Name: --text-md (13.6px / 0.85rem), --font-weight-semibold (600)
- Time: --text-xs (10.4px), --color-text-muted, right-aligned
- Preview: --text-sm (11.5px), --color-text-secondary, single-line truncated (overflow: hidden, text-overflow: ellipsis, white-space: nowrap)

### States

From Design Bible Section 11.5:

| State | Background | Transform | Shadow |
|-------|-----------|-----------|--------|
| Default | rgba(255,255,255,0.02) | translateY(0) | none |
| Hover | rgba(255,255,255,0.05) | translateY(-2px) | shadow-md + accent-glow |
| Active | — | translateY(-1px) | — |
| Selected | accent-glow-subtle | translateY(0) | border: 1px border-accent |

Transition: all 150ms ease-out-expo

### Hover-Reveal Actions

From Design Bible Section 11.5:
- Opacity: 0 → 1 over 150ms
- 50ms stagger per button (left to right)
- Each button: 28×28px, --radius-xs
- Star active (favorited): gold fill (#f59e0b)

### Colors

- Favorite active: #f59e0b (gold)
- Archive: --color-text-muted
- Mute (muted): --color-danger
- Delete: --color-danger

### Animations

| Animation | Duration | Easing | Trigger |
|-----------|----------|--------|---------|
| btnHover | 150ms | ease-out-expo | Item hover (translateY + shadow) |
| stagger | 50ms per button | — | Action buttons appear |
| scale | 100ms | ease-out-expo | Action button hover (scale 1.2) |

### Accessibility

- Conversation item: role="listitem" + role="button"
- aria-label: "Conversation with {name}"
- Action buttons: aria-label with descriptive text
- Online dot: aria-label="Online" or "Offline"
- Focus ring via :focus-visible on item

## Acceptance Criteria

- [ ] Avatar shows initials on gradient background (48×48px)
- [ ] Name, time, and preview display correctly
- [ ] Preview truncated to single line with ellipsis
- [ ] Online dot visible at top-right of avatar (8px green dot, 2px white border)
- [ ] Hover state: translateY(-2px), background changes, shadow appears
- [ ] Hover reveals 4 action buttons with stagger animation
- [ ] Favorite star active: gold fill, inactive: outline
- [ ] Each action button has aria-label
- [ ] Selected state: accent-glow-subtle background
- [ ] Height: 64px, padding: 16px 20px
- [ ] All spacing on 8px grid
- [ ] Transition: 150ms ease-out-expo

## Self-Review Checklist

- [ ] Follows Design Bible Sections 2.10, 11.5, 12.5
- [ ] Avatar gradient from hashToColor()
- [ ] All spacing matches pixel specs
- [ ] Hover-reveal stagger works correctly
