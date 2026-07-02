# ChatsTab — Implementation Prompt

## Mission

Implement the Chats tab content inside HubView. This tab displays the user's conversation list with search, favorites, archive functionality, and empty states. It provides access to individual conversations and allows managing conversation metadata.

## Scope

Covers the Chats tab content including:
- Search bar with 🔍 icon and clear button
- Conversation list with sorted items (favorites → recency → archived)
- Conversation item component with hover-reveal actions
- Archived section with collapsible toggle
- Loading skeleton (3 shimmer items)
- Empty states for no conversations and no search results

Does NOT cover: The HubView shell, individual ChatView, database storage operations.

## Files Expected to Be Modified

- `src/views/ChatsTab.tsx` — Main component
- `src/styles/components/utilities.css` — Tab-specific styles
- `src/components/ui/icons/SearchIcon.tsx` — Search icon
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **Input** (Section 2.2) — Search bar
- **Badge** (Section 2.5) — Notification counts, online/offline dots
- **Button** (Section 2.1) — Icon buttons for actions, "Get Started" CTA
- **Card** (Section 2.3) — Archived section container (collapsible)
- **ConversationItem** (see prompt 23) — Individual conversation rows
- **LoadingSpinner** (Section 2.7) — Search loading state

## Components to Create

- **ConvList** — Container with stagger animation for conversation items
- **ArchivedSection** — Collapsible section header + archived items
- **ChatsEmptyState** — "No conversations yet" with icon and CTA
- **SearchEmptyState** — "No conversations found" with hint text

## Layout Hierarchy

```
<ChatsTab>
  <div class="chats-tab">
    <!-- Search Bar -->
    <div class="chats-search">
      <Input
        variant="default"
        placeholder="Search conversations…"
        icon={<SearchIcon />}
        clearable
      />
    </div>

    <!-- Loading Skeleton -->
    <div class="chats-skeleton">
      <div class="skeleton-conv-item" />
      <div class="skeleton-conv-item" />
      <div class="skeleton-conv-item" />
    </div>

    <!-- Conversation List (loaded state) -->
    <div class="chats-list" role="list">
      <ConversationItem
        avatar="AB"
        name="Alice"
        time="2m ago"
        preview="Hey, are you there?"
        online={true}
        favorite={true}
        archived={false}
        muted={false}
      />
      <ConversationItem
        avatar="CD"
        name="Charlie"
        time="1h ago"
        preview="See you tomorrow!"
        online={true}
        favorite={false}
      />
      <!-- ... more items ... -->
    </div>

    <!-- Archived Section -->
    <ArchivedSection>
      <ConversationItem
        avatar="EF"
        name="Eve"
        archived={true}
        preview="(archived)"
      />
    </ArchivedSection>

    <!-- Empty State (no conversations) -->
    <ChatsEmptyState>
      <MessageIcon size={48} muted />
      <h2>No conversations yet</h2>
      <p>Generate an invite link to host a connection...</p>
      <Button variant="default">Get Started</Button>
    </ChatsEmptyState>

    <!-- Search Empty State -->
    <SearchEmptyState>
      <p>No conversations found</p>
      <p>Try adjusting your search terms or clear the filter.</p>
    </SearchEmptyState>
  </div>
</ChatsTab>
```

## Design Implementation Requirements

### Exact Spacing

From Design Bible Sections 3.3b & 12.5:

- Search bar: 36px height, border-radius 12px, below tab bar top
- Search padding: 12px 16px
- Search icon: 16px from left edge, 16px icon
- Clear button: right side, opacity 0.6 (hover 1.0)
- Conversation item: 64px height, internal padding 16px 20px
- Avatar: 48×48px, border-radius 14px
- Avatar-to-text gap: 16px
- Name to time: right-aligned
- Preview to time: below name
- Online dot: 8px, 2px white border, top-right of avatar
- Items gap: 8px (visually, the divider line)
- Section gap: 32px between conv list and archived
- Search to list gap: 8px

### Typography

