# M2M Design System — MASTER

> Source of truth for all UI work. Generated with ui-ux-pro-max design intelligence,
> adapted for a privacy-first E2EE desktop messenger. Page-specific deviations live in
> `design-system/pages/<page>.md` and override this file.

## Product identity

- **What**: M2M — private, end-to-end-encrypted desktop messenger (Tauri + React)
- **Feeling**: calm vault, precision instrument, quiet confidence. Premium like
  SwiftUI settings panes / Linear / Things 3 — never flashy, never corporate SaaS.
- **Audience**: security-conscious individuals; family use included (legibility matters)

## Style direction

**Frosted depth on obsidian** (glassmorphism, dark-first):

- Layered translucent surfaces over a deep near-black backdrop (`--color-bg`)
- `backdrop-filter: blur(10–20px)` on elevated layers only (modals, popovers,
  composer, sidebar) — NOT on every card (perf + noise)
- 1px light borders `rgba(255,255,255,0.06–0.14)` to catch edges
- One accent hue drives the whole system (runtime-selectable); derived tints/glow
  MUST all stem from it
- Depth = blur + shadow + saturation, never heavy gradients per element

**Anti-patterns (avoid)**: AI purple-pink gradients everywhere, neon glow spam,
emoji as UI icons, more than 1–2 animated elements per view, decorative noise that
hurts text contrast.

## Color tokens

Dark is default; light theme is fully supported via `[data-theme="light"]`.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--color-bg` | #0A0A0C | #F5F6F8 | app backdrop |
| `--color-surface` | rgba glass layers | solid equivalents | panels/cards |
| `--color-text` | #EDEEF2 ≥4.5:1 | #16181D | primary text |
| `--color-text-muted` | ~60% opacity | ~55% | secondary |
| `--color-accent` | user-set (default #6366F1) | same | interactive tint |
| `--color-success` | #34D399 | #059669 | online/delivered/success |
| `--color-danger` | #F87171 | #DC2626 | destructive/errors |

**Accent rule (fix in progress)**: when the user picks an accent color, ALL derived
tokens must update: `--color-accent-dim`, `--accent-glow`, gradient stops, focus ring.
Use `color-mix(in srgb, var(--color-accent) N%, transparent)` instead of static values.

## Typography

- **Inter Variable** (self-hosted via @fontsource — NO Google Fonts network calls;
  privacy requirement) for everything UI
- **JetBrains Mono** for fingerprints, keys, timestamps, hex
- Scale: 12 caption / 13 body-small / 14 body / 15 emphasis / 18 title / 22 display
- Weight discipline: 400 body, 500 labels/buttons, 650 titles. Never below 400.
- Tabular numerals (`font-variant-numeric`) anywhere counts/timers render

## Spacing & geometry

- 4px base scale (`--space-*` tokens exist — use them, no magic numbers)
- Radii: 8 controls / 12 cards / 16 sheets-modals / full for pills-avatars
- Message bubbles: 14px radius with 4px "tail corner" toward sender side

## Motion (SwiftUI-grade)

- Durations: micro 120ms, standard 200ms, sheet/modal 280ms
- Easing: `cubic-bezier(0.32, 0.72, 0, 1)` (iOS-like decelerate) for enter,
  ease-out for exit; springs feel via slight overshoot ONLY on view transitions
- Animate **transform + opacity only** (compositor-friendly); animate at most 1–2
  elements per view; `prefers-reduced-motion: reduce` kills all non-essential motion
- Every interactive element: hover state (150ms), visible focus ring (2px accent),
  active press scale 0.98

## Iconography

- SVG icon set only (45 icons already in `src/components/ui/icons/`).
  **Zero emoji glyphs in UI chrome.** Emoji remain valid only inside message content.
- Stroke width 1.75, 20px grid, `currentColor`

## Component contracts

- Buttons: variant set (primary/secondary/ghost/danger/icon), sizes sm/md/lg;
  loading state swaps label for spinner; `cursor: pointer`; disabled = 50% + no-events
- Inputs: single focus treatment (border+ring accent), error text slot, clearable
- Modals: overlay blur(12px) + scale-in 0.96→1, Escape close, focus trap, restore
- Toasts: top-right stack, role=alert, auto-dismiss with subtle progress line
- Lists (conversations): 44px min row height, hover reveals actions, selected state
  = accent-tinted glass, never color-only

## Accessibility gates (pre-delivery checklist)

- [ ] Text contrast ≥ 4.5:1 in BOTH themes
- [ ] Visible focus ring on every interactive element incl. modal contents
- [ ] Full keyboard operability (Tab order = visual order, Space activates buttons)
- [ ] Hover-only interactions have keyboard/touch equivalent
- [ ] `prefers-reduced-motion` respected
- [ ] No emoji-as-icon in chrome
- [ ] Text reflows without clipping at 100%–200% zoom
- [ ] Touch/click targets ≥ 32×32px

## Stack notes

- React 19 + hand-rolled CSS (BEM-ish). No new runtime deps for styling.
- CSS custom properties are the token layer; `tokens.css` is canonical.
- Tauri window: 900×700 default — design for 600px minimum width gracefully.
