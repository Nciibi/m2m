# PeerProfile — Implementation Prompt

## Mission

Implement the Peer Profile modal for viewing and verifying a peer's identity fingerprint. This modal enables the security verification flow where users compare fingerprints via an out-of-band channel.

## Scope

Covers the Peer Profile modal including:
- Local fingerprint display
- Peer fingerprint display
- Side-by-side comparison layout
- Verification status (unverified/verified)
- "Confirm Match & Verify" button
- Verification success flow

Does NOT cover: The actual verification backend, ChatView header shield icon.

## Files Expected to Be Modified

- `src/components/PeerProfile.tsx` — Component
- `src/styles/components/modal.css` — Modal styles
- `src/components/ui/icons/ShieldIcon.tsx` — Security icon

## Components to Reuse

- **Modal** (Section 2.4) — Dialog shell
- **Card** (Section 2.3) — Fingerprint display cards
- **Button** (Section 2.1) — Verify, close actions
- **Badge** (Section 2.5) — Verification status

## Components to Create

- **FingerprintCard** — Single fingerprint display card
- **FingerprintComparison** — Side-by-side layout with labels

## Layout Hierarchy

```
<Modal open={isOpen} onClose={handleClose}>
  <div class="verify-modal">
    <h2 id="verify-title">Verify Peer Fingerprint</h2>
    <p id="verify-desc">
      Compare fingerprints via a secure out-of-band channel (e.g., in-person,
      phone call, or another encrypted app).
    </p>

    <div class="fp-comparison">
      <FingerprintCard label="You (Local)" fingerprint={localFP} verified={true} />
      <FingerprintCard label="Peer" fingerprint={peerFP} verified={isVerified} />
    </div>

    <Badge variant={isVerified ? "success" : "warning"}>
      {isVerified ? "Verified" : "Not yet verified"}
    </Badge>

    <Button
      variant="default"
      onClick={confirmVerification}
      disabled={isVerified}
    >
      Confirm Match & Verify
    </Button>
  </div>
</Modal>
```

## Design Implementation Requirements

### Typography

- Title: --text-xl, --font-weight-bold
- Description: --text-sm, --color-text-secondary
- Fingerprint value: --font-mono, --text-sm
- Status badge: --text-xs, --font-weight-semibold

### Colors

- Local card: --color-accent border highlight
- Peer card (unverified): --color-border-default
- Peer card (verified): --color-success border
- Verify button (unverified): default accent
- Verify button (verified): --color-success

### Shadows

- Modal: --shadow-modal
- Verified card: --shadow-accent-glow

### Icons

- ShieldIcon — Header indicator (size 24px)
- VerifiedIcon — Checkmark badge

## States

| State | Visual | Behavior |
|-------|--------|----------|
| Unverified | Warning shield, "Not yet verified" badge | Confirm button enabled |
| Verified | Green shield, "Verified" badge (success color) | Confirm button disabled |
| Verifying | Button shows spinner | Wait for confirmation backend |
| Mismatch | Error modal: "Fingerprints do not match" | Close only |

## Security Considerations

- Fingerprints are safe to display (public key hash, not private key)
- All verification instructions emphasize out-of-band comparison
- "Do NOT proceed" warning on mismatch
- Verification is local-only (no data sent to server)
- Confirmed verification persisted locally

## Acceptance Criteria

- [ ] Modal opens from shield icon click in ChatView
- [ ] Shows local fingerprint and peer fingerprint side-by-side
- [ ] Fingerprints displayed in monospace font with colons (a1b2:c3d4:...)
- [ ] Each fingerprint card labeled clearly
- [ ] Status badge shows "Not yet verified" (warning) or "Verified" (success)
- [ ] "Confirm Match & Verify" button initiates verification
- [ ] On success: toast "Peer verified", shield turns green
- [ ] On mismatch: error message, modal stays open
- [ ] Escape or click outside closes modal
- [ ] Focus trap active while modal open
- [ ] aria-modal, aria-labelledby, aria-describedby applied

## Self-Review Checklist

- [ ] Follows Design Bible Sections 4.3 and 4.7
- [ ] Modal specs match Section 2.4
- [ ] All states handled (unverified, verified, verifying, mismatch)
- [ ] Accessibility ARIA attributes correct
- [ ] Focus trap implemented
