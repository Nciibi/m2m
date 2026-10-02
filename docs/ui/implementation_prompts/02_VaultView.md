# VaultView — Implementation Prompt

## Mission

Implement the VaultView for creating and unlocking the local encrypted vault. This view handles passphrase entry with a strength meter, confirm field (first-time), tips toggle, and vault creation/unlock flow. Communicates security through glass-morphism, glow animations, and clear feedback.

## Scope

Covers the full VaultView including:
- Lock/unlock icon with glowBreathe animation
- Passphrase input with eye toggle and paste button
- Strength bar with dynamic color mapping and real-time entropy calculation
- Confirm passphrase field (create mode only)
- Tips toggle with diceware advice
- Error states for short/weak/mismatch/wrong passphrase
- Loading state with unlockBounce animation
- Returning user fingerprint hint

Does NOT cover: Backend vault operations (unlock_vault, create_vault commands), identity generation (handled by backend after vault creation).

## Files Expected to Be Modified

- `src/views/VaultView.tsx` — Main component
- `src/styles/components/utilities.css` — Vault-specific styles
- `src/components/ui/icons/LockIcon.tsx` — Lock icon (idle)
- `src/components/ui/icons/UnlockIcon.tsx` — Unlock icon (loading/success)
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **Button** (Section 2.1) — "Create Vault" / "Unlock" primary button
- **Input** (Section 2.2) — Passphrase input, Confirm input (mono variant, with eye toggle)
- **LoadingSpinner** (Section 2.7) — Inline spinner during vault operation

## Components to Create

- **StrengthBar** — 4px height bar with dynamic width and color (red/yellow/green/cyan)
- **VaultTips** — Expandable tips section about strong passphrases
- **FingerprintHint** — Truncated fingerprint display below button

## Layout Hierarchy

```
<VaultView>
  <div class="vault-view">
    <!-- Icon -->
    <div class="vault-icon">
      <LockIcon /> / <UnlockIcon />         <!-- 80×80px, glass container -->
    </div>

    <!-- Title -->
    <h1 class="vault-view__title">           <!-- --text-xl, centered -->
    <p class="vault-view__desc">             <!-- --text-sm, secondary, centered -->
    <p class="vault-view__hint">             <!-- --text-sm, muted -->

    <!-- Form -->
    <form class="vault-form">
      <!-- Passphrase Input -->
      <div class="vault-input-group">
        <Input variant="mono" placeholder="Passphrase" />
        <button class="input__eye">👁</button>     <!-- eye toggle -->
        <button class="input__paste">📋</button>    <!-- paste -->
      </div>

      <!-- Strength Bar (hidden until typing) -->
      <StrengthBar bits={entropy} />                <!-- 4px height -->

      <!-- Confirm Input (create mode only) -->
      <div class="vault-input-group">
        <Input variant="mono" placeholder="Confirm passphrase" />
        <button class="input__eye">👁</button>
      </div>

      <!-- Match/Mismatch indicator -->
      <p class="vault-form__match" />              <!-- success or error text -->

      <!-- Tips Toggle -->
      <button class="vault-tips__toggle">          <!-- accent, underlined -->
        What makes a strong passphrase? [▼]
      </button>
      <VaultTips expanded={showTips} />

      <!-- Submit -->
      <Button variant="default" fullWidth>
        Create Vault / Unlock
      </Button>
    </form>

    <!-- Fingerprint Hint (unlock mode only) -->
    <FingerprintHint fingerprint={partial} />
  </div>
</VaultView>
```

## Design Implementation Requirements

### Exact Spacing

From Design Bible Sections 3.2 & 12.2:

- Icon to title gap: 16px
- Title to description gap: 28px
- Description to hint gap: 4px
- Hint to input gap: 42px
- Input to strength bar gap: 4px
- Strength bar to confirm input gap: 4px
- Confirm input to match text gap: 4px
- Match text to tips toggle gap: 8px
- Tips toggle to submit button gap: 12px
- Submit button to fingerprint hint gap: 16px
- Input max-width: 380px
- All content centered, max-width 380px for form

### Typography

