# GroupChatView — Implementation Prompt

## Mission

Implement the Group Chat View for encrypted group conversations. This view is similar to ChatView but adapted for multiple participants with sender labels, group member management, and Sender Key encryption indicators.

## Scope

Covers the Group Chat View including:
- Group header with group name, member count, and back button
- Message list with sender labels on received messages
- Support for group-specific reactions and context menus
- Group member management actions (info panel accessible from header)

Does NOT cover: 1:1 ChatView (separate prompt), GroupInfoPanel (separate prompt), group creation modal, Sender Key encryption backend.

## Files Expected to Be Modified

- `src/views/GroupChatView.tsx` — Main component
- `src/styles/components/utilities.css` — View-specific styles
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **MessageBubble** (prompt 22) — With sender label variant
- **Button** (Section 2.1) — Send, back, info actions
- **Input** (Section 2.2) — Message textarea
- **LoadingSpinner** (Section 2.7) — Loading states
- **Badge** (Section 2.5) — Member count, status
- **TypingIndicator** (Section 2.13) — Multi-peer typing

## Components to Create

- **GroupChatHeader** — Group name + member count + back + info
- **GroupSenderLabel** — Sender name above received bubble

## Layout Hierarchy

```
<GroupChatView>
  <!-- Header -->
  <GroupChatHeader>
    <Button icon><ArrowLeftIcon /></Button>
    <div>
      <span>Group Name</span>
      <Badge>3 members</Badge>
    </div>
    <Button icon aria-label="Group info"><InfoIcon /></Button>
  </GroupChatHeader>

  <!-- Message List -->
  <MessageList>
    <DateSeparator text="Today" />

    <!-- Received with sender label -->
    <GroupSenderLabel name="Alice" />
    <MessageBubble direction="received">
      Hey everyone!
    </MessageBubble>

    <!-- Sent -->
    <MessageBubble direction="sent" status="read">
      Hi Alice!
    </MessageBubble>

    <!-- Received with reactions -->
    <GroupSenderLabel name="Bob" />
    <MessageBubble direction="received">
      How's it going?
      <Reactions reactions={[{emoji: "👍", count: 2}]} />
    </MessageBubble>
  </MessageList>

  <!-- Input Area (same as ChatView) -->
  <ChatInput />
</GroupChatView>
```

## Design Implementation Requirements

### Typography

- Group name: --text-md, --font-weight-semibold
- Member count: --text-xs, --color-text-muted
- Sender label (received messages): --text-xs, --color-accent, shown above bubble

### Sender Label

Received messages in groups show a sender label above the bubble:
- Position: left-aligned, 4px above bubble
- Color: --color-text-accent (a5b4fc dark / 4f46e5 light)
- Font: --text-xs (10px), --font-weight-medium
- Only shown in group chats (not 1:1)
- Each sender gets a consistent color from hashToColor()

### Error Messages

From Design Bible Part 3 Section 21.8:

| ID | Trigger | Message | Type |
|----|---------|---------|------|
| G-003 | Group not found | "Group not found. It may have been deleted." | toast, 5s |
| G-005 | Not a member | "You are not a member of this group." | toast, 5s |
| G-008 | Send failed | "Failed to send group message." | toast, 5s |
| G-009 | Load failed | "Failed to load group messages." | toast, 5s |

## Animations

Same as ChatView: msgSlide (400ms), msgReceived (500ms), staggered.

## Keyboard Shortcuts

Same as ChatView: Ctrl+Enter send, Shift+Enter newline, Esc back.

## Accessibility

- Group header: role="banner"
- Sender labels: aria-label="From {sender name}"
- Group info button: aria-label="Group information"
- Same message accessibility as ChatView

## Acceptance Criteria

- [ ] Header shows group name and member count
- [ ] Back button returns to conversation list
- [ ] Info button opens GroupInfoPanel
- [ ] Received messages show sender label above bubble
- [ ] Sender labels use color from hashToColor()
- [ ] Sent messages show status indicators (sending/sent/delivered/read)
- [ ] Same input functionality as 1:1 ChatView
- [ ] Reactions work on group messages
- [ ] Context menu available on right-click
- [ ] Keyboard shortcuts match ChatView

## Self-Review Checklist

- [ ] Follows Design Bible Section 4.6
- [ ] Sender labels implemented correctly
- [ ] i18n strings match catalog
