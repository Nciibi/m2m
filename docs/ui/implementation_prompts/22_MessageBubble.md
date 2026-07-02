# MessageBubble — Implementation Prompt

## Mission

Implement the MessageBubble component for displaying individual encrypted messages in ChatView. This component handles sent and received messages with status indicators, reactions, context menus, and animations.

## Scope

Covers the MessageBubble component including:
- Sent message bubble (right-aligned, accent gradient)
- Received message bubble (left-aligned, elevated bg)
- Status indicators (⏳ sending, ✓ sent, ✓✓ delivered, ✓✓ bright read)
- Timestamp, edited badge, self-destruct timer in footer
- Reactions row below bubble (pill buttons)
- Context menu on right-click (Edit/Delete)
- Message entry animations (msgSlide 400ms, msgReceived 500ms)
- Group sender label support

Does NOT cover: The ChatView message list container, emoji picker, full context menu (see prompt 24).

## Files Expected to Be Modified

- `src/components/MessageBubble.tsx` — Main component
- `src/styles/components/utilities.css` — Component styles
- `src/components/ui/icons/CheckIcon.tsx` — Sent status
- `src/components/ui/icons/CheckDoubleIcon.tsx` — Delivered/read status
- `src/components/ui/icons/ClockIcon.tsx` — Sending status

## Components to Reuse

- **Badge** (Section 2.5) — Edited badge, self-destruct timer, reaction counts

## Components to Create

- **MessageStatus** — Status icon + position in footer
- **MessageFooter** — Timestamp + status + edited + timer row
- **ReactionRow** — Pill buttons below bubble
- **MessageContextMenu** — Right-click menu (see also prompt 24)

## Layout Hierarchy

**Sent Bubble:**
```
<div class="msg-bubble msg-bubble--sent">
  <div class="msg-bubble__body">Hello! How are you?</div>
  <div class="msg-bubble__footer">
    <span class="msg-time">12:30 PM</span>
    <span class="msg-edited" hidden>edited</span>
    <span class="msg-timer" hidden>🔥 0:30</span>
    <CheckDoubleIcon size={12} />        <!-- or ClockIcon, CheckIcon -->
  </div>
</div>
```

**Received Bubble:**
```
<div class="msg-bubble msg-bubble--received">
  <div class="msg-bubble__sender" hidden>Alice</div>    <!-- group only -->
  <div class="msg-bubble__body">I'm doing great!</div>
  <div class="msg-bubble__footer">
    <span class="msg-time">12:31 PM</span>
  </div>
</div>
```

**Reactions:**
```
<div class="msg-reactions">
  <button class="reaction-pill">👍 2</button>
  <button class="reaction-pill reaction-pill--self">❤️ 1</button>
  <button class="reaction-pill">😂 3</button>
</div>
```

## Design Implementation Requirements

### Exact Specs

From Design Bible Section 2.11:

- Max-width: 75% of container
- Padding: --space-sm (12px) --space-md (16px)
- Border-radius: --radius-lg (18px)
- Bottom corner: 4px (opposite direction of alignment)
  - Sent: border-bottom-right-radius: 4px
  - Received: border-bottom-left-radius: 4px
- Gap between consecutive same-sender: --space-xxs (4px)
- Gap between different-sender: --space-sm (12px)

**Sent bubble:**
- Background: --color-accent-gradient
- Text: white
- Shadow: --shadow-bubble-sent (0 4px 15px rgba(99,102,241,0.3))
- Footer: white at 0.7 opacity

**Received bubble:**
- Background: --color-bg-elevated
- Text: --color-text-primary
- Shadow: --shadow-bubble-received (0 4px 15px rgba(0,0,0,0.2))
- Footer: --color-text-secondary at 0.5 opacity

### Typography

