# ChatView — Implementation Prompt

## Mission

Implement the ChatView, the primary messaging interface for encrypted 1:1 conversations. This view handles sending/receiving messages, file transfers, reactions, message editing/deletion, typing indicators, and session management.

## Scope

Covers the full ChatView including:
- Header: shield icon, encryption status, hub back, online dot, disconnect
- Session banner with encryption info
- Message list with sent/received bubbles, date separators, reactions
- File request banners and transfer progress
- Typing indicator with animated dots
- Ctrl+F search bar overlay
- Input area: attach, emoji, textarea, timer select, send button
- Drop zone drag-and-drop overlay
- Scroll-to-bottom FAB
- All connection states and error handling

Does NOT cover: Groups (separate prompt), individual component details (see MessageBubble, EmojiPicker, etc.).

## Files Expected to Be Modified

- `src/views/ChatView.tsx` — Main component
- `src/styles/components/utilities.css` — View-specific styles
- `src/components/ui/icons/ShieldIcon.tsx` — Security indicator
- `src/components/ui/icons/ArrowLeftIcon.tsx` — Back navigation
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **MessageBubble** (prompt 22) — Sent/received message display
- **Button** (Section 2.1) — Send, attach, emoji, disconnect, reconnect, accept/reject
- **Input** (Section 2.2) — Search bar (compact variant)
- **LoadingSpinner** (Section 2.7) — Inline spinner
- **ProgressBar** (Section 2.8) — File transfer progress
- **Badge** (Section 2.5) — Connection status, reconnection attempts
- **EmojiPicker** (prompt 21) — Emoji selection
- **TypingIndicator** (Section 2.13) — Peer typing animation

## Components to Create

- **ChatHeader** — Shield, status, back, disconnect
- **SessionBanner** — Encryption info with 🔒 icon and fingerprint
- **DateSeparator** — "─── Today ───" styled divider
- **MessageList** — Scrollable list with infinite scroll
- **FileRequestBanner** — Incoming file accept/reject
- **FileTransferProgress** — Progress bar with speed/ETA
- **SearchOverlay** — Ctrl+F toggled search (also see prompt 19)
- **ChatInput** — Textarea with toolbar (attach, emoji, send)
- **DropZone** — Drag-and-drop overlay with dashed border
- **ScrollToBottomFAB** — Floating scroll button
- **RetentionPolicyBar** — Expiration/export controls

## Layout Hierarchy

From Design Bible Section 12.6:

```
<ChatView>
  <!-- Header: Y=0, height 52px -->
  <ChatHeader>
    <div class="chat-header">
      <ShieldIcon />                  <!-- 🛡 gray or ✅ green -->
      <span>Encrypted Session</span>
      <div class="chat-header__right">
        <Button icon aria-label="Back to conversations"><ArrowLeftIcon /></Button>
        <OnlineDot />                  <!-- connection status -->
        <Button variant="danger" sm>Disconnect</Button>
      </div>
    </div>
  </ChatHeader>

  <!-- File Request Banner: Y=52, height 52px -->
  <FileRequestBanner>
    <div class="file-req">
      <FileIcon />
      <span>report.pdf</span>
      <span>2.4 MB</span>
      <Button variant="default" sm>Accept</Button>
      <Button variant="ghost" sm>Reject</Button>
    </div>
  </FileRequestBanner>

  <!-- File Transfer Progress: Y=104, height 72px -->
  <FileTransferProgress>
    <div class="file-progress">
      <FileIcon />
      <span>photo.jpg</span>
      <span>4.2 MB</span>
      <ProgressBar value={65} variant="default" />
      <span>2.1 MB/s · 12s remaining</span>
    </div>
  </FileTransferProgress>

  <!-- Search Bar: Y=176, height 40px (Ctrl+F toggle) -->
  <SearchOverlay visible={showSearch}>
    <Input placeholder="Search messages… (Esc)" icon={<SearchIcon />} />
    <span>3 results</span>
  </SearchOverlay>

  <!-- Typing Indicator: Y=216, height 28px -->
  <TypingIndicator visible={peerTyping}>
    <div class="typing-indicator">
      <span>●</span><span>●</span><span>●</span>
      <span>Peer is typing…</span>
    </div>
  </TypingIndicator>

  <!-- Message Area: Y=244, flex:1, overflow-y:auto -->
  <MessageList>
    <!-- Session Banner -->
    <SessionBanner>
      <LockIcon size={22} />
      <p>End-to-end encrypted session established.</p>
      <p class="fingerprint">a1b2c3d4e5f6...</p>
    </SessionBanner>

    <DateSeparator text="Today" />

    <MessageBubble direction="sent" status="read" time="12:30 PM">
      Hey, how are you?
    </MessageBubble>

    <MessageBubble direction="received" time="12:31 PM">
      I'm doing great! You?
      <Reactions reactions={[{emoji: "👍", count: 1}, {emoji: "❤️", count: 1}]} />
    </MessageBubble>

    <DateSeparator text="Yesterday" />

    <MessageBubble direction="sent" status="delivered" time="9:15 PM">
      See you tomorrow!
    </MessageBubble>

    <!-- Loading older messages -->
    <div class="msg-loading">Loading older messages…</div>
    <!-- Beginning -->
    <div class="msg-beginning">Beginning of conversation</div>
    <!-- Empty state -->
    <div class="msg-empty">
      <MessageIcon size={48} />
      <h3>Start the conversation</h3>
      <p>Send a message below to begin your encrypted conversation.</p>
    </div>
  </MessageList>

  <!-- Drop Zone (visible on drag) -->
  <DropZone>
    <div class="drop-zone">
      <p>Drop files here to send</p>
    </div>
  </DropZone>

  <!-- Scroll-to-bottom FAB -->
  <ScrollToBottomFAB visible={scrolledUp}>
    <Button variant="icon"><ArrowDownIcon /></Button>
  </ScrollToBottomFAB>

  <!-- Retention Policy Bar -->
  <RetentionPolicyBar>
    <Select options={["No Expiration", "1 Hour", "1 Day", "1 Week"]} compact />
    <Button variant="ghost" sm>Export</Button>
  </RetentionPolicyBar>

  <!-- Input Area: Y=672, auto height (42-120px) -->
  <ChatInput>
    <div class="chat-input">
      <Button icon aria-label="Attach a file"><AttachIcon /></Button>
      <Button icon aria-label="Add emoji"><SmileyIcon /></Button>
      <textarea placeholder="Type a secure message…" rows={1} />
      <Select compact options={["Off","5s","30s","1m","5m","1h","24h"]} />
      <Button variant="default" aria-label="Send message"><SendIcon /></Button>
    </div>
  </ChatInput>

  <!-- Footer: Y=720, height 24px -->
  <div class="chat-footer">
    <span>End-to-end encrypted</span>
    <span>Ctrl+Enter to send · Esc to go back</span>
  </div>
</ChatView>
```

