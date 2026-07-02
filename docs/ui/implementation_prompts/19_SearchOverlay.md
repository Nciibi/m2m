# SearchOverlay — Implementation Prompt

## Mission

Implement the search overlay for finding messages within a conversation (ChatView Ctrl+F) and searching conversations (Chats tab). This provides live search with result highlighting and keyboard navigation.

## Scope

Covers search functionality including:
- Search input with 🔍 icon and clear X button
- Results list with matched text highlighting
- Debounced search (300ms)
- Loading state during search
- Empty and no-results states
- Keyboard navigation through results

Does NOT cover: Backend search implementation, conversation list search (handled by ChatsTab).

## Files Expected to Be Modified

- `src/components/SearchOverlay.tsx` — ChatView search component
- `src/styles/components/utilities.css` — Component styles

## Components to Reuse

- **Input** (Section 2.2) — Search input (compact variant, clearable)

## Layout Hierarchy

```
<SearchOverlay visible={isOpen}>
  <div class="search-overlay">
    <Input
      placeholder="Search messages… (Esc)"
      icon={<SearchIcon />}
      clearable
      autoFocus
    />
    <span class="search-count">3 results</span>

    <div class="search-results">
      <div class="search-result">
        <span class="search-result__preview">
          ...matched <mark>text</mark> highlighting...
        </span>
        <span class="search-result__time">12:30 PM</span>
      </div>
    </div>

    <!-- Loading -->
    <div class="search-loading">Searching…</div>

    <!-- No results -->
    <div class="search-empty">No messages found</div>
  </div>
</SearchOverlay>
```

## Design Implementation Requirements

### Specs

- Height: 40px (search bar), padding 8px 32px
- Input: compact variant (36px height)
- Results area: below search bar, scrollable
- Search count: --text-xs, --color-text-muted, right-aligned

### Colors

- Highlighted text: --color-accent-bg as background on <mark>
- Selected result: --color-accent-glow-subtle bg

### States

| State | Visual |
|-------|--------|
| Empty (no query) | "Search messages…" placeholder |
| Typing (debouncing) | No visible change (debounce 300ms) |
| Loading | "Searching…" text, spinner optional |
| Results | List with <mark> highlighting |
| No results | "No messages found" text |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Ctrl+F (ChatView) | Toggle search overlay |
| Escape | Close search, return focus to input |
| Enter (with result) | Scroll to selected message, close search |
| ArrowUp/Down | Navigate through results |

### Performance

- Debounce search input at 300ms
- Max 100 results shown
- Search response: < 200ms (backend must match)
- Clean up search results on close

### Accessibility

- Search input: aria-label="Search messages"
- Results: role="listbox"
- Each result: role="option", aria-selected
- Search count: aria-live="polite"
- Close on Escape returns focus to message input

## Acceptance Criteria

- [ ] Search overlay toggles with Ctrl+F in ChatView
- [ ] Search input auto-focused when opened
- [ ] Placeholder shows "Search messages… (Esc)"
- [ ] Clear X button appears when input has value
- [ ] Search results show with matched text highlighted (<mark>)
- [ ] Result count shown ("3 results")
- [ ] ArrowUp/Down navigates through results
- [ ] Enter scrolls to selected message and closes search
- [ ] Escape closes search and returns focus
- [ ] "No messages found" shown when no results
- [ ] Debounce 300ms prevents excessive searches
- [ ] Search count updates in real-time

## Self-Review Checklist

- [ ] Follows Design Bible Section 3.4, 3.3b
- [ ] Keyboard shortcuts from Section 6.4
- [ ] Performance budget: < 200ms response
