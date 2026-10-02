# ContextMenus — Implementation Prompt

## Mission

Implement the context menu component for right-click interactions on message bubbles. The context menu provides actions like Edit and Delete with proper positioning, animation, and focus management.

## Scope

Covers the context menu component including:
- Right-click trigger on message bubbles
- Menu positioning (below bubble, aligned to outer edge)
- Menu items with hover states and danger styling
- Fade-in animation (100ms)
- Keyboard navigation (arrows, Enter/Space, Escape)
- Click-outside-to-close behavior

Does NOT cover: Specific menu actions (Edit, Delete) — those are handled by the parent ChatView.

## Files Expected to Be Modified

- `src/components/MessageContextMenu.tsx` — Component
- `src/styles/components/utilities.css` — Component styles

## Components to Reuse

- **Button** (Section 2.1) — Menu items (ghost variant)

## Layout Hierarchy

```
<MessageContextMenu
  open={isOpen}
  position={position}
  alignment={direction}       <!-- 'sent' or 'received' -->
  onClose={handleClose}
>
  <div class="msg-context-menu" role="menu">
    <button class="context-menu__item" role="menuitem" onClick={handleEdit}>
      Edit
    </button>
    <button class="context-menu__item context-menu__item--danger" role="menuitem" onClick={handleDelete}>
      Delete
    </button>
  </div>
</MessageContextMenu>
```

## Design Implementation Requirements

### Specs

From Design Bible Sections 2.11 & 11.6:

- Position: below the message bubble
- Alignment: right-aligned for sent messages, left-aligned for received
- Min-width: 120px
- Background: --color-bg-elevated
- Border: 1px --color-border-default
- Border-radius: --radius-md (12px)
- Shadow: --shadow-lg
- Items: 32px height, padding --space-xs (8px) --space-md (16px)
- Danger items (Delete): --color-danger text
- Animation: fadeIn 100ms (opacity 0→1)

### States

| Element | Default | Hover |
|---------|---------|-------|
| Menu item | bg: transparent, text: primary | bg: --color-bg-hover |
| Menu item (danger) | bg: transparent, text: danger | bg: --color-danger-bg, text: danger |

### Keyboard Navigation

From Design Bible Section 13.2:

| Key | Action |
|-----|--------|
| ArrowUp | Previous item |
| ArrowDown | Next item |
| Enter/Space | Activate selected item |
| Escape | Close menu |
| Click outside | Close menu |

### Focus Trap

- First menu item receives focus on open
- Arrow keys cycle through items
- Escape returns focus to the message bubble
- Tab is not used (Arrow keys for menu navigation)

### Positioning Logic

```
Menu position calculation:
- Below the bubble: top = bubble.bottom + 4px
- Sent bubble (right-aligned): right = bubble.right
- Received bubble (left-aligned): left = bubble.left
- If menu would overflow viewport: flip to above
- If menu would overflow horizontally: flip alignment
```

### Window resize: close the menu

### Accessibility

- role="menu" on container
- role="menuitem" on each item
- aria-label="Message actions" on menu
- Focus management: first item focused on open
- onClose returns focus to trigger element

## Acceptance Criteria

- [ ] Right-click on message bubble opens context menu
- [ ] Menu positioned below bubble, aligned correctly (sent=right, received=left)
- [ ] Edit item with ghost styling
- [ ] Delete item with danger color
- [ ] Menu items 32px height with correct padding
- [ ] Hover state changes background
- [ ] Fade-in animation 100ms
- [ ] ArrowUp/Down navigates items
- [ ] Enter/Space activates selected item
- [ ] Escape closes menu
- [ ] Click outside closes menu
- [ ] Window resize closes menu
- [ ] Focus returns to message bubble on close
- [ ] role="menu", role="menuitem" applied

## Self-Review Checklist

- [ ] Follows Design Bible Sections 2.11 and 11.6
- [ ] Positioning logic handles viewport edges
- [ ] Keyboard navigation complete
- [ ] Focus trap implemented