## Design Implementation Requirements

### Exact Spacing

From Design Bible Section 12.6:
- Header: 52px, padding 0 32px
- File request: 52px, padding 8px 32px
- File progress: 72px, padding 8px 32px
- Search bar: 40px, padding 8px 32px
- Typing indicator: 28px, padding 4px 32px
- Message area: flex 1, padding 0 32px, gap between bubbles: 8px
- Session banner: full-width within padding
- Date separator: centered, horizontal lines on both sides
- Input area: auto height (42px min, 120px max with text), padding 16px 32px
- Footer: 24px, padding 4px 32px
- FAB: 40×40px, bottom 80px, right 32px

### Typography

- Header "Encrypted Session": --text-sm, --color-text-secondary
- Message text: --text-md (0.85rem / 13.6px)
- Timestamp: --text-xs (0.65rem / 10.4px)
- Date separator: --text-xs, --color-text-muted
- Input placeholder: --text-base, --color-text-placeholder
- Footer: --text-xs (10px), --color-text-muted
- Session banner: --text-sm, --color-text-muted
- Fingerprint in session banner: --text-xs, --font-mono

### Colors

- Sent bubble bg: --color-accent-gradient
- Sent bubble text: white
- Received bubble bg: --color-bg-elevated
- Received bubble text: --color-text-primary
- Session banner bg: --color-bg-card
- Input area bg: transparent, border-top: 1px --color-border-default
- DIsconnected input: background overlay, "Cannot send while disconnected" text
- Reconnecting badge: --color-warning

### Glass Effects

- Input area: transparent (inherits app-shell glass)

### Shadows

- Sent bubble: --shadow-bubble-sent
- Received bubble: --shadow-bubble-received
- Input area: none (flush with bottom)
- FAB: --shadow-md

### Icons

- ShieldIcon — Security (gray unverified, green verified)
- LockIcon — Session encryption (22px in banner)
- ArrowLeftIcon — Back to hub
- OnlineDot / OfflineDot — Connection status
- AttachIcon — 📎 file attach
- SmileyIcon — 😊 emoji picker
- SendIcon — ➤ send message
- SearchIcon — 🔍 search
- ArrowDownIcon — ⬇ scroll to bottom
- FileIcon — 📄 file transfer

## States

### Connection States

| State | Header Visual | Input | Behavior |
|-------|--------------|-------|----------|
| Connected | Shield (gray/green), green dot | Enabled, send button accent | Normal operation |
| Disconnected (verified) | Red dot, "Reconnect" button | Disabled | Can attempt reconnect |
| Disconnected (unverified) | Navigate to hub automatically | — | No reconnect |
| Reconnecting | Badge "Reconnecting (2/5)…" | Disabled | Exponential backoff |

## Animations

| Animation | Duration | Easing | Trigger |
|-----------|----------|--------|---------|
| msgSlide | 400ms | ease-out-expo | Message sent (translateY + opacity) |
| msgReceived | 500ms | ease-out-expo | Message received (translateY + opacity + glow) |
| stagger | i × 50ms | — | Consecutive messages |
| fadeIn | 150ms | ease-out-expo | File request/progress banners |
| fabAppear | 300ms | ease-out-expo | Scroll FAB |
| slideInRight | 500ms | ease-out-expo | View entrance |

