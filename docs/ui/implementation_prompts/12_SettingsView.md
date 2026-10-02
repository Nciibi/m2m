# SettingsView — Implementation Prompt

## Mission

Implement the SettingsView, the main configuration screen for M2M. This view organizes all settings into sections: Identity, Theme, Network, Discovery, Security, STUN Servers, and About, using the card-based layout pattern.

## Scope

Covers the full SettingsView including:
- All settings sections with cards
- Settings row layout (label: 120px fixed, value: flex, action)
- Identity: fingerprint + public key display with copy
- Theme: appearance toggle (light/dark/system), accent color
- Network: public IP, NAT type, STUN status, private mode, Tor
- Discovery: LAN/DHT toggles, peer count, privacy notice
- Security: screen capture, clipboard, idle lock, vault actions
- STUN Servers: reachability status, latency, add/remove/reset
- About: version, crypto algorithms

Does NOT cover: Individual security settings details (separate prompt 13), theme details (separate prompt 14), backend operations.

## Files Expected to Be Modified

- `src/views/SettingsView.tsx` — Main component
- `src/styles/components/utilities.css` — View-specific styles
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **Card** (Section 2.3) — Each settings section
- **Button** (Section 2.1) — Action buttons (copy, toggle, test, reset, lock)
- **Input** (Section 2.2) — STUN server input
- **Select** (Section 2.9) — Clipboard timer, idle lock timer
- **Badge** (Section 2.5) — Status indicators (OK/FAIL, online/offline)

## Components to Create

- **SettingsRow** — Label + value + action layout row
- **ToggleSwitch** — ON/OFF slider toggle
- **ThemeToggleButtons** — ☀️ 🌙 🖥️ button group

## Layout Hierarchy

From Design Bible Section 12.7:

```
<SettingsView>
  <!-- Header: Y=0, height 52px -->
  <div class="settings-header">
    <GearIcon size={20} />
    <h1>Settings</h1>
    <Button icon aria-label="Back to Hub"><ArrowLeftIcon /></Button>
  </div>

  <!-- Content: scrollable, Y=52 -->
  <div class="settings-content">
    <!-- Section: Identity -->
    <SectionTitle text="Identity" />
    <Card>
      <SettingsRow label="Fingerprint" value="a1b2:c3d4:..." action={<CopyButton />} />
      <SettingsRow label="Public Key" value="0xabcd1234..." mono />
    </Card>

    <!-- Section: Theme -->
    <SectionTitle text="Theme" />
    <Card>
      <SettingsRow label="Appearance">
        <ThemeToggleButtons current="dark" />
      </SettingsRow>
      <SettingsRow label="Accent Color" value="#6366f1" action={<ResetButton />} />
    </Card>

    <!-- Section: Network -->
    <SectionTitle text="Network" />
    <Card>
      <SettingsRow label="Public IP" value="203.0.113.42" action={<CopyButton />} />
      <SettingsRow label="" action={<Button ghost>Discover via STUN</Button>} />
      <SettingsRow label="NAT Type" value="RestrictedCone" />
      <SettingsRow label="STUN Servers" value="3/4 reachable" />
      <SettingsRow label="Private Mode" action={<ToggleSwitch />} />
      <SettingsRow label="Tor" action={<ToggleSwitch />} />
      <SettingsRow label="" action={<Button ghost>Test Tor</Button>} />
      <SettingsRow label="Connectivity" action={<Button ghost>Check</Button>} />
    </Card>

    <!-- Section: Discovery -->
    <SectionTitle text="Discovery" />
    <Card>
      <SettingsRow label="LAN Discovery" action={<ToggleSwitch />} />
      <SettingsRow label="DHT Discovery" action={<ToggleSwitch />} />
      <SettingsRow label="Discovered Peers" value="3 found" action={<Button ghost>Refresh</Button>} />
      <p class="settings-privacy-notice">⚠️ Both are OFF by default for privacy...</p>
    </Card>

    <!-- Section: Security -->
    <SectionTitle text="Security" />
    <Card>
      <SettingsRow label="Screen Capture" action={<ToggleSwitch />} />
      <SettingsRow label="Clipboard Auto-Clear" action={<Select options={["Off","5s","10s","30s","60s"]} />} />
      <SettingsRow label="Idle Vault Lock" action={<Select options={["Off","1m","5m","10m","30m"]} />} />
      <SettingsRow label="" action={<Button ghost>Lock Now</Button>} />
      <SettingsRow label="" action={<Button ghost>Clear Clipboard</Button>} />
    </Card>

    <!-- Section: STUN Servers -->
    <SectionTitle text="STUN Servers" />
    <Card>
      <SettingsRow label="" value="[OK] stun.l.google.com:19302  12ms" action={<Button icon><CloseIcon /></Button>} />
      <SettingsRow label="" value="[OK] stun1.l.google.com:19302  18ms" action={<Button icon><CloseIcon /></Button>} />
      <SettingsRow label="" value="[FAIL] stun.custom.com:3478" action={<Button icon><CloseIcon /></Button>} />
      <SettingsRow label="" action={<Input placeholder="host:port" compact />} />
      <SettingsRow label="" action={<Button ghost>Add</Button>} />
      <SettingsRow label="" action={<Button ghost>Reset</Button>} />
    </Card>

    <!-- Section: About -->
    <SectionTitle text="About" />
    <Card>
      <SettingsRow label="Version" value="2.5.x" />
      <SettingsRow label="Crypto" value="Ed25519 · X25519 · XChaCha20" />
    </Card>
  </div>
</SettingsView>
```

