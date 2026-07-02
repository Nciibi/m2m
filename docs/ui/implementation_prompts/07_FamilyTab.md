# FamilyTab — Implementation Prompt

## Mission

Implement the Family tab content inside HubView. This tab displays trusted family contacts, allowing the user to manage family members, view their status, and connect to them.

## Scope

Covers the Family tab including:
- Family member list with nicknames and key display
- Add family member form (key hex input + nickname)
- Remove family member action with confirmation
- Connect to family member action
- Empty state for no family members

Does NOT cover: The HubView shell, database operations for family table, relay connections.

## Files Expected to Be Modified

- `src/views/FamilyTab.tsx` — Main component
- `src/styles/components/utilities.css` — Tab-specific styles

## Components to Reuse

- **Card** (Section 2.3) — Member display cards
- **Button** (Section 2.1) — Connect, Remove, Add actions
- **Input** (Section 2.2) — Key hex input, nickname input
- **Badge** (Section 2.5) — Status indicators

## Components to Create

- **FamilyMemberCard** — Member display with nickname, key, actions
- **FamilyEmptyState** — Empty state with guidance
- **AddMemberForm** — Inline form for adding a family member

## Layout Hierarchy

```
<FamilyTab>
  <div class="family-tab">

    <!-- Add Member Form -->
    <Card title="Add Family Member" icon="PlusIcon">
      <Input placeholder="Peer key hex" variant="mono" />
      <Input placeholder="Nickname" />
      <Button variant="default">Add Member</Button>
    </Card>

    <!-- Family Member List -->
    <div class="family-list">
      <FamilyMemberCard
        nickname="Mom"
        keyHex="a1b2c3d4e5f6..."
        status="online"
        onConnect={handleConnect}
        onRemove={handleRemove}
      />
      <FamilyMemberCard
        nickname="Dad"
        keyHex="f6e5d4c3b2a1..."
        status="offline"
        onConnect={handleConnect}
        onRemove={handleRemove}
      />
    </div>

    <!-- Empty State -->
    <FamilyEmptyState>
      <HomeIcon size={48} muted />
      <h2>No family members yet</h2>
      <p>Add trusted peers as family members for quick access.</p>
    </FamilyEmptyState>

  </div>
</FamilyTab>
```

## Design Implementation Requirements

### Exact Spacing

- Card padding: --space-lg (20px)
- Input to input gap: --space-sm (12px)
- Input to button gap: --space-md (16px)
- Card to card gap: --space-md (16px)
- Member card internal: 16px padding
- Nickname to key gap: 4px

### Typography

- Nickname: --text-md, --font-weight-semibold
- Key hex: --text-xs, --font-mono, --color-text-muted
- Status: --text-xs, --color-success or --color-text-muted
- Empty title: --text-lg, 600 weight

### Colors

- Online indicator: --color-success
- Offline indicator: --color-text-muted
- Remove button: --color-danger

### Icons

- HomeIcon — Empty state (48px)
- PlusIcon — Add member header
- OnlineDot / OfflineDot — Status

## States

| State | Visual | Behavior |
|-------|--------|----------|
| Empty | Explanation text | Guide user to add members |
| Loading | Skeleton cards (2 items) | While data loads |
| Online member | Green dot, Connect enabled | Can start chat |
| Offline member | Gray dot, Connect shows status | Can still attempt connect |
| Adding | Button spinner | Wait for backend |
| Removing | Confirmation dialog | Confirm before remove |

## Accessibility

- Add key input: aria-label="Peer key hex"
- Nickname input: aria-label="Nickname"
- Connect buttons: aria-label with nickname
- Remove buttons: aria-label with nickname, role="button"

## Security Considerations

- Family contacts stored with public key hex only (never private key)
- Remove requires confirmation
- Family contacts are locally stored only

## Acceptance Criteria

- [ ] Family member list shows all saved family contacts
- [ ] Each member shows nickname, truncated key, and online/offline status
- [ ] Connect button initiates connection to family member
- [ ] Remove button shows confirmation before deleting
- [ ] Add member form accepts key hex + nickname
- [ ] Empty state guides user to add first family member
- [ ] Loading skeleton shown while fetching family list
- [ ] Error handling on add/remove/connect failures

## Self-Review Checklist

- [ ] Follows Design Bible Sections 3.3d and database schema Section 24.1
- [ ] All spacing on 4px grid
- [ ] CSS custom properties used
- [ ] i18n strings match catalog