- Title ("Set Up Your Vault" / "Unlock Your Vault"): `--text-xl` (1.1rem / 17.6px), `--font-weight-bold` (700), `--color-text-primary`, centered
- Description: `--text-sm` (0.72rem / 11.5px), `--color-text-secondary`, centered, line-height 1.5
- Hint ("Minimum 12 chars · Argon2id"): `--text-sm` (11px / 0.72rem), `--color-text-muted`, centered
- Input value: `--font-mono`, `--text-base` (13px), `--color-text-primary`
- Strength label: `--text-xs` (10px), weight varies by strength level
- Match/mismatch: `--text-xs` (10px)
- Tips toggle: `--text-sm`, `--color-text-accent`, underlined
- Tips content: `--text-sm`, `--color-text-secondary`
- Fingerprint hint: `--text-xs`, `--font-mono`, `--color-text-muted`, centered, opacity 0.7

### Colors

- Icon container (lock): `--color-accent-gradient` 20% opacity bg, 1px `--color-border-accent` border, `box-shadow: 0 0 40px var(--color-accent-glow-subtle)`
- Icon color: `--color-accent` (idle), `--color-accent-bright` (active)
- Input bg: `--color-bg-input`, border: `--color-border-default`
- Input focus: `--color-bg-input-focus`, border: `--color-border-active`
- Input error: `--color-danger-bg`, border: `--color-danger`
- Strength bar weak (#ef4444), fair (#f59e0b), strong (#10b981), very-strong (#22d3ee)
- Match text: `--color-success`
- Mismatch text: `--color-danger`

### Glass Effects

- Icon container: `backdrop-filter: var(--glass-blur-sm)`

### Shadows

- Input: `--shadow-inner` (default), `0 0 0 3px var(--color-accent-glow)` (focus)
- Input error: `0 0 0 3px var(--color-danger-glow)`
- Button: `--shadow-accent`

### Icons

- `LockIcon` — Locked state (idle, pulse animation)
- `UnlockIcon` — Unlocked state (loading/success)
- `EyeIcon` / `EyeOffIcon` — Passphrase visibility toggle
- `ChevronDownIcon` — Tips toggle expand indicator

## States

### Hover States

- Submit button: translateY(-2px) + shadow-accent-strong, 150ms
- Eye toggle: background brighten
- Tips toggle: text color brighten
- Paste button: background brighten

### Focus States

- Input: `border-color: var(--color-border-active)`, `box-shadow: 0 0 0 3px var(--color-accent-glow)`
- Submit button: `outline: 3px solid var(--color-accent-glow)` via `:focus-visible`
- Eye toggle: focus ring

### Active States

- Submit button: translateY(0) + scale(0.98)

### Disabled States

- Submit button (when passphrase < 12 chars): opacity 0.5, cursor not-allowed, no shadow
- Submit button (during loading): spinner shown, text hidden

### Loading States

- Lock icon animates to unlock: `unlockBounce` 600ms, `--ease-out-back` (spring)
- Button shows `LoadingSpinner` (18px inline ring), text hidden
- Input fields disabled during operation

**Strength bar states** (from Design Bible Section 3.2):

| State | Bar Color | Label Color | Text |
|-------|-----------|-------------|------|
| Hidden (no input) | transparent | — | — |
| Too short (<12 chars) | #ef4444 (danger) | #ef4444 | "Too short (min 12)" |
| Weak (<40 bits) | #ef4444 → #f59e0b gradient | #f59e0b | "Weak — ~32 bits" |
| Fair (40-60 bits) | #f59e0b (warning) | #f59e0b | "Fair — ~52 bits" |
| Strong (60-80 bits) | #10b981 (success) | #10b981 | "Strong — ~72 bits" |
| Very strong (>80 bits) | #22d3ee (cyan) | #22d3ee | "Very Strong — ~96 bits" |

### Empty States

Not applicable (VaultView is the initial state).

### Error States

From Design Bible Part 3 Section 21.1:

| ID | Trigger | Message | Type | Display |
|----|---------|---------|------|---------|
| V-001 | < 12 chars | "Passphrase must be at least 12 characters." | error | inline, field shake |
| V-002 | Mismatch (create) | "Passphrases do not match." | error | inline, field shake |
| V-003 | Entropy < 40 bits | "Passphrase too weak: ~{bits} bits. Use longer (aim for 60+). Try a diceware phrase with 5+ random words." | error | inline, field shake + strength bar red |
| V-004 | Argon2id failure | "Failed to derive encryption key. The vault may be corrupted." | error | inline, 8s |
| V-005 | Wrong passphrase | "Wrong passphrase. Please try again." | error | inline, shake + clear input |
| V-006 | Identity key failure | "Failed to read identity key. The vault may be corrupted. If this persists, you may need to create a new identity." | error | inline, "Repair Vault" button |
| V-007 | DB open failure | "Could not open vault database: {path}. Check file permissions." | error | toast, 8s |
| V-014 | Already unlocked | "Vault is already unlocked." | info | toast, 4s |

**Shake animation**: `shake` 400ms, translateX oscillation, on form/input on error.

## Animations

From Design Bible Sections 5 & 14:

| Animation | Duration | Easing | Property | Trigger |
|-----------|----------|--------|----------|---------|
| `glowBreathe` | 3s | ease-in-out | box-shadow | Continuous (idle lock icon) |
| `pulseRing` | 3s | ease-in-out | transform + opacity | Icon idle state |
| `shake` | 400ms | ease-out-expo | translateX | Error state |
| `unlockBounce` | 600ms | ease-out-back | scale + rotate | Vault unlock success |
| `spin` | 0.6s | linear | rotate | Loading spinner |
| `fadeIn` | 150ms | ease-out-expo | opacity | Strength bar, tips |
| `expandDown` | 300ms | ease-out-expo | max-height, opacity | Tips expand |

**Glow breathe spec:**
```css
@keyframes glowBreathe {
  0%, 100% { box-shadow: 0 0 20px var(--color-accent-glow-subtle); }
  50% { box-shadow: 0 0 40px var(--color-accent-glow); }
}
```

**Shake spec:**
```css
@keyframes shake {
  0%, 100% { transform: translateX(0); }
  10%, 30%, 50%, 70%, 90% { transform: translateX(-4px); }
  20%, 40%, 60%, 80% { transform: translateX(4px); }
}
```

## Keyboard Shortcuts

From Design Bible Sections 6.4 & 13.2:

| Key | Context | Action |
|-----|---------|--------|
| Enter | Form | Submit (Create/Unlock) |
| Tab | Input fields | Next field (passphrase → confirm → submit) |
| Escape | Tips expanded | Close tips |
| Escape | Input focused (empty) | Blur input |

## Mouse Interactions

From Design Bible Section 13.1:

| Element | Hover | Click |
|---------|-------|-------|
| Submit button | translateY(-2px) + shadow | scale(0.98) → submit |
| Eye toggle | Background brighten | Toggle password visibility |
| Paste button | Background brighten | Paste clipboard content |
| Tips toggle | Text brighten | Toggle tips panel |
| Close/X on input | Opacity 0.6→1.0 | Clear input value |

## Interactions

- **Typing**: Strength bar updates in real-time, entropy calculated every keystroke
- **Match check (create)**: Green checkmark + "Passphrases match" when confirm matches passphrase, shown in real-time
- **Mismatch (create)**: Red error "Passphrases do not match", shown in real-time below confirm field
- **Eye toggle**: Toggle input type between "password" and "text"
- **Paste button**: Reads clipboard (via Tauri clipboard API) and inserts into field
- **Error auto-clear**: Error messages auto-clear on next keystroke
- **Tips collapse**: Click toggle to expand/collapse; state persists during session

## Accessibility

From Design Bible Sections 6 & 16:

- Passphrase input: `aria-label="Passphrase"`, `aria-describedby` linking to error message when in error state
- Confirm input: `aria-label="Confirm passphrase"`
- Error states: `aria-invalid="true"` on the relevant input, `aria-describedby="error-{id}"`
- Eye toggle: `aria-label="Toggle passphrase visibility"`, `aria-pressed` reflecting state
- Paste button: `aria-label="Paste from clipboard"`
- Strength bar: `role="progressbar"`, `aria-valuenow`, `aria-valuemin="0"`, `aria-valuemax="100"`
- Match/mismatch text: `aria-live="polite"` for dynamic updates
- Tips toggle: `aria-expanded` reflecting panel state, `aria-controls` pointing to tips panel
- Submit button loading: `aria-busy="true"`
- All interactive elements must have visible focus ring via `:focus-visible`
- Input `autocomplete="off"` (security)

## Responsive Behavior

From Design Bible Section 7:

- **Desktop (>1000px)**: Max-width 380px for form, centered
- **Tablet (600-1000px)**: Reduced padding, same max-width
- **Mobile (<600px)**: Full-width form (no max-width constraint), full-bleed container
- Icon reduces to 64px on mobile

## Performance Considerations

From Design Bible Section 20:

- Vault unlock: < 1.5s (Argon2id derivation + decryption — backend handles this; frontend shows loading state)
- Theme switch: < 50ms
- Strength bar updates: debounced at 50ms (not every keystroke)
- No layout shifts: strength bar area reserved (shown/hidden with opacity, not display)
- `will-change: transform` on the lock icon (continuous glow animation)

## Security Considerations

From Design Bible Sections 8 & 38:

- Eye toggle allows user to verify what they typed
- Input `autocomplete="off"` prevents browser save
- No passphrase stored in state — sent directly via IPC and cleared after
- Vault lock zeroizes keys in memory (backend)
- Fingerprint hint shows partial only (first 12 chars)
- No autocomplete on any vault form field
- Clipboard paste handled via Tauri API (not browser navigator.clipboard)
- Argon2id protects stored keys (mentioned in hint)
- Error messages safe: no stack traces, no file paths exposed to user (except V-007 which shows path for debugging)

## Edge Cases

From Design Bible Sections 9 & 32:

- **Passphrase too short**: Show error V-001 immediately, submit button disabled
- **Passphrase mismatch (create)**: Real-time feedback, submit disabled
- **Too weak passphrase**: Show error V-003, submit allowed but warns
- **Wrong passphrase (unlock)**: Error V-005, field shakes + clears
- **Vault corrupted**: Error V-006, "Repair Vault" button shown
- **Already unlocked**: Toast V-014, navigate away
- **Rapid submit clicks**: Button shows loading spinner, input disabled, prevent double-submit
- **Empty submit**: Show "minimum 12 chars" error
- **Tips toggle fast clicks**: Expand animation can be interrupted (use max-height transition)
- **Paste on empty field**: Works normally; paste on non-empty field appends at cursor
- **Escape when input has value**: Clears the field (from Section 2.2 Input specs)

## Acceptance Criteria

- [ ] Lock icon displays with glowBreathe animation (3s cycle)
- [ ] Title and description match the mode (create/unlock)
- [ ] Passphrase input uses mono font, eye toggle visible
- [ ] Paste button visible and functional (reads from Tauri clipboard)
- [ ] Strength bar hidden initially, appears on first keystroke
- [ ] Strength bar color and label match the 5 strength states
- [ ] Confirm field only visible in create mode
- [ ] Real-time match check with green checkmark on match
- [ ] Real-time mismatch error on mismatch
- [ ] Tips toggle expands/collapses with expandDown animation (300ms)
- [ ] Submit button disabled when passphrase < 12 chars
- [ ] Loading state shows unlockBounce animation on icon + spinner in button
- [ ] Error states show correct messages with shake animation (400ms)
- [ ] Errors auto-clear on next keystroke
- [ ] Input `autocomplete="off"` on all fields
- [ ] All animations respect prefers-reduced-motion
- [ ] All text meets WCAG contrast requirements
- [ ] Button hover/focus/active states match spec
- [ ] Responsive at desktop, tablet, and mobile breakpoints
- [ ] i18n strings match the string catalog (Part 3 Section 29.2)

## Self-Review Checklist

- [ ] Does the layout match the pixel specs in Design Bible Sections 12.2 & 12.3?
- [ ] Are all spacing values multiples of the 4px grid?
- [ ] Are all CSS custom properties from the token system used (not hardcoded)?
- [ ] Are animations using only transform and opacity (except box-shadow for glowBreathe)?
- [ ] Is prefers-reduced-motion respected?
- [ ] Are all icon-only buttons accessible with aria-label?
- [ ] Does the component handle all states (idle, typing, match, mismatch, strength levels, loading, error, tips)?
- [ ] Are keyboard interactions implemented (Enter, Tab, Escape)?
- [ ] Are all text strings using the i18n system (not hardcoded)?
- [ ] No redesign, no improvisation, no invented layouts — follows Design Bible exactly