## Keyboard Shortcuts

From Design Bible Sections 6.4 & 13.2:

| Key | Action |
|-----|--------|
| Ctrl+Enter | Send message |
| Shift+Enter | New line in textarea |
| Escape | Back to hub (when input empty) |
| Ctrl+F | Toggle search bar |
| Ctrl+K | Open settings |
| Tab | Cycle through input elements |

## Interactions

- **Send**: Ctrl+Enter or click send → message encrypts + sends → status: sending → sent → delivered → read
- **Newline**: Shift+Enter in textarea
- **Auto-grow textarea**: Grows up to 120px max, then scrolls
- **Infinite scroll**: Scroll up → load older messages (30 at a time)
- **Reactions**: Hover message → reaction picker after 500ms → click emoji
- **Right-click message**: Context menu (Edit/Delete)
- **Click shield**: Open fingerprint verification modal
- **Click peer name**: Open peer profile modal
- **Drag file**: Drop zone overlay appears → drop to send
- **Reconnect**: Click "Reconnect" button → exponential backoff (1s, 2s, 4s, 8s, 16s)
- **Self-destruct**: Select timer → countdown shown on sent message

## Accessibility

- Message area: aria-live="polite" for new messages
- Send button: aria-label="Send message (Ctrl+Enter)"
- Attach button: aria-label="Attach a file"
- Emoji button: aria-label="Add emoji"
- Disconnect: aria-label="Disconnect from peer"
- Reconnect: aria-label="Attempt to reconnect"
- Each message bubble: aria-label with sender + preview
- File request: role="alert"
- Session banner: role="status"
- Focus order: Header → file banners → search → messages (read-only) → FAB → attach → emoji → textarea → timer → send

## Responsive Behavior

- **Desktop (>1000px)**: 32px horizontal padding, max-width 75% for bubbles
- **Tablet (600-1000px)**: 24px padding, same layout
- **Mobile (<600px)**: 16px padding, max-width 85% for bubbles, reduced header height (44px)

## Performance Considerations

- Message list load (100 messages): < 500ms
- Virtual scrolling not needed (< 200 messages visible)
- React.memo on message bubbles (re-render only when content changes)
- Stagger animation max: 500ms (10 messages)
- will-change: transform on bubbles during animation

## Security Considerations

- Self-destruct timer per message (5s-24h)
- Read receipts show when messages read
- E2EE via X3DH + Double Ratchet (indicated in session banner)
- Screen capture protection (Windows)
- No persistent notification content
- Disconnect button available to end session explicitly

## Edge Cases

- **Send while disconnected**: Warning shown, message queued for delivery
- **Message too long (64KB)**: Warning at 90%, block at 100%
- **Edit window expired (24h)**: Toast warning
- **Delete already deleted message**: Toast info
- **Rapid sends**: Queue messages, maintain order
- **Slow connection**: Message shows ⏳ "sending" until confirmed

## Acceptance Criteria

- [ ] Header shows shield icon with correct state and "Encrypted Session" label
- [ ] Back button navigates to HubView (Esc also works)
- [ ] Connection dot shows online/offline/reconnecting states
- [ ] Message list displays sent (right) and received (left) bubbles
- [ ] Bubbles use correct colors, shadows, and border-radius
- [ ] Timestamps and status icons shown on all messages
- [ ] Date separators between messages on different days
- [ ] Reactions shown below bubbles as pill buttons
- [ ] Right-click shows context menu (Edit/Delete)
- [ ] Input area has attach, emoji, textarea, timer, send
- [ ] Textarea auto-grows from 42px to 120px
- [ ] Ctrl+Enter sends, Shift+Enter newline
- [ ] Escape returns to hub (when input empty)
- [ ] Ctrl+F toggles search overlay
- [ ] Drag-and-drop shows drop zone overlay
- [ ] File request banner appears with Accept/Reject
- [ ] File transfer progress shows with speed/ETA
- [ ] Typing indicator shows animated dots
- [ ] Reconnect button shows on verified peer disconnect
- [ ] Scroll-to-bottom FAB appears when scrolled up
- [ ] Session banner shows encryption info
- [ ] All states handled (connected, disconnected, reconnecting, sending)
- [ ] Animations match spec (msgSlide 400ms, msgReceived 500ms)
- [ ] Responsive at all breakpoints
- [ ] Accessibility ARIA attributes applied

## Self-Review Checklist

- [ ] Follows Design Bible Sections 3.4 and 12.6 exactly
- [ ] Y-coordinate layout matches spec exactly
- [ ] All spacing on 4px grid
- [ ] CSS custom properties used throughout
- [ ] All states handled
- [ ] Keyboard shortcuts implemented
- [ ] Animations use transform/opacity only
- [ ] prefers-reduced-motion respected
- [ ] i18n strings match catalog
