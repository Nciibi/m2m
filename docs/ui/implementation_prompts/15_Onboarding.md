# Onboarding — Implementation Prompt

## Mission

Implement the 4-step onboarding wizard that educates first-time users about M2M's security and privacy features. This wizard appears after the loading splash during first launch.

## Scope

Covers the onboarding wizard including:
- 4 information steps (Welcome, Identity, Encryption, Ready)
- Step indicator with 4 dots (24×24px)
- Navigation controls (Back, Get Started/Next/Start Messaging)
- Step content with icon, title, description
- Slide animations between steps

Does NOT cover: The SetupView loading splash, Vault creation (navigates to VaultView after completion).

## Files Expected to Be Modified

- `src/components/OnboardingWizard.tsx` — Component
- `src/styles/components/utilities.css` — Component styles
- `src/hooks/useTranslation.ts` — For i18n strings

## Components to Reuse

- **Button** (Section 2.1) — Navigation buttons
- **Badge** (Section 2.5) — Step indicator dots (custom styled)

## Components to Create

- **StepIndicator** — 4 dots with active/done/next states
- **StepCard** — Individual step content display

## Layout Hierarchy

```
<OnboardingWizard>
  <div class="onboarding">
    <!-- Icon -->
    <div class="onboarding-icon">
      <span>{step.icon}</span>        <!-- 48px emoji in glass container -->
    </div>

    <!-- Title -->
    <h1 class="onboarding__title">{step.title}</h1>    <!-- --text-2xl, centered -->

    <!-- Description -->
    <p class="onboarding__desc">{step.desc}</p>         <!-- --text-md, secondary -->

    <!-- Step Indicator -->
    <StepIndicator current={stepIndex} total={4}>
      <div class="step-dot step-dot--active" />        <!-- accent fill -->
      <div class="step-dot step-dot--done" />          <!-- success fill + check -->
      <div class="step-dot" />                          <!-- outlined (next) -->
      <div class="step-dot" />                          <!-- outlined -->
    </StepIndicator>

    <!-- Navigation -->
    <div class="onboarding__nav">
      {stepIndex > 1 && <Button variant="ghost" onClick={prev}>Back</Button>}
      <Button variant="default" onClick={nextOrFinish}>
        {stepIndex === 4 ? "Start Messaging" : stepIndex === 1 ? "Get Started" : "Next"}
      </Button>
    </div>
  </div>
</OnboardingWizard>
```

## Step Content

From Design Bible Section 3.1:

| Step | Title | Icon | Description |
|------|-------|------|-------------|
| 1 | Welcome to M2M | 🚀 | "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking." |
| 2 | Your Identity is Local | 🔑 | "Your keys are generated on this device and never leave it." |
| 3 | End-to-End Encrypted | 🔒 | "Messages use X3DH + Double Ratchet (Signal protocol). Ed25519 signing, X25519 key exchange, XChaCha20-Poly1305 encryption." |
| 4 | Ready to Go! | ✅ | "Share your invite link with a trusted peer to start chatting. Both sides must generate and share invites." |

## Step Indicator Specs

```
  ●  ○  ○  ○    ← active: accent fill, done: success fill + checkmark, next: outlined
```
- 24×24px dots, --space-sm (12px) gap
- Active: --color-accent fill
- Done: --color-success fill, checkmark icon inside
- Next (future): 2px --color-border-default border, transparent fill
- Clickable only on completed steps

## Animations

| Animation | Duration | Easing | Trigger |
|-----------|----------|--------|---------|
| slideInRight | 500ms | ease-out-expo | Forward step |
| slideInLeft | 500ms | ease-out-expo | Backward step |
| fadeIn | 150ms | ease-out-expo | Icon crossfade |
| popIn | 300ms | ease-out-back | Step dot state change |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Enter | Next/Start (forward) |
| Escape | Back (when not on step 1) |

## Accessibility

- Step indicator: role="tablist", each dot role="tab"
- Active dot: aria-selected="true"
- Done dot: aria-label="Step {n} completed"
- Next dot: aria-label="Step {n}"
- Navigation buttons: descriptive aria-label
- Step content: role="tabpanel", aria-labelledby pointing to step indicator

## Acceptance Criteria

- [ ] Step indicator shows 4 dots with correct states
- [ ] Step 1 shows "Welcome to M2M" with 🚀 icon
- [ ] Step 2 shows "Your Identity is Local" with 🔑 icon
- [ ] Step 3 shows "End-to-End Encrypted" with 🔒 icon
- [ ] Step 4 shows "Ready to Go!" with ✅ icon
- [ ] "Get Started" on step 1, "Next" on 2-3, "Start Messaging" on 4
- [ ] "Back" button visible on steps 2-4
- [ ] Forward animation: slideInRight (500ms)
- [ ] Backward animation: slideInLeft (500ms)
- [ ] Step indicator dots update correctly on navigation
- [ ] Done dots show checkmark + success color
- [ ] "Start Messaging" navigates to VaultView (create mode)
- [ ] Onboarding only shown on first launch
- [ ] All text uses i18n strings

## Self-Review Checklist

- [ ] Follows Design Bible Section 3.1 exactly
- [ ] Step indicator pixel specs match
- [ ] i18n strings match Section 29.3
- [ ] Animations use transform/opacity only
