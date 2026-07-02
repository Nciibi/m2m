# Implementation Prompt: Emoji Picker Component

## 1. Mission
Implement a fixed-size emoji picker that displays 64 emoji characters in an 8-column grid with smooth open/close animations. This picker appears above the message input when toggled and supports both mouse and keyboard navigation with proper accessibility.

## 2. Scope
- 8-column grid layout with 64 emoji characters
- Open/close animations with fade-in and scale transform
- Keyboard navigation (arrow keys, Enter to select, Escape to close)
- Click-outside detection to auto-close
- Toggle button to open/close picker
- Hover effects on emoji cells

## 3. Files Expected to Be Modified
- `src/components/ui/EmojiPicker.tsx` — Main component
- `src/components/ChatView/MessageInput.tsx` — Parent that contains emoji button
- `src/styles/components/emoji-picker.css` — Styling and animations

## 4. Components to Reuse
- None (emoji picker is self-contained)

## 5. Components to Create
- `EmojiPicker` — Container and grid
- `EmojiCell` — Individual emoji button (role="gridcell")
- `EmojiPickerContent` — Inner grid wrapper

## 6. Layout Hierarchy (ASCII art)

```
┌─────────────────────────────────────────────────┐
│ Emoji Picker (above input, 8px gap)             │
│ ┌───────────────────────────────────────────┐   │
│ │ [😀] [😁] [😂] [🤣] [😊] [😉] [😍] [🥰] │   │  ← 8 columns
│ │ [😘] [😜] [😎] [🤩] [👍] [👎] [✌️] [🤞] │   │  ← gap: 2px
│ │ [👊] [💪] [🙌] [👏] [🤝] [🔥] [⭐] [💯] │   │  ← button: 32×32px
│ │ [❤️] [🧡] [💛] [💚] [💙] [💜] [🖤] [🤍] │   │
│ │ [💔] [💖] [✨] [🎉] [🙏] [💀] [☠️] [👋] │   │
│ │ [🫂] [🤗] [😤] [😭] [😱] [🤔] [🙄] [😴] │   │
│ │ [✅] [❌] [❗] [❓] [➕] [➖] [🚀] [🎂]  │   │
│ │ [🎁] [💰] [🔒] [🔓]                      │   │
│ └───────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

## 7. Design Implementation Requirements

### Exact Spacing
- Grid columns: 8
- Gap between cells: 2px
- Padding inside picker: 8px all sides
- Position: Above emoji button, 8px vertical gap
- Fixed width: 280px (calculated: 8 × 32px + 7 × 2px + 8px × 2 padding)

### Typography
- N/A (emoji only, no text labels)

### Colors
- Background: `--color-bg-elevated`
- Border: 1px `--color-border-default`
- Hover background: `--color-bg-hover`
- No text color (emoji is visual only)

### Glass Effects
- `backdrop-filter: var(--glass-blur-sm)` on container
- Border radius: `--radius-md` (12px)

### Shadows
- `--shadow-lg` for depth

### Icons
- 64 specific emoji from Design Bible spec (see Assets section below)

## 8. States

### Closed
- Container hidden (display: none or opacity: 0)
- Emoji button shows unpressed state

### Opening
- Animation: `fadeIn` 150ms + `scale` (0.95 → 1.0)
- Easing: `var(--ease-out-expo)`

### Open
- All emoji visible and interactive
- Hover: cell background becomes `--color-bg-hover`, scale(1.3)
- Focus: cell receives keyboard focus ring (3px `--color-accent-glow` outline)

### Closing
- Reverse animations (fade out + scale down)
- Duration: 150ms
- Emoji no longer interactive

## 9. Animations

| Animation | Duration | Property | Trigger |
|-----------|----------|----------|---------|
| `fadeIn` | 150ms | opacity (0 → 1) | Open picker |
| `scale` | 150ms | transform (scale 0.95 → 1.0) | Open picker |
| `cellHover` | 100ms | scale + background | Hover on cell |
| `fadeOut` | 150ms | opacity (1 → 0) | Close picker |
| `scaleDown` | 150ms | transform (scale 1.0 → 0.95) | Close picker |

All animations use `--ease-out-expo` cubic-bezier.

## 10. Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Arrow Up` | Move focus up one cell (wraps to bottom) |
| `Arrow Down` | Move focus down one cell (wraps to top) |
| `Arrow Left` | Move focus left one cell (wraps to right) |
| `Arrow Right` | Move focus right one cell (wraps to left) |
| `Enter` / `Space` | Select focused emoji, close picker |
| `Escape` | Close picker without selection |
| `Home` | Move focus to first emoji |
| `End` | Move focus to last emoji |

