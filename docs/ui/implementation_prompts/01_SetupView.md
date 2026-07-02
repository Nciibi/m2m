# SetupView — Implementation Prompt

## Mission

Implement the SetupView (loading splash + onboarding wizard) that appears on application launch. This view handles identity key generation display, the 4-step first-run onboarding wizard, and the loading transition for returning users.

## Scope

Covers the full SetupView including:
- Loading splash with sonar ring animation and crypto badge
- 4-step onboarding wizard (Welcome → Identity → Encryption → Ready)
- Step indicator dots with active/done/next states
- Loading state for key generation (2-3s)

Does NOT cover: Vault creation (goes to VaultView after setup), identity generation backend, app shell layout (handled by AppShell).

## Files Expected to Be Modified

- `src/views/SetupView.tsx` — Main component
- `src/styles/components/utilities.css` — View-specific styles (if needed)
- `src/components/ui/icons/KeyIcon.tsx` — Key/identity icon
- `src/components/ui/icons/LockIcon.tsx` — Lock icon for encryption step
- `src/hooks/useTranslation.ts` — For i18n strings (see string catalog)

## Components to Reuse

- **Button** (Section 2.1) — "Get Started", "Next", "Back", "Start Messaging" buttons
- **LoadingSpinner** (Section 2.7) — Inline spinner for loading state

## Components to Create

- **StepIndicator** — 3 dots (24×24px, `--space-sm` gap), active: accent fill, done: success fill + checkmark, next: outlined
- **StepContent** — Renders current step's icon (48px), title (`--text-2xl`), description (`--text-md`)
- **SonarRing** — 80×80px glass icon container with expanding ring animation

## Layout Hierarchy

```
<SetupView>
  <div class="setup-view">
    <!-- Loading State -->
    <div class="setup-loading">
      <SonarRing icon="🔑" />              <!-- 80×80px, centered -->
      <h1 class="setup-loading__title">     <!-- --text-2xl, centered -->
      <p class="setup-loading__desc">       <!-- --text-md, secondary -->
      <div class="setup-loading__dots">     <!-- 3 bouncing dots -->
        <span>●</span><span>●</span><span>●</span>
      </div>
      <div class="setup-crypto-badge">      <!-- Badge: Ed25519 · X25519 · XChaCha -->
    </div>

    <!-- Onboarding Wizard -->
    <div class="setup-onboarding">
      <SonarRing icon="{step.icon}" />       <!-- changes per step -->
      <div class="setup-step__content">
        <StepIndicator currentStep={index} totalSteps={4} />
        <StepContent step={stepData} />
      </div>
      <div class="setup-step__nav">
        <Button variant="ghost" onClick={back}>Back</Button>   <!-- hidden on step 1 -->
        <Button variant="default" onClick={next/start}>
          {step === 4 ? "Start Messaging" : step === 1 ? "Get Started" : "Next"}
        </Button>
      </div>
    </div>
  </div>
</SetupView>
```

## Design Implementation Requirements

### Exact Spacing

From Design Bible Section 3.1 & 12.1:

- Icon container: 80×80px, centered
- Icon to title gap: 20px (Y=140 to Y=160)
- Title to description gap: 6px line-height inside desc, 30px total block (Y=160 to Y=190)
- Description to loading dots gap: 40px (Y=190 to Y=230)
- Dots to crypto badge gap: 40px (Y=230 to Y=270)
- Content max-width: 480px
- All content centered horizontally within app-shell

### Typography

- Title ("Initializing Secure Enclave"): `--text-2xl` (1.3rem / 20.8px), `--font-weight-bold` (700), `--color-text-primary`
- Description: `--text-md` (0.85rem / 13.6px), `--color-text-secondary`, line-height: 1.6
- Crypto badge: `--text-xs` (0.65rem / 10.4px), `--font-mono`, `--color-text-muted`
- Step title: `--text-2xl` (1.3rem), 700 weight
- Step description: `--text-md`, secondary color, line-height 1.6

### Colors