- Message body: --text-md (0.85rem / 13.6px)
- Timestamp: --text-xs (0.65rem / 10.4px)
- Edited badge: --text-xs, italic, --color-text-muted
- Self-destruct timer: --text-xs, --color-warning font
- Reaction pill text: --text-xs (10px)
- Sender label (group): --text-xs, --color-text-accent

### Status Indicators

| State | Icon | Size | Color |
|-------|------|------|-------|
| SENDING | ClockIcon | 10px | --color-text-muted |
| SENT | CheckIcon | 12px | --color-text-muted |
| DELIVERED | CheckDoubleIcon | 12px | --color-accent-bright |
| READ | CheckDoubleIcon | 12px | --color-accent (bright solid) |

### Colors

- Self-reaction: accent border + tinted background
- Context menu danger items: --color-danger

### Animations

| Animation | Duration | Easing | Property | Trigger |
|-----------|----------|--------|----------|---------|
| msgSlide | 400ms | ease-out-expo | translateY + opacity | Sent message |
| msgReceived | 500ms | ease-out-expo | translateY + opacity + box-shadow | Received message |
| stagger | i × 50ms | — | animation-delay | Consecutive (max 500ms) |

**Sent animation keyframes:**
- 0ms: opacity 0, translateY(8px)
- 50ms: opacity 0.3, translateY(4px)
- 200ms: opacity 0.8, translateY(1px)
- 400ms: opacity 1, translateY(0)
- Shadow: none → shadow-bubble-sent over 400ms

**Received animation keyframes:**
- 0ms: opacity 0, translateY(10px), box-shadow glow
- 100ms: box-shadow expands
- 300ms: opacity 0.9, translateY(2px)
- 500ms: opacity 1, translateY(0), shadow-bubble-received

### Interactions

From Design Bible Section 13.1:

| Element | Hover | Click | Right-click |
|---------|-------|-------|-------------|
| Message bubble | Show reaction picker after 500ms | N/A | Show context menu |
| Reaction pill | scale(1.1) | Toggle reaction | N/A |
| Avatar (in bubble) | scale(1.05) | Peer profile | N/A |

### Context Menu

From Section 2.11 and 11.6:
- Position: below bubble, aligned to outer edge (sent=right, received=left)
- Min-width: 120px, border-radius md, shadow lg
- Items: Edit (ghost), Delete (danger color)
- Animation: fadeIn 100ms

### Accessibility

- Message bubble: aria-label with sender name + message preview
- Status icons: aria-label="Message sent/delivered/read"
- Time: no special label needed
- Reactions: aria-label="{emoji} with {count} reactions"
- Context menu: role="menu", role="menuitem"

## Acceptance Criteria

- [ ] Sent bubble: right-aligned, accent gradient bg, white text, shadow-bubble-sent
- [ ] Received bubble: left-aligned, elevated bg, text-primary, shadow-bubble-received
- [ ] Correct border-radius (lg + 4px opposite corner)
- [ ] Status icons show correct state (sending → sent → delivered → read)
- [ ] Timestamp shown in footer (--text-xs)
- [ ] Edited badge shown when message is edited
- [ ] Self-destruct timer shown when active (🔥 M:SS)
- [ ] Reactions row below bubble as pill buttons
- [ ] Self-reaction has accent highlight
- [ ] Hover shows reaction picker after 500ms
- [ ] Right-click shows context menu with Edit/Delete
- [ ] Sent message animation: msgSlide 400ms
- [ ] Received message animation: msgReceived 500ms
- [ ] Consecutive messages stagger by 50ms
- [ ] Group chat shows sender label on received messages
- [ ] Max-width 75% of container
- [ ] All ARIA labels applied
- [ ] prefers-reduced-motion respected

## Self-Review Checklist

- [ ] Follows Design Bible Sections 2.11 and 11.6 exactly
- [ ] Status state machine correct (sending → sent → delivered → read)
- [ ] Animations use transform/opacity only
- [ ] All spacing on 4px grid
- [ ] CSS custom properties used