## 11. Mouse Interactions

| Interaction | Behavior |
|-------------|----------|
| Hover over emoji | `scale(1.3)` + `background: --color-bg-hover`, 100ms |
| Click emoji | Select emoji, close picker, insert into message input |
| Click outside picker | Close picker without selection |

## 12. Interactions

- **Emoji Selection**: When user clicks or presses Enter on an emoji, emit event with selected emoji character. Parent component inserts emoji into message text.
- **Auto-Close**: Close picker when emoji is selected, when Escape pressed, or when click occurs outside.
- **Dismiss**: All close actions should reset grid focus to first cell for next open.

## 13. Accessibility

**ARIA attributes**:
- Container: `role="grid"`, `aria-label="Emoji picker"`, `aria-hidden` when closed
- Each emoji cell: `role="gridcell"`, `aria-label="emoji name"` (e.g., "grinning face")
- Keyboard navigation: arrow keys navigate cells within grid
- Focus management: First cell auto-focused on open, Shift+Tab/Tab cycle only within picker

**Screen reader**:
- Announce "Emoji picker" on open
- Announce selected emoji name on selection

## 14. Responsive Behavior

**Desktop (> 1000px)**:
- Fixed width: 280px
- Position: absolute, above emoji button
- No scroll (all 64 emoji fit in 8×8 grid)

**Tablet/Mobile (<1000px)**:
- Same width and layout (icon buttons are sized appropriately)
- Ensure picker doesn't exceed screen width with margin adjustments

## 15. Performance

- Emoji picker is a controlled component
- Grid cells render via `.map()` over emoji array
- No animation frame drops: use `transform` and `opacity` only
- Lazy-load emoji picker component (code-split from main)

## 16. Security

- No user input in emoji picker — static emoji list
- No XSS risk from emoji characters

## 17. Edge Cases

- **More than 8 columns on mobile**: Adjust grid to 6 columns, show 2 rows with 6 emoji each (future enhancement)
- **No emoji selected**: Close picker on Escape, return to input without change
- **Rapid clicking**: Debounce emoji selection to prevent double-insert
- **Focus loss during navigation**: Reset to first cell if focus leaves picker while open

## 18. Acceptance Criteria

- [ ] 8-column grid displays all 64 emoji without overflow
- [ ] Hover effect (scale + background) works smoothly
- [ ] Arrow key navigation cycles through all emoji in order
- [ ] Enter or Space selects focused emoji
- [ ] Escape closes picker without selection
- [ ] Click outside closes picker
- [ ] Open animation (150ms fade + scale) is smooth
- [ ] Close animation (150ms reverse) is smooth
- [ ] All emoji are keyboard-navigable
- [ ] ARIA attributes satisfy automated accessibility audit
- [ ] No focus trap (Escape releases focus correctly)
- [ ] Selected emoji is inserted into parent input
- [ ] Grid focuses on first cell when reopened

## 19. Self-Review Checklist

- [ ] Component accepts `isOpen` prop and `onClose`, `onSelect` callbacks
- [ ] All 64 emoji from Design Bible are hardcoded in array
- [ ] Focus trap works: Tab/Shift+Tab cycles within picker
- [ ] Hover state uses `--color-bg-hover`
- [ ] Focus state uses 3px `--color-accent-glow` outline
- [ ] Animations use CSS keyframes with `will-change: transform`
- [ ] `prefers-reduced-motion` respected (disable animations)
- [ ] Grid has role="grid" and each cell has role="gridcell"
- [ ] aria-label on container and each cell
- [ ] No memory leaks: cleanup listeners on unmount
- [ ] Click-outside detection doesn't interfere with parent
- [ ] Emoji picker doesn't cause layout shift (fixed position)

---

## Assets

**64 Emoji from spec**:
```
😀 😁 😂 🤣 😊 😉 😍 🥰
😘 😜 😎 🤩 👍 👎 ✌️ 🤞
👊 💪 🙌 👏 🤝 🔥 ⭐ 💯
❤️ 🧡 💛 💚 💙 💜 🖤 🤍
💔 💖 ✨ 🎉 🙏 💀 ☠️ 👋
🫂 🤗 😤 😭 😱 🤔 🙄 😴
✅ ❌ ❗ ❓ ➕ ➖ 🚀 🎂
🎁 💰 🔒 🔓
```