- Search placeholder: --text-sm, --color-text-placeholder
- Conversation name: --text-md (13.6px / 0.85rem), --font-weight-semibold (600)
- Time: --text-xs (10.4px / 0.65rem), --color-text-muted, right-aligned
- Preview: --text-sm (11.5px / 0.72rem), --color-text-secondary, single-line truncated
- Empty state title: --text-lg (15.2px), 600 weight
- Empty state description: --text-md, muted
- Archived header: --text-sm, --color-text-secondary

### Colors

- Search bar bg: --color-bg-input
- Search bar border: --color-border-default
- Search icon: --color-text-muted
- Conversation item default bg: rgba(255,255,255,0.02)
- Conversation item hover bg: rgba(255,255,255,0.05)
- Online dot: --color-success (#10b981)
- Offline dot: --color-text-muted (#94a3b8)
- Favorite star (active): gold (#f59e0b)
- Empty state icon: --color-text-muted

### Glass Effects

- None beyond inherited app-shell glass

### Shadows

- Conversation item hover: --shadow-md + --shadow-accent-glow

### Icons

- SearchIcon — Search bar (16px)
- CloseIcon — Clear search (14px)
- StarIcon — Favorite toggle (active: gold fill, inactive: outline)
- FolderIcon — Archive toggle
- BellIcon / BellOffIcon — Mute toggle
- TrashIcon — Delete
- MessageIcon — Empty state (48px)

## States

### Hover States
- Conversation item: translateY(-2px), bg 0.05, shadow-md + accent-glow, 150ms
- Action buttons (hover-reveal): scale(1.2), tinted
- Favorite star hover: gold tint
- Search clear: opacity 0.6→1.0

### Focus States
- Search input: border-active + 3px accent-glow ring via :focus-visible
- Each conversation item: focus ring via :focus-visible
- Action buttons: focus ring

### Active States
- Conversation item: translateY(-1px)
- Action buttons: scale(0.95)

### Disabled States
- Deleted conversations: crossfade out after confirmation

### Loading States
From Design Bible Part 3 Section 23.1:

**Conversation list skeleton (3 items):**
- Each skeleton: 64px height, matching real conversation item dimensions
- Avatar skeleton: 48×48px rounded rectangle (--radius-lg), shimmer fill
- Name skeleton: full-width shimmer bar (--radius-sm)
- Preview skeleton: 60% width shimmer bar (--radius-sm)
- Time skeleton: 40px width shimmer bar, right-aligned
- Shimmer animation: @keyframes shimmer 2s linear infinite
  - Dark: rgba(255,255,255,0.03) → rgba(255,255,255,0.08) → rgba(255,255,255,0.03)
  - Light: rgba(0,0,0,0.03) → rgba(0,0,0,0.08) → rgba(0,0,0,0.03)

### Empty States
From Design Bible Section 9.1:

**No conversations:**
```
💬 (MessageIcon, 48px, muted)
No conversations yet  (--text-lg, 600 weight)
Generate an invite link to host a connection, or paste an invite from a peer to join.  (--text-md, muted)
[Get Started]  (default button — navigates to Connect tab)
```

**Search no results:**
```
No conversations found  (--text-lg, 600 weight)
Try adjusting your search terms or clear the filter.  (--text-md, muted)
```

### Error States

| Trigger | Message | Type |
|---------|---------|------|
| List load failure | "Failed to load conversations." | toast, 5s |
| Favorite toggle failure | "Failed to toggle favorite." | toast, 4s |
| Archive toggle failure | "Failed to archive conversation." | toast, 4s |
| Delete failure | "Failed to delete conversation." | toast, 5s |

## Animations

| Animation | Duration | Easing | Trigger |
|-----------|----------|--------|---------|
| stagger | 30ms per item | — | Initial mount (max 300ms) |
| fadeIn | 150ms | ease-out-expo | Search results update |
| btnHover | 150ms | ease-out-expo | Item hover |
| expandDown | 300ms | ease-out-expo | Archived section toggle |
| shimmer | 2s | linear | Loading skeleton (continuous) |

## Keyboard Shortcuts

From Design Bible Sections 6.4 & 13.2:

| Key | Action |
|-----|--------|
| ArrowUp / ArrowDown | Navigate conversation list |
| Enter | Open selected conversation |
| Tab | Search → first conv item → next items |
| Escape | Clear search focus / blur input |

## Interactions

- **Search**: Live as-you-type filtering (case-insensitive, matches name + preview)
- **Sorting order**: 1) Active non-archived first, 2) Favorites ★ first within active, 3) By last_message_at descending, 4) Archived last
- **Click conv item**: Navigate to ChatView for that conversation
- **Favorite toggle**: Star fills gold on favorite; affects sort order
- **Archive toggle**: Moves conversation to archived section; re-toggle unarchives
- **Mute toggle**: Bell with slash; suppresses notifications
- **Delete**: Requires confirmation (use Confirm dialog from prompt 25)
- **Archived section**: Click header to expand/collapse

