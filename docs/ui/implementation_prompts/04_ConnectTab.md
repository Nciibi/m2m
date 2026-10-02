# ConnectTab — Implementation Prompt

## Mission

Implement the Connect tab content shown inside HubView. This tab provides two primary actions: hosting a connection by generating a one-time signed invite, and joining a connection by pasting a peer's invite. It also displays the user's identity fingerprint for out-of-band verification.

## Scope

Covers the Connect tab content including:
- "Host a Connection" card with invite generation, copy, countdown, and recent invites
- "Join a Connection" card with invite input, validation, naming panel, and connect action
- Identity fingerprint display at the bottom
- Listening indicator when the relay is active

Does NOT cover: The HubView shell (header, tab bar), the actual relay/listening backend logic, fingerprint verification modal (PeerProfile).

## Files Expected to Be Modified

- `src/views/ConnectTab.tsx` — Main component
- `src/styles/components/utilities.css` — Tab-specific styles
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **Button** (Section 2.1) — Generate Invite, Copy, Connect, paste buttons
- **Input** (Section 2.2) — Invite link input, display name inputs
- **Card** (Section 2.3) — Two main action cards
- **Badge** (Section 2.5) — Status labels

## Components to Create

- **ListeningIndicator** — Green pulsing dot + "Listening for incoming connections" text
- **CountdownTimer** — 🔥 M:SS countdown display
- **RecentInvitesList** — List of last 5 generated invites with copy buttons
- **FingerprintDisplay** — Truncated fingerprint with copy button

## Layout Hierarchy

```
<ConnectTab>
  <div class="connect-tab">
    <!-- Listening Indicator -->
    <div class="connect-listening">
      <span class="connect-listening__dot" />  <!-- green, pulseRing animation -->
      <span>Listening for incoming connections</span>
    </div>

    <!-- Card: Host a Connection -->
    <Card title="Host a Connection" icon="PlusIcon">
      <p>Generate a one-time signed invite...</p>

      <Button variant="default">Generate Invite Link</Button>

      <!-- Invite output (shown after generation) -->
      <div class="connect-invite__output">
        <Input variant="mono" readonly value="m2m://a1b2c3..." />
        <Button variant="icon" aria-label="Copy to clipboard"><CopyIcon /></Button>
        <CountdownTimer expiresAt={timestamp} />
      </div>

      <div class="connect-recent">
        <h4>Recent Invites</h4>
        <RecentInvitesList items={recentInvites} />
      </div>
    </Card>

    <!-- Card: Join a Connection -->
    <Card title="Join a Connection" icon="LinkIcon">
      <p>Paste an invite link from a trusted peer...</p>

      <div class="connect-join__input">
        <Input placeholder="m2m://..." variant="mono" />
        <Button variant="default" disabled={!validInvite}>Connect</Button>
      </div>

      <!-- Valid check (shown when invite parsed) -->
      <div class="connect-join__valid">✓ Valid Invite Found</div>

      <!-- Naming Panel (expandDown animation) -->
      <div class="connect-naming">
        <Input placeholder="Your name (how they see you)" />
        <Input placeholder="Their name (how you see them)" />
      </div>
    </Card>

    <hr class="connect-divider" />

    <!-- Fingerprint -->
    <FingerprintDisplay fingerprint={identity.fingerprint} />
  </div>
</ConnectTab>
```

## Design Implementation Requirements

### Exact Spacing

- Listening indicator: 16px below tab bar, 32px horizontal padding (desktop)
- Listening dot: 8px green dot, pulseRing 2s animation
- Listening text: --text-sm (11px), --color-success, 500 weight
- Card to listening gap: 16px
- Card header padding: --space-lg
- Card internal gaps: 16px between sections
- Invite output: full-width monospace input + copy button side-by-side
- Countdown: 8px below invite output
- Recent invites section: 8px gap between items
- Join invite input: flex row, input takes remaining space, Connect button
- Naming panel: 8px gap between input fields
- Divider: 1px color-border-default, margin 20px 0
- Fingerprint: --text-sm, --font-mono, centered, muted

### Typography

- Card title: --text-lg, --font-weight-semibold
- Card description: --text-sm, --color-text-secondary
- Invite link value: --font-mono, --text-sm
- Countdown: --text-xs (10px), --color-warning
- Valid check: --text-sm, --color-success
- Section divider: --text-xs
- Fingerprint: --text-sm, --font-mono, --color-text-muted
- Name input placeholders: "Your name (how they see you)", "Their name (how you see them)"

### Colors