## Design Implementation Requirements

### Settings Row Exact Layout

From Design Bible Part 2 Section 12.7:

```
┌──────────────────────────────────────────────────────────┐
│  ← 20px padding                                          │
│                                                          │
│  Label (120px fixed)     Value (flex: 1)     [action]    │
│  font: 13px              font: 12px          depends     │
│  500 weight              mono for keys                   │
│  secondary               primary                         │
│                                                          │
│  gap: 12px between label and value                       │
│  gap: 8px between value and action                       │
│  divider: 1px border-default, margin 8px 0              │
└──────────────────────────────────────────────────────────┘
```

### Typography

- Settings section title: --text-sm, --color-text-muted, letter-spacing uppercase (0.08em)
- Settings label: --text-sm (12px), --font-weight-medium (500), --color-text-secondary
- Settings value: --text-sm (12px), --color-text-primary
- Mono values (keys, IPs): --font-mono
- Privacy notice: --text-xs, --color-warning

### Colors

- Card bg: --color-bg-card with glass blur
- Card border: --color-border-default
- Section divider lines: --color-border-default
- OK status: --color-success
- FAIL status: --color-danger
- Toggle ON: --color-accent
- Toggle OFF: --color-bg-input

### Shadows

- Cards: --shadow-card

### Icons

- GearIcon — Settings header (20px)
- ArrowLeftIcon — Back to hub
- CopyIcon — Copy buttons
- SunIcon — Light theme
- MoonIcon — Dark theme
- MonitorIcon — System theme
- CloseIcon — Remove STUN server

## States

From Design Bible Section 3.5 states table:

| Element | State | Visual |
|---------|-------|--------|
| STUN discover | Loading | Button spinner |
| STUN discover | Complete | IP shown, diagnostics updated |
| Tor toggle | On | Checkbox filled, proxy active |
| Tor toggle | Testing | "Testing Tor…" toast |
| Copy fingerprint | Copied | Icon switches to checkmark for 2s |
| Reset accent | Clicked | Color resets to #6366f1 |

## Error Messages

Part 3 Section 21.6:
- S-001 to S-003: STUN server add validation
- S-004: Remove last STUN server blocked
- S-005: Reset to defaults toast
- S-006 to S-009: Tor toggle/test errors
- S-010: Private mode toggle error
- S-011: Theme save error
- S-012: Screen capture toggle error
- S-013: Clipboard clear error
- S-014 to S-015: Vault lock
- S-016 to S-017: Connectivity check

## Accessibility

- Each settings row: proper labels
- Toggle switches: role="switch", aria-checked
- Section headings: proper heading hierarchy (h2, h3)
- Copy buttons: aria-label="Copy {field name}"
- All interactive elements: focus ring via :focus-visible

## Responsive Behavior

- **Desktop (>1000px)**: 32px horizontal padding, cards full width
- **Tablet (600-1000px)**: 24px padding
- **Mobile (<600px)**: 16px padding, settings rows stack vertically (label above value)

## Acceptance Criteria

- [ ] All 7 sections present in correct order
- [ ] Settings row layout matches spec exactly (label 120px, value flex, action)
- [ ] Identity section shows fingerprint + public key with copy
- [ ] Theme section shows appearance toggle buttons + accent color
- [ ] Network section shows IP, NAT, STUN, private mode, Tor, connectivity
- [ ] Discovery section shows LAN/DHT toggles with privacy notice
- [ ] Security section shows all 5 controls
- [ ] STUN Servers section shows list with status + add/remove/reset
- [ ] About section shows version and crypto
- [ ] Copy buttons show checkmark feedback for 2s
- [ ] Toggle switches work correctly (on/off states)
- [ ] STUN add validates format (S-001)
- [ ] Error toasts shown on failures
- [ ] Responsive at all breakpoints
- [ ] Keyboard navigation follows correct focus order

## Self-Review Checklist

- [ ] Follows Design Bible Sections 3.5 and 12.7 exactly
- [ ] Settings row pixel layout matches spec
- [ ] All spacing on 4px grid
- [ ] CSS custom properties used throughout
- [ ] i18n strings match Section 29.9
- [ ] Error messages from Section 21.6