- Title text: `--color-text-primary` (#f8fafc dark / #0f172a light)
- Description text: `--color-text-secondary` (#cbd5e1 dark / #475569 light)
- Crypto badge text: `--color-text-muted` (#94a3b8 dark / #64748b light)
- Crypto badge bg: `--color-bg-card` with `backdrop-filter: var(--glass-blur-sm)`
- Loading dots: `--color-accent-bright` (#c7d2fe)
- Icon container: `--color-accent-gradient` background, `--shadow-accent-strong`

### Glass Effects

- Crypto badge: `background: var(--color-bg-card)`, `backdrop-filter: var(--glass-blur-sm)` (blur(20px) saturate(200%))

### Shadows

- Icon container: `--shadow-accent-strong` (0 8px 24px rgba(99, 102, 241, 0.4)), plus `0 0 60px var(--color-accent-glow)`

### Icons

- `KeyIcon` — 🔑 for identity step
- `LockIcon` — 🔒 for encryption step
- Step icons as emoji: 🚀, 🔑, 🔒, ✅ (48px each, inside glass container)

## States

### Hover States

- Buttons: translateY(-2px) + shadow-accent-strong, 150ms ease-out-expo
- "Back" ghost button: `--color-text-secondary` → `--color-text-primary`

### Focus States

- Buttons: `outline: 3px solid var(--color-accent-glow)`, `:focus-visible` only

### Active States

- Buttons: translateY(0) + scale(0.98), 100ms ease-out-expo

### Disabled States

- "Back" button hidden on step 1
- "Next" button disabled briefly during transition

### Loading States

**Initial loading (2-3s):**
- SonarRing animation: `sonarRing` 2.5s, 0s/0.6s/1.2s staggered rings
- 3 loading dots: `dotBounce` 1.4s with 0s, 0.2s, 0.4s stagger
- Dots: 8px each, 6px gap, `--color-accent-bright`

**Step transition:**
- Content slides: `slideInRight` 500ms (forward), `slideInLeft` 500ms (back)
- Icon crossfade: 300ms ease-out-expo

### Empty States

Not applicable for SetupView (it's a loading state itself).

### Error States

From Design Bible Part 3 Section 21.10:

| ID | Trigger | Message | Type | Display |
|----|---------|---------|------|---------|
| O-001 | Key generation failure | "Failed to generate identity keys. This is a critical error. Please restart." | error | full-screen, "Restart" button |
| O-002 | Key storage failure | "Failed to store identity keys. Check disk space and permissions." | error | full-screen, "Retry" button |
| O-003 | Network init failure | "Failed to initialize networking. Some features may not work." | warning | toast, 6s |
| O-004 | Database init failure | "Failed to initialize local database: {error}" | error | toast, 8s |

For critical errors (O-001, O-002): replace entire view content with error state — AlertTriangle icon 48px warning, error message, action button.

## Animations

From Design Bible Sections 5 & 14:

| Animation | Duration | Easing | Property | Trigger |
|-----------|----------|--------|----------|---------|
| `appEntrance` | 800ms | ease-out-expo | translateY + opacity | App mount |
| `sonarRing` | 2.5s | ease-out-expo | transform + opacity | Continuous (loading) |
| `dotBounce` | 1.4s | ease-in-out | scale + opacity | Continuous (loading) |
| `slideInRight` | 500ms | ease-out-expo | translateX | Step forward |
| `slideInLeft` | 500ms | ease-out-expo | translateX | Step backward |
| `spin` | 0.6s | linear | rotate | Any spinner |
| `fadeIn` | 150ms | ease-out-expo | opacity | Element appearance |

**Sonar ring spec:**
```css
@keyframes sonarRing {
  0% { transform: scale(1); opacity: 0.6; }
  50% { transform: scale(1.3); opacity: 0; }
  100% { transform: scale(1.3); opacity: 0; }
}
/* Three rings staggered: 0s, 0.6s, 1.2s */
```

**Performance rules:**
- Animate only transform and opacity
- Use will-change: transform on the sonar ring elements
- Respect prefers-reduced-motion

## Keyboard Shortcuts

From Design Bible Sections 6.4 & 13.2:

| Key | Context | Action |
|-----|---------|--------|
| Enter | Onboarding | Activate next/finish button |
| Escape | Onboarding (step 2-4) | Go back one step |
| Tab | Onboarding | Focus navigation buttons |

No arrow key navigation for the wizard (step buttons are discrete).

## Mouse Interactions

From Design Bible Section 13.1:

| Element | Hover | Click |
|---------|-------|-------|
| Button (default) | translateY(-2px) + shadow | scale(0.98) |
| Button (ghost) | text color change | Execute action |
| Step indicator dot | cursor pointer | Navigate to step (completed steps only) |

## Interactions

- **Auto-advance from loading**: After 2-3s, if first run: show onboarding step 1; otherwise: navigate to VaultView
- **Onboarding navigation**: Sequential — user must click Next/Start to advance
- **Step indicator**: Clickable only on completed steps (can revisit)
- **Forward navigation**: "Get Started" (step 1), "Next" (steps 2-3), "Start Messaging" (step 4)
- **Back navigation**: "Back" ghost button shown on steps 2-4, returns to previous step
- **Completion**: "Start Messaging" sets `first_run_complete`, then navigates to VaultView

## Accessibility

From Design Bible Sections 6 & 16:

- Icon in container: `role="presentation"` or `aria-hidden="true"` (decorative)
- Loading state: `role="status"`, `aria-label="Loading identity keys"`
- Step indicator: `role="tablist"`, each dot `role="tab"` with `aria-selected`
- Active step content: `aria-current="step"`
- Buttons: proper `aria-label` if icon-only
- Focus ring: `outline: 2px solid var(--color-accent); outline-offset: 2px` via `:focus-visible`
- All text must meet WCAG AAA contrast (16:1 on dark bg): `--color-text-primary` and `--color-text-secondary`
- `prefers-reduced-motion`: disable all animations

## Responsive Behavior

From Design Bible Section 7:

- **Desktop (>1000px)**: Max-width 480px centered content, floating glass shell
- **Tablet (600-1000px)**: Max-width 100%, reduced padding to --space-xl
- **Mobile (<600px)**: Full-bleed, padding --space-md, icon 64px (reduced from 80px)

## Performance Considerations

From Design Bible Section 20:

- Cold start: < 3s (loading state handles this)
- Warm start: < 1s
- First contentful paint: < 500ms
- No layout shifts (CLS = 0)
- Sonar ring on GPU (transform only)
- Dot animation on GPU (transform only)

## Security Considerations

From Design Bible Sections 8 & 38:

- Never expose private keys in any UI — only display crypto badge mentioning algorithms
- No message content logged during setup
- All tracing is redacted: `tracing::warn!(error = %e)`
- Key generation is a one-time event; never re-display keys

## Edge Cases

From Design Bible Sections 9 & 32:

- **Key generation fails**: Show full-screen error with "Restart" button (O-001)
- **Storage fails on first run**: Show full-screen error with "Retry" button (O-002)
- **User navigates back from step 1**: "Back" button hidden on step 1
- **Rapid clicking**: Debounce navigation buttons during transition animation
- **Onboarding already completed (returning user)**: Skip onboarding, show loading → navigate to VaultView
- **Network init failure**: Toast warning, continue to VaultView (non-fatal)

## Acceptance Criteria

- [ ] Loading splash appears on app startup with sonar ring animation
- [ ] SonarRing has 3 staggered expanding rings (0s, 0.6s, 1.2s)
- [ ] Loading dots bounce continuously during key generation
- [ ] Crypto badge shows "Ed25519 · X25519 · XChaCha20-Poly1305"
- [ ] First-time users see 4-step onboarding wizard after loading
- [ ] Step indicator shows 4 dots with correct active/done/next states
- [ ] Each step has correct icon, title, and description from the spec
- [ ] Navigation buttons work: Get Started → Next → Next → Start Messaging
- [ ] "Back" button appears on steps 2-4 and returns to previous step
- [ ] "Back" button hidden on step 1
- [ ] Step transitions animate with slideInRight/slideInLeft (500ms)
- [ ] Returning users skip wizard, go from loading to VaultView
- [ ] Error states show correct messages with action buttons
- [ ] All animations respect prefers-reduced-motion
- [ ] All text meets WCAG contrast requirements
- [ ] Loading completes within 3s (cold) / 1s (warm)
- [ ] Responsive at desktop, tablet, and mobile breakpoints
- [ ] Button hover/focus/active states match spec
- [ ] i18n strings match the string catalog (Part 3 Section 29.3)

## Self-Review Checklist

- [ ] Does the layout match the pixel specs in Design Bible Section 12.1?
- [ ] Are all spacing values multiples of the 4px grid?
- [ ] Are all CSS custom properties from the token system used (not hardcoded)?
- [ ] Are animations using only `transform` and `opacity`?
- [ ] Is `prefers-reduced-motion` respected?
- [ ] Are all icon-only buttons accessible with `aria-label`?
- [ ] Does the component handle all states (loading, steps, error)?
- [ ] Are keyboard interactions implemented (Enter, Escape)?
- [ ] Are all text strings using the i18n system (not hardcoded)?
- [ ] No redesign, no improvisation, no invented layouts — follows Design Bible exactly
