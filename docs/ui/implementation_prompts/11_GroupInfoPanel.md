# GroupInfoPanel — Implementation Prompt

## Mission

Implement the Group Information Panel, a modal or panel that displays group details, member list with roles, and provides member management actions for group admins.

## Scope

Covers the Group Info Panel including:
- Group name display (and editing for admins)
- Member list with avatar, name, role, and status
- Add member functionality
- Remove member (admin only)
- Leave group action
- Loading and error states

Does NOT cover: Group creation, group messaging, backend group management operations.

## Files Expected to Be Modified

- `src/components/GroupInfoPanel.tsx` — Component
- `src/styles/components/utilities.css` — Panel-specific styles

## Components to Reuse

- **Modal** (Section 2.4) — Dialog shell
- **Card** (Section 2.3) — Info sections
- **Button** (Section 2.1) — Actions (add, remove, leave, edit)
- **Badge** (Section 2.5) — Role badges (admin/member)
- **Avatar** — Member avatars with initials

## Components to Create

- **MemberListItem** — Single member row with avatar, name, role, actions
- **GroupNameEditor** — Inline editable group name

## Layout Hierarchy

```
<Modal>
  <GroupInfoPanel>
    <div class="group-info">
      <!-- Group Name -->
      <Card>
        <h3>Group Name</h3>
        <GroupNameEditor name={groupName} editable={isAdmin} />
      </Card>

      <!-- Member List -->
      <Card title="Members">
        <MemberListItem
          avatar="AB"
          name="Alice"
          role="admin"
          status="online"
        />
        <MemberListItem
          avatar="CD"
          name="Charlie"
          role="member"
          status="offline"
        />
      </Card>

      <!-- Actions -->
      <Button variant="default" onClick={addMember}>Add Member</Button>
      <Button variant="danger" onClick={leaveGroup}>
        {isAdmin ? "Leave Group" : "Leave Group"}
      </Button>
    </div>
  </GroupInfoPanel>
</Modal>
```

## Design Implementation Requirements

### Error Messages

From Design Bible Part 3 Section 21.8:

| ID | Trigger | Message |
|----|---------|---------|
| G-006 | Admin leave with members | "You cannot leave as admin while other members are present. Transfer admin first or remove all members." |
| G-007 | Non-admin remove member | "Only admins can remove members." |
| G-010 | Invite to group failed | "Failed to invite member to group." |
| G-011 | Group name update failed | "Failed to update group name." |

## States

| State | Visual | Behavior |
|-------|--------|----------|
| Loading | Skeleton with 3 member placeholders | While data loads |
| Admin view | Edit button on group name, remove on members | Full management |
| Member view | No edit/remove, only leave | Read-only info |
| Empty members | Cannot happen (minimum 2 members) | N/A |
| Leave (admin with others) | Warning modal | Transfer admin or remove members |

## Accessibility

- Group info: aria-label="Group information"
- Member list: role="list"
- Remove button: aria-label="Remove {name} from group"
- Leave button: aria-label="Leave group"

## Acceptance Criteria

- [ ] Group name displayed (editable for admins)
- [ ] Member list with all members shown
- [ ] Each member shows avatar, name, role badge, online status
- [ ] Admin can remove members (with confirmation)
- [ ] Admin cannot leave if other members present (G-006)
- [ ] Non-admin can leave without restriction
- [ ] Add member opens member selection
- [ ] Loading state shows skeleton
- [ ] Error messages shown on failure
- [ ] All actions show appropriate feedback (toast)

## Self-Review Checklist

- [ ] Follows Design Bible Section 4.6
- [ ] Database schema (groups, group_members) from Section 24.2
- [ ] Error messages from Section 21.8
- [ ] i18n strings match catalog