- Listening dot: --color-success (#10b981)
- Valid checkmark: --color-success
- Invite link copy button: --color-accent
- Countdown warning: --color-warning (#f59e0b)
- Expired countdown (00:00): --color-danger
- Tor warning: --color-warning text on --color-warning-bg

### Glass Effects

- Cards inherit: background --color-bg-card, backdrop-filter var(--glass-blur-sm)

### Shadows

- Cards: --shadow-card
- Copy button hover: --shadow-accent-strong

### Icons

- PlusIcon — Host card header
- LinkIcon — Join card header
- CopyIcon — Copy buttons
- CheckIcon — Valid checkmark
- OnlineDot — Listening indicator
- CloseIcon — Clear invite input

## States

### Hover States
- Generate Invite button: translateY(-2px) + shadow-accent-strong
- Copy button: opacity 0.6→1.0, scale(1.05)
- Connect button (enabled): translateY(-2px) + shadow-accent-strong
- Connect button (disabled): no change

### Focus States
- All inputs: border-active + accent-glow focus ring via :focus-visible
- Buttons: 3px accent-glow outline via :focus-visible

### Active States
- Buttons: translateY(0) + scale(0.98)

### Disabled States
- Connect button (no valid invite): opacity 0.5, cursor not-allowed, no shadow
- Generate Invite during generation: loading spinner

### Loading States
- Generate Invite button: LoadingSpinner (18px inline), text hidden
- Connect button: LoadingSpinner, "Connecting…" text hidden

### Empty States
- No recent invites: section hidden entirely
- No invite pasted yet: naming panel hidden

### Error States

From Design Bible Part 3 Section 21.2:

| ID | Trigger | Message | Type |
|----|---------|---------|------|
| C-001 | Invalid invite format | "Invalid invite link format. Expected 'm2m://...'" | inline error |
| C-002 | Invite expired | "This invite has expired. Ask the peer to generate a new one." | inline error |
| C-003 | Self-connect attempt | "You cannot connect to yourself." | warning inline |
| C-004 | Connection timeout | "Connection timed out. The peer may be offline or behind a firewall." | toast 8s |
| C-005 | Connection refused | "Connection refused. The peer may not be listening." | toast 8s |

**Tor inbound warning** (when Tor enabled + invite generated):
```
⚠️ Tor Inbound Warning
Tor is enabled for outbound connections, but this invite
contains your real IP address.
```
Warning box: --color-warning-bg, 1px --color-warning border, --radius-md, --text-sm

## Animations

| Animation | Duration | Easing | Trigger |
|-----------|----------|--------|---------|
| pulseRing | 2s | ease-in-out | Listening dot (continuous) |
| expandDown | 300ms | ease-out-expo | Naming panel appears on valid invite |
| fadeIn | 150ms | ease-out-expo | Countdown appears, valid checkmark |
| popIn | 300ms | ease-out-back | Copied feedback (checkmark) |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Tab | Cycle through: Generate → Copy → Connect button → inputs |
| Enter | Activate focused button or submit form |
| Escape | Clear invite input / close error |

## Interactions

- **Generate Invite**: Click → button shows spinner → invite appears with copy button + countdown
- **Copy to clipboard**: Click copy → icon crossfades to checkmark for 2s, then reverts
- **Invite auto-expiry**: Countdown ticks down every second; at 00:00 shows expired state
- **Paste invite**: Input validates in real-time; if valid format → green checkmark + naming panel
- **Connect flow**: Click Connect → button spinner → on success navigate to ChatView
- **Recent invites**: Click copy on any recent invite; list capped at 5 items
- **Fingerprint copy**: Icon changes to checkmark for 2s

## Accessibility

- Invite input: aria-label="Paste invite link"
- Copy buttons: aria-label="Copy to clipboard"
- Generated invite: aria-label="Generated invite link"
- Naming inputs: aria-label="Your display name" / "Their display name"
- Countdown: aria-live="polite" (updates every second)
- Connect button disabled: title/tooltip explaining why
- Tor warning: role="alert"
- All inputs: aria-invalid="true" in error state
- Focus order: Generate → Copy → Connect → inputs → fingerprint copy

## Responsive Behavior

- **Desktop (>1000px)**: Cards at full width within 32px padding
- **Tablet (600-1000px)**: 24px padding, same card layout
- **Mobile (<600px)**: 16px padding, invite output stacks vertically, Connect button full-width

## Performance Considerations

- Countdown timer: use requestAnimationFrame or 1s setInterval, clean up on unmount
- Debounce invite validation (200ms)
- Recent invites list: max 5 items, no virtual scrolling needed

## Security Considerations

- Invite links contain IP + public key — one-time use, 60min validity
- Tor warning when Tor enabled but IP still exposed
- Fingerprint display shows truncated public key (not private key)
- Copy to clipboard via Tauri API (not browser API)
- No private key data ever rendered

## Edge Cases

- **Invalid paste format**: Show C-001 immediately
- **Expired invite pasted**: Show C-002
- **Self-connect**: Show C-003 warning
- **Rapid generate clicks**: Debounce generation (disable button during loading)
- **Network disconnect during connect**: Show C-004 timeout toast
- **All 5 recent invites slots full**: Oldest drops off when new one generated
- **Countdown at 00:00**: Show "Expired" label, hide copy button

## Acceptance Criteria

- [ ] "Host a Connection" card shows Generate Invite button
- [ ] After generation, invite link appears with copy button and countdown (60 min)
- [ ] Copy button shows checkmark feedback for 2s
- [ ] Countdown decrements every second, shows 🔥 M:SS format
- [ ] At 00:00 shows expired state (red)
- [ ] Recent invites section shows up to 5 generated invites
- [ ] "Join a Connection" card shows invite input
- [ ] Valid invite format shows green checkmark + naming panel
- [ ] Invalid invite shows inline error
- [ ] Connect button disabled until valid invite parsed
- [ ] Connecting shows spinner in button
- [ ] Identity fingerprint shown at bottom with copy button
- [ ] Tor warning shown when applicable
- [ ] All ARIA attributes applied
- [ ] Keyboard navigation works

## Self-Review Checklist

- [ ] Follows Design Bible Sections 3.3a and 12.4 exactly
- [ ] All spacing on 4px grid
- [ ] CSS custom properties used throughout
- [ ] All states handled (generated, expired, valid, invalid, connecting)
- [ ] Animations only use transform/opacity
- [ ] All text strings use i18n
