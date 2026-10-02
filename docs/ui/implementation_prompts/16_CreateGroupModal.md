# CreateGroupModal — Implementation Prompt

## Mission

Implement the Create Group modal for creating new encrypted group conversations. This modal allows the user to set a group name and select initial members.

## Scope

Covers the Create Group modal including:
- Group name input field
- Member selection from contacts/family
- Selected member list with remove option
- Create button with validation
- Loading and error states

Does NOT cover: Group chat view, group info panel, backend group creation.

## Files Expected to Be Modified

- `src/components/CreateGroupModal.tsx` — Component
- `src/styles/components/modal.css` — Modal styles

## Components to Reuse

- **Modal** (Section 2.4) — Dialog shell
- **Input** (Section 2.2) — Group name input
- **Button** (Section 2.1) — Create, cancel, add member actions
- **Badge** (Section 2.5) — Member count, role badges

## Layout Hierarchy

```
<Modal open={isOpen} onClose={handleClose}>
  <div class="create-group">
    <h2 id="cg-title">Create Group</h2>

    <label>Group Name</label>
    <Input placeholder="Group name" />

    <label>Members</label>
    <div class="member-search">
      <Input placeholder="Search contacts…" icon={<SearchIcon />} />
    </div>

    <div class="member-list">
      <div class="member-row">
        <Avatar initials="AB" />
        <span>Alice</span>
        <Badge>Selected</Badge>
      </div>
      <div class="member-row">
        <Avatar initials="CD" />
        <span>Charlie</span>
        <Button variant="ghost" onClick={add}>+ Add</Button>
      </div>
    </div>

    <div class="selected-members">
      <Badge>Alice ✕</Badge>
      <Badge>Bob ✕</Badge>
    </div>

    <p class="cg-error" />
    <!-- G-001: "A group must have at least 2 members (including yourself)." -->

    <div class="modal-footer">
      <Button variant="secondary" onClick={handleClose}>Cancel</Button>
      <Button variant="default" disabled={!isValid} onClick={handleCreate}>
        Create Group
      </Button>
    </div>
  </div>
</Modal>
```

## Design Implementation Requirements

### Error Messages

From Design Bible Part 3 Section 21.8:

| ID | Trigger | Message | Type |
|----|---------|---------|------|
| G-001 | < 2 members | "A group must have at least 2 members (including yourself)." | inline error |
| G-002 | Encryption error | "Failed to create group encryption keys." | toast, 6s |

### States

| State | Visual | Behavior |
|-------|--------|----------|
| Empty name | Name input placeholder | Create disabled |
| Insufficient members | Inline error (G-001) | Create disabled |
| Loading | Button spinner | Wait for backend |
| Error | Toast G-002 | Stay on modal |

### Accessibility

- Modal: role="dialog", aria-modal="true", aria-labelledby="cg-title"
- Focus trap while modal open
- First input (group name) receives focus on open
- Escape to close, Cancel button to close

## Acceptance Criteria

- [ ] Modal opens from HubView (via button or future UI)
- [ ] Group name input with placeholder
- [ ] Contact search input with list of available contacts
- [ ] Selected members shown as removable badges
- [ ] Minimum 2 members (including self) validation
- [ ] Create button disabled until valid
- [ ] Loading state on create (button spinner)
- [ ] Error toast on encryption failure (G-002)
- [ ] Close on success → navigate to new group chat
- [ ] Focus trap active while open
- [ ] Escape and backdrop click close modal
- [ ] Modal specs match Section 2.4 (480px width, 80vh max-height)

## Self-Review Checklist

- [ ] Follows Design Bible Section 4.6
- [ ] Modal specs from Section 2.4
- [ ] Error messages from Section 21.8
- [ ] Focus trap implemented