## Accessibility

- Conversation list: role="list"
- Each conversation item: role="listitem" + role="button", aria-label with conversation name
- Search input: aria-label="Search conversations"
- Action buttons: aria-label for each (e.g., "Add to favorites")
- Online dot: aria-label="Online" / "Offline"
- Archived section: aria-expanded on toggle
- Empty state: role="status"
- Skeleton loading: aria-label="Loading conversations", aria-busy="true"

## Responsive Behavior

- **Desktop (>1000px)**: 32px horizontal padding
- **Tablet (600-1000px)**: 24px padding
- **Mobile (<600px)**: 16px padding, 40px avatar (reduced from 48px), 56px item height (reduced from 64px)

## Performance Considerations

- Conversation list (50 items): < 300ms load time
- Search: < 200ms response (debounced 150ms)
- Virtual scrolling not needed (< 100 items typical)
- Skeleton shows max 3s before fading out
- Stagger animation only on first mount, not re-renders

## Security Considerations

- Conversation previews truncated (single line) to prevent shoulder surfing
- Archived conversations hidden from main list for privacy
- No message content visible in list (only preview)
- Delete requires confirmation to prevent accidental data loss

## Edge Cases

- **Long conversation names**: Truncate with ellipsis
- **Very long previews**: Single-line truncate
- **Zero conversations**: Show empty state with CTA
- **Search while offline**: Results from local cache
- **Rapid favorite toggling**: Debounce backend calls
- **Delete with unread messages**: Show confirmation with warning
- **Archive all conversations**: Only archived section visible
- **Network error during action**: Toast error, revert optimistic update

## Acceptance Criteria

- [ ] Search bar is 36px with 🔍 icon and clear X button
- [ ] Search filters conversations in real-time by name and preview
- [ ] Conversation items render with 64px height, 48px avatar, correct spacing
- [ ] Items sorted: favorites first, by recency, archived last
- [ ] Favorites show gold star, affect sort position
- [ ] Hover reveals action buttons (favorite, archive, mute, delete)
- [ ] Each action button has correct hover state and aria-label
- [ ] Archived section collapsible with header toggle
- [ ] Loading skeleton shows 3 shimmer items until data loads
- [ ] Empty state shows when no conversations exist
- [ ] Search empty state shows when no results match
- [ ] All keyboard navigation works (arrows, Enter, Tab)
- [ ] Online dot appears on online peers
- [ ] Skeleton fades out 200ms after load
- [ ] Stagger animation on mount (30ms per item)
- [ ] Responsive at all breakpoints

## Self-Review Checklist

- [ ] Follows Design Bible Sections 3.3b and 12.5 exactly
- [ ] All spacing on 4px grid
- [ ] CSS custom properties throughout
- [ ] All states handled (loading, loaded, empty, search, error)
- [ ] Sorting matches spec: favorites → recency → archived
- [ ] Animations only use transform/opacity
- [ ] i18n strings match string catalog
