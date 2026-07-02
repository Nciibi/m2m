# SecuritySettings — Implementation Prompt

## Mission

Implement the Security section of SettingsView, providing controls for screen capture protection, clipboard auto-clear, idle vault lock, and manual vault/clipboard actions.

## Scope

Covers the Security settings card including:
- Screen capture protection toggle (Windows native, macOS/Linux stub)
- Clipboard auto-clear timer select (Off, 5s, 10s, 30s, 60s)
- Idle vault lock timer select (Off, 1m, 5m, 10m, 30m)
- "Lock Now" button
- "Clear Clipboard" button
- All error/feedback toasts

Does NOT cover: Full SettingsView shell (handled by prompt 12), native FFI implementations.

## Files Expected to Be Modified

- `src/components/SecuritySettings.tsx` — Component
- `src/styles/components/utilities.css` — Component styles

## Components to Reuse

- **Button** (Section 2.1) — Lock Now, Clear Clipboard
- **Select** (Section 2.9) — Timer dropdowns
- **ToggleSwitch** (from prompt 12) — On/off toggles

## Layout Hierarchy

```
<Card title="Security">
  <SettingsRow label="Screen Capture" action={<ToggleSwitch />} />
  <SettingsRow label="Clipboard Clear" action={<Select options={timerOptions} />} />
  <SettingsRow label="Idle Lock" action={<Select options={idleOptions} />} />
  <hr class="settings-divider" />
  <SettingsRow label="" action={<Button ghost>Lock Now</Button>} />
  <SettingsRow label="" action={<Button ghost>Clear Clipboard</Button>} />
</Card>
```

## Design Implementation Requirements

### Typography

- Labels: --text-sm, 500 weight, secondary color
- Select values: --text-sm, primary color

### Error Messages

From Design Bible Part 3 Section 21.7:

| ID | Trigger | Message | Type |
|----|---------|---------|------|
| SEC-001 | Verification confirm | "Peer verified. Always verify fingerprints via a trusted out-of-band channel." | success toast, 4s |
| SEC-003 | Clipboard auto-clear set | "Clipboard will auto-clear in {n} seconds." | info toast, 3s |
| SEC-004 | Clipboard cleared | "Clipboard cleared." | info toast, 3s |
| SEC-005 | Screen capture enabled | "Screen capture protection enabled. Your window will not appear in screenshots." | info toast, 4s |
| SEC-006 | Screen capture disabled | "Screen capture protection disabled." | info toast, 4s |
| SEC-007 | Idle lock enabled | "Vault will auto-lock after {n} minutes of inactivity." | info toast, 4s |
| SEC-008 | Vault auto-locked | "Vault auto-locked due to inactivity." | info toast, 4s |
| S-012 | Screen capture toggle fail | "Failed to toggle screen capture protection." | warning toast, 4s |
| S-013 | Clipboard clear fail | "Failed to clear clipboard." | warning toast, 4s |
| S-015 | Vault lock fail | "Failed to lock vault: {error}" | error toast, 5s |

## Security Considerations

- Screen capture protection uses Windows SetWindowDisplayAffinity (WDA_EXCLUDEFROMCAPTURE)
- macOS and Linux use no-op stubs
- Clipboard clear uses Tauri clipboard API
- Auto-lock zeroizes keys in memory
- All timers are client-side only (no server involved)

## Acceptance Criteria

- [ ] Screen capture toggle with ON/OFF state
- [ ] Clipboard auto-clear select with all options
- [ ] Idle vault lock select with all options
- [ ] Lock Now button locks vault (with toast feedback)
- [ ] Clear Clipboard button clears clipboard (with toast feedback)
- [ ] All settings persist across app restarts
- [ ] Error toasts shown on failures
- [ ] Correct feedback toasts shown on state changes

## Self-Review Checklist

- [ ] Follows Design Bible Section 8.2 and 21.7
- [ ] Privacy-first defaults: all OFF
- [ ] i18n strings match catalog
