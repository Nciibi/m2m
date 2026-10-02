# Dialogs — Implementation Prompt

## Mission

Implement the generic dialog/modal system for M2M, including the fingerprint verification dialog, confirmation dialogs, alert dialogs, and prompt dialogs. Every dialog follows the Design Bible's modal specifications with proper focus management and accessibility.

## Scope

Covers dialog components including:
- Generic Confirm dialog (title, body, Cancel/Confirm)
- Generic Alert dialog (title, body, OK)
- Generic Prompt dialog (title, body, input, Cancel/Confirm)
- Fingerprint verification dialog (local + peer fingerprints)
- Full focus trap implementation
- Opening/closing animations
- Backdrop click handling

Does NOT cover: Specific modal instances (CreateGroupModal, InvitePeerModal) — those have their own prompts.

## Files Expected to Be Modified

- `src/components/ConfirmDialog.tsx` — Confirm dialog
- `src/components/AlertDialog.tsx` — Alert dialog
- `src/components/PromptDialog.tsx` — Prompt dialog
- `src/components/VerifyDialog.tsx` — Fingerprint verification dialog
- `src/components/Modal.tsx` — Base modal component
- `src/styles/components/modal.css` — Modal styles

## Components to Reuse

- **Button** (Section 2.1) — Dialog action buttons
- **Input** (Section 2.2) — Prompt dialog input
- **Badge** (Section 2.5) — Status indicator in verify dialog

## Components to Create

- **Modal** — Base reusable modal with backdrop, content, focus trap
- **FingerprintCard** — Fingerprint display card for verify dialog

## Modal Specs

From Design Bible Section 2.4 and 11.3:

```
┌────────────────────────────────────────┐
│  [─ backdrop: bg-modal-backdrop ─]     │
│                                        │
│    ┌─── modal surface ─────────────┐   │
│    │  Title                    [✕] │   │  ← padding: --space-xl
│    │                               │   │
│    │  Body content (scrolls if     │   │
│    │  exceeds max-height)          │   │
│    │                               │   │
│    │  [─── Footer ───]             │   │  ← padding: --space-lg
│    │  [Cancel]  [Confirm]          │   │
│    └───────────────────────────────┘   │
└────────────────────────────────────────┘
```

- Width: 480px (default), 90vw max
- Max-height: 80vh
- Border-radius: --radius-xl (24px)
- Background: --color-bg-elevated
- Backdrop: --color-bg-modal-backdrop (rgba(0,0,0,0.65 dark / 0.35 light)
- Shadow: --shadow-modal
- z-index: 9999

### Opening Sequence (300ms total)

From Design Bible Section 11.3:
- 0ms: Backdrop fade-in (0 → 0.6 opacity)
- 0ms: Content scale-up (0.95 → 1.0) + fade-in (0 → 1)
- 50ms: Scrollbar lock applied to body
- 100ms: Focus trapped in modal
- 150ms: First focusable element receives focus
- 300ms: Animation complete

### Closing Sequence (200ms total)
- 0ms: Backdrop fade-out
- 0ms: Content scale-down (1.0 → 0.95) + fade-out
- 50ms: Focus returned to trigger element
- 100ms: Scrollbar lock released
- 200ms: Animation complete

### Focus Trap

From Design Bible Section 32.1:
- Save currently focused element reference on open
- Set focus to first focusable element inside modal
- Tab: cycle forward through modal elements
- Shift+Tab: cycle backward
- Escape: close modal, return focus to saved element
- Click outside: close modal, return focus
- aria-hidden="true" on all sibling elements while open

### Fingerprint Verification Dialog

From Design Bible Section 4.7 and Part 3 Section 21.7:

```
┌────────────────────────────────────────────┐
│  Verify Peer Fingerprint                   │
│                                            │
│  Compare fingerprints via a secure         │
│  out-of-band channel...                    │
│                                            │
│  ┌─── You (Local) ───┐ ┌─── Peer ───────┐ │
│  │ a1b2:c3d4:e5f6:  │ │ a1b2:c3d4:e5f6:│ │
│  │ g7h8:i9j0:k1l2:  │ │ g7h8:i9j0:k1l2:│ │
│  │ m3n4:o5p6        │ │ m3n4:o5p6      │ │
│  └───────────────────┘ └────────────────┘ │
│                                            │
│  [Not yet verified / Verified]            │
│                                            │
│  [Confirm Match & Verify]                 │
└────────────────────────────────────────────┘
```

### Error Messages

| ID | Trigger | Message | Type |
|----|---------|---------|------|
| SEC-001 | Verified | "Peer verified. Always verify fingerprints via a trusted out-of-band channel." | success toast, 4s |
| SEC-002 | Mismatch | "Fingerprints do not match. Do NOT proceed with this peer." | error modal |

## Accessibility

- Modal: role="dialog", aria-modal="true"
- aria-labelledby pointing to title element
- aria-describedby pointing to body element
- All siblings aria-hidden="true" while modal open
- Focus trap: Tab cycles within modal elements only
- First focusable element focused on open
- Escape key to close
- Return focus to trigger on close

## Acceptance Criteria

- [ ] Modal renders with backdrop and centered content
- [ ] Opening animation: backdrop fade + content scale-up (300ms)
- [ ] Closing animation: reverse (200ms)
- [ ] Focus trap: Tab cycles within modal, Shift+Tab reverses
- [ ] Escape closes modal and returns focus
- [ ] Backdrop click closes modal
- [ ] Confirm dialog shows with Cancel and Confirm buttons
- [ ] Alert dialog shows with OK button
- [ ] Prompt dialog shows with input field
- [ ] Fingerprint dialog shows local + peer fingerprints side-by-side
- [ ] Verify button works with success/mismatch messages
- [ ] Scrollbar locked while modal open
- [ ] aria-modal, aria-labelledby, aria-describedby applied
- [ ] Siblings aria-hidden="true" while modal open
- [ ] Modal width 480px, max-height 80vh
- [ ] z-index at 9999

## Self-Review Checklist

- [ ] Follows Design Bible Sections 2.4 and 11.3 exactly
- [ ] Focus trap from Section 32.1
- [ ] Opening/closing timing from Section 11.3
- [ ] z-index from Section 25 (9999)
- [ ] All ARIA attributes from Section 16
