# InvitePeerModal — Implementation Prompt

## Mission

Implement the Invite Peer modal that displays a generated one-time invite link with copy functionality, countdown timer, and security warnings.

## Scope

Covers the Invite Peer modal including:
- Generated invite link display (monospace, read-only)
- Copy to clipboard button with feedback
- Countdown timer (🔥 M:SS format)
- Tor inbound warning
- States: generating, ready, expired, copied

Does NOT cover: The Connect tab "Host a Connection" card, invite generation backend.

## Files Expected to Be Modified

- `src/components/InvitePeerModal.tsx` — Component
- `src/styles/components/modal.css` — Modal styles

## Components to Reuse

- **Modal** (Section 2.4) — Dialog shell
- **Button** (Section 2.1) — Copy, close, generate
- **Input** (Section 2.2) — Invite display (readonly mono variant)

## Components to Create

- **CountdownTimer** — 🔥 M:SS or HH:MM:SS display

## Layout Hierarchy

```
<Modal open={isOpen} onClose={handleClose}>
  <div class="invite-modal">
    <h2 id="invite-title">Share Invite Link</h2>

    <div class="invite-link">
      <Input variant="mono" readonly value="m2m://a1b2c3d4e5f6..." />
      <Button icon aria-label="Copy to clipboard">
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
    </div>

    <CountdownTimer expiresAt={expiresAt} />
    <p class="invite-expiry">Expires in 59:32</p>

    <!-- Tor Warning -->
    <div class="tor-warning" hidden={!torEnabled}>
      ⚠️ Tor Inbound Warning
      Tor is enabled for outbound connections, but this invite
      contains your real IP address.
    </div>
  </div>
</Modal>
```

## Design Implementation Requirements

### Typography

- Invite link: --font-mono, --text-sm
- Countdown: --text-md, --color-warning (or --color-danger when expired)
- Tor warning: --text-sm, --color-warning

### Colors

- Countdown active: --color-warning (#f59e0b)
- Countdown expired: --color-danger (#ef4444)
- Copy icon (idle): --color-text-muted
- Copy icon (hover): --color-text-primary
- Checkmark (copied): --color-success
- Tor warning bg: --color-warning-bg

### States

| State | Visual | Behavior |
|-------|--------|----------|
| Generating | Button spinner, no link | Wait for backend |
| Ready | Invite displayed, countdown ticking | Copy enabled |
| Copied | Copy icon → checkmark for 2s | Reverts to copy icon |
| Expired | Countdown shows 00:00, --color-danger | Copy disabled |
| Tor warning | Yellow warning box below invite | Shown when Tor enabled |

### Security

- Invite link: one-time use, 60-minute validity (configurable, max 24h)
- Tor warning: when Tor enabled for outbound but IP still in invite
- Copy uses Tauri clipboard API

## Acceptance Criteria

- [ ] Modal shows generated invite link in monospace read-only input
- [ ] Copy button copies link to clipboard
- [ ] Copy icon cross-fades to checkmark for 2s then reverts
- [ ] Countdown timer ticks down every second
- [ ] Countdown shows 🔥 M:SS format (or HH:MM:SS for long durations)
- [ ] Expired state shows 00:00 in red
- [ ] Tor warning shown when applicable
- [ ] Generating state shows spinner
- [ ] Close (Escape, X button, backdrop click)
- [ ] Focus trap active while open

## Self-Review Checklist

- [ ] Follows Design Bible Section 4.3
- [ ] Modal specs from Section 2.4
- [ ] Security warnings from Section 8.2
- [ ] Countdown timer handled correctly
