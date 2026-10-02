# Shortcuts — Implementation Prompt

## Mission

Implement the keyboard shortcuts system for M2M, including a shortcut help modal that displays all available keyboard shortcuts organized by context. All shortcuts are documented and accessible.

## Scope

Covers keyboard shortcuts including:
- Global shortcuts: Tab, Shift+Tab, Enter, Space, Esc, Ctrl+K, Ctrl+,, Ctrl+N, ?
- ChatView shortcuts: Ctrl+Enter send, Shift+Enter newline, Esc back, Ctrl+F search
- Conversation list: ArrowUp/ArrowDown navigate
- Modal/Context menu/Emoji picker keyboard navigation
- Shortcut help modal (toggle with ?)
- Keyboard shortcut badge components

Does NOT cover: Actual shortcut handler implementation in each view (handled by individual view prompts).

## Files Expected to Be Modified

- `src/components/ShortcutsModal.tsx` — Shortcut help modal
- `src/components/ShortcutHint.tsx` — Inline shortcut hint badges
- `src/hooks/useKeyboard.ts` — Global keyboard handler hook
- `src/styles/components/utilities.css` — Component styles

## Components to Reuse

- **Modal** (Section 2.4) — Shortcut help dialog shell
- **Badge** (Section 2.5) — Keyboard key representation

## Components to Create

- **ShortcutsModal** — ?-toggleable modal with all shortcuts
- **ShortcutHint** — Inline "Ctrl+Enter" style badges
- **ShortcutSection** — Grouped shortcut list with header

## All Shortcuts

From Design Bible Sections 6.4 & 13.2:

### Global

| Key | Action |
|-----|--------|
| Tab | Move to next focusable element |
| Shift+Tab | Move to previous focusable element |
| Enter | Activate focused element |
| Space | Toggle focused checkbox/switch |
| Ctrl+K | Open Settings |
| Ctrl+, | Open Settings |
| Ctrl+N | Switch to Connect tab |
| ? | Toggle shortcut help modal |

### ChatView

| Key | Action |
|-----|--------|
| Ctrl+Enter | Send message |
| Shift+Enter | New line in textarea |
| Escape | Back to Hub (when input empty) |
| Ctrl+F | Toggle search overlay |
| Ctrl+K | Open Settings |

### Conversation List (Chats tab)

| Key | Action |
|-----|--------|
| ArrowUp | Previous conversation |
| ArrowDown | Next conversation |
| Enter | Open selected conversation |

### Modal

| Key | Action |
|-----|--------|
| Tab | Cycle forward through elements |
| Shift+Tab | Cycle backward through elements |
| Escape | Close modal |
| Enter | Activate focused button |

### Context Menu

| Key | Action |
|-----|--------|
| ArrowUp | Previous item |
| ArrowDown | Next item |
| Enter/Space | Activate selected item |
| Escape | Close menu |

### Emoji Picker

| Key | Action |
|-----|--------|
| Arrow keys | Navigate emoji grid |
| Enter | Select emoji |
| Escape | Close picker |

### Tab Bar

| Key | Action |
|-----|--------|
| Left/Right Arrow | Switch between tabs |
| Enter/Space | Activate selected tab |

## Layout Hierarchy

```
<ShortcutsModal>
  <div class="shortcuts-modal">
    <h2>Keyboard Shortcuts</h2>

    <ShortcutSection title="Global">
      <div class="shortcut-row">
        <span class="shortcut-row__action">Open Settings</span>
        <div class="shortcut-row__keys">
          <kbd>Ctrl</kbd> + <kbd>K</kbd>
        </div>
      </div>
      <div class="shortcut-row">
        <span class="shortcut-row__action">Open Connect tab</span>
        <div class="shortcut-row__keys">
          <kbd>Ctrl</kbd> + <kbd>N</kbd>
        </div>
      </div>
      <!-- ... -->
    </ShortcutSection>

    <ShortcutSection title="ChatView">
      <div class="shortcut-row">
        <span class="shortcut-row__action">Send message</span>
        <div class="shortcut-row__keys">
          <kbd>Ctrl</kbd> + <kbd>Enter</kbd>
        </div>
      </div>
      <!-- ... -->
    </ShortcutSection>
  </div>
</ShortcutsModal>
```

## Design Specs

### Shortcut Row Layout

```
┌──────────────────────────────────────────┐
│  Action text (flex: 1)     [Ctrl] + [K]  │
│  font: 13px, primary       font: 11px    │
│  400 weight                mono, badge   │
└──────────────────────────────────────────┘
```

- Row height: 32px
- Gap: 12px between action and keys
- Bottom divider: 1px border-default

### Keyboard Badge (kbd)

- Background: --color-bg-input
- Border: 1px --color-border-default
- Border-radius: --radius-xs (4px)
- Padding: 2px 6px
- Font: --text-xs, --font-mono
- Gap between modifier + key: 4px

### Inline Shortcut Hints

In footer text areas (ChatView):
```
Ctrl+Enter to send  ·  Esc to go back
```
- Font: --text-xs (10px), --color-text-muted
- "Ctrl+Enter" and "Esc" can use the badge styling

## Accessibility

- Shortcuts modal: toggle with ? key
- Modal has role="dialog", aria-modal="true"
- Focus trap active in modal
- All shortcut keys shown visually
- Shortcuts work with keyboard-only navigation
- Focus ring visible on keyboard nav only (:focus-visible)

## Acceptance Criteria

- [ ] ? key toggles shortcut help modal
- [ ] Modal shows all shortcuts organized by section
- [ ] Global section shows Tab, Shift+Tab, Enter, Esc, Ctrl+K, Ctrl+N, ?
- [ ] ChatView section shows Ctrl+Enter, Shift+Enter, Esc, Ctrl+F
- [ ] Conversation list section shows ArrowUp/Down, Enter
- [ ] Modal section shows Tab, Shift+Tab, Esc
- [ ] Context menu section shows ArrowUp/Down, Enter, Esc
- [ ] Emoji picker section shows Arrow keys, Enter, Esc
- [ ] Shortcut keys displayed as kbd badges
- [ ] Shortcut hints shown inline (ChatView footer: "Ctrl+Enter to send")
- [ ] Focus visible on keyboard navigation only
- [ ] Modal has proper ARIA attributes
- [ ] Modal closes on Escape or click outside

## Self-Review Checklist

- [ ] Follows Design Bible Sections 6.4 and 13.2 exactly
- [ ] All shortcuts from both tables included
- [ ] Shortcut badges styled as kbd elements
- [ ] Modal specs from Section 2.4
