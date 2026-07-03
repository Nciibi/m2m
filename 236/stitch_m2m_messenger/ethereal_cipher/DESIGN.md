---
name: Ethereal Cipher
colors:
  surface: '#111319'
  surface-dim: '#111319'
  surface-bright: '#373940'
  surface-container-lowest: '#0c0e14'
  surface-container-low: '#1a1b21'
  surface-container: '#1e1f26'
  surface-container-high: '#282a30'
  surface-container-highest: '#33343b'
  on-surface: '#e2e2ea'
  on-surface-variant: '#c7c4d7'
  inverse-surface: '#e2e2ea'
  inverse-on-surface: '#2f3037'
  outline: '#908fa0'
  outline-variant: '#464554'
  surface-tint: '#c0c1ff'
  primary: '#c0c1ff'
  on-primary: '#1000a9'
  primary-container: '#8083ff'
  on-primary-container: '#0d0096'
  inverse-primary: '#494bd6'
  secondary: '#4edea3'
  on-secondary: '#003824'
  secondary-container: '#00a572'
  on-secondary-container: '#00311f'
  tertiary: '#ffb3ad'
  on-tertiary: '#68000a'
  tertiary-container: '#ff5451'
  on-tertiary-container: '#5c0008'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e1e0ff'
  primary-fixed-dim: '#c0c1ff'
  on-primary-fixed: '#07006c'
  on-primary-fixed-variant: '#2f2ebe'
  secondary-fixed: '#6ffbbe'
  secondary-fixed-dim: '#4edea3'
  on-secondary-fixed: '#002113'
  on-secondary-fixed-variant: '#005236'
  tertiary-fixed: '#ffdad7'
  tertiary-fixed-dim: '#ffb3ad'
  on-tertiary-fixed: '#410004'
  on-tertiary-fixed-variant: '#930013'
  background: '#111319'
  on-background: '#e2e2ea'
  surface-variant: '#33343b'
  surface-glass: rgba(12, 14, 24, 0.82)
  surface-elevated: rgba(28, 30, 44, 0.7)
  accent-bright: '#c7d2fe'
  warning: '#f59e0b'
  border-glass: rgba(255, 255, 255, 0.08)
  input-bg: rgba(255, 255, 255, 0.05)
  text-primary: '#f1f5f9'
  text-secondary: '#cbd5e1'
  text-muted: '#94a3b8'
typography:
  headline-xl:
    fontFamily: Inter
    fontSize: 1.75rem
    fontWeight: '700'
    lineHeight: '1.2'
    letterSpacing: -0.02em
  headline-lg:
    fontFamily: Inter
    fontSize: 1.4rem
    fontWeight: '600'
    lineHeight: '1.3'
  headline-md:
    fontFamily: Inter
    fontSize: 1.2rem
    fontWeight: '600'
    lineHeight: '1.4'
  body-lg:
    fontFamily: Inter
    fontSize: 1.05rem
    fontWeight: '400'
    lineHeight: '1.5'
  body-md:
    fontFamily: Inter
    fontSize: 0.9375rem
    fontWeight: '400'
    lineHeight: '1.5'
  body-sm:
    fontFamily: Inter
    fontSize: 0.8125rem
    fontWeight: '400'
    lineHeight: '1.5'
  label-mono:
    fontFamily: JetBrains Mono
    fontSize: 0.75rem
    fontWeight: '500'
    lineHeight: '1.4'
    letterSpacing: 0.02em
  label-xs:
    fontFamily: Inter
    fontSize: 0.68rem
    fontWeight: '600'
    lineHeight: '1.2'
  headline-xl-mobile:
    fontFamily: Inter
    fontSize: 1.4rem
    fontWeight: '700'
    lineHeight: '1.2'
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  base: 4px
  xs: 8px
  sm: 12px
  md: 16px
  lg: 24px
  xl: 32px
  2xl: 48px
  3xl: 64px
  container-max: 1000px
  gutter: 16px
---

## Brand & Style

This design system is engineered for a premium, privacy-centric communication experience. The visual narrative balances high-tech security with a sophisticated, desktop-grade aesthetic—described as "Signal meets macOS." 

The design style is **Glassmorphism**, characterized by deep backdrop blurs, translucent layers, and high-fidelity edge lighting that simulates physical glass. This approach conveys transparency (integrity) through visual depth. The interface should feel like a series of light-refracting objects floating in a dark, atmospheric void. High-contrast typography and precise monochromatic accents reinforce a sense of elite encryption and technical reliability.

Targeting users who value both aesthetic polish and absolute data sovereignty, the UI evokes an emotional response of calm, focused security and professional-grade utility.

## Colors

The palette is anchored by a deep navy-black foundation to minimize eye strain and emphasize the "void" space. The primary indigo serves as the high-energy signal for actions and encryption states, while secondary and tertiary colors are reserved for semantic status indicators.

- **Primary (Indigo):** Used for primary buttons, active states, and "Secure Session" glows.
- **Surface Strategy:** Backgrounds use `#030408`. Layers above this use translucent variants of navy with a high-saturation (180%) backdrop filter to maintain legibility.
- **Borders:** All glass containers must use a 1px border at 8% white opacity to simulate the refraction of light on a glass edge.
- **Accents:** Use "Accent Bright" for hover states or subtle highlights to ensure the indigo doesn't become muddy against the dark background.

## Typography

This design system uses a dual-font strategy:
1. **Inter:** The primary workhorse for all UI navigation and message content. Its high legibility and neutral character provide a modern, "App-like" feel similar to premium desktop OS environments.
2. **JetBrains Mono:** Reserved for technical data, public keys, fingerprints, and timestamps. This font choice signals the underlying cryptographic nature of the product.

**Scale and Weight:**
Headlines should utilize semibold or bold weights with tighter letter-spacing for a compact, authoritative look. Body text stays strictly at regular weight (400) to ensure the glass backdrop doesn't interfere with readability. Use the `label-xs` for metadata like timestamps or "read" receipts.

## Layout & Spacing

The layout follows a **Fixed Grid** philosophy for the core application container to mimic a focused desktop application. The primary interface is a centered floating glass vessel with a maximum width of 1000px.

**Rhythm:**
A strict 4px base unit controls all internal spacing. Margins between disparate components (like the sidebar and chat view) should use `lg` (24px) or `xl` (32px) to allow the background blur to breathe.

**Breakpoints:**
- **Desktop (1024px+):** Centered floating card with 48px margins from screen edges.
- **Tablet (768px - 1023px):** Edge-to-edge container with internal 16px padding; sidebar collapses into a drawer.
- **Mobile (< 767px):** Full-screen backgrounds; glassmorphism intensity is reduced (24px blur) to improve performance.

## Elevation & Depth

Depth is conveyed through **Backdrop Blurs** and **Tonal Layers** rather than standard shadows.

1. **Level 0 (Base):** The dark navy background (`#030408`).
2. **Level 1 (Main Container):** `surface-glass` with a 48px backdrop blur and a 1px border at 8% white.
3. **Level 2 (Cards/Bubbles):** `surface-elevated` with a 24px backdrop blur.
4. **Modals/Popovers:** High-contrast elevation with a deep `0 25px 80px rgba(0, 0, 0, 0.7)` shadow to separate it from the main glass vessel.

**Edge Lighting:**
All Level 1 and Level 2 containers must include a subtle linear gradient on the border (top to bottom: white at 10% to white at 2%) to simulate a light source coming from above.

## Shapes

The shape language is "Hyper-Soft," utilizing large radii to contrast with the technical, sharp nature of encryption.

- **Main Container:** 32px (`rounded-2xl`+) to create a friendly, modern frame.
- **Cards & Chat Bubbles:** 12px to 16px to maintain a compact yet approachable look.
- **Buttons & Toggles:** Should lean toward pill-shaped (Full) for high-frequency interaction points, mimicking macOS interface standards.

## Components

**Buttons:**
- **Primary:** Solid Indigo gradient with a subtle outer glow (`0 0 15px rgba(99, 102, 241, 0.3)`).
- **Secondary:** Transparent with the 1px white border and 5% white hover fill.

**Chat Bubbles:**
- **Sent:** Indigo to Violet gradient, right-aligned, white text.
- **Received:** `surface-elevated` background, left-aligned, `text-primary`.
- **Spacing:** Group messages from the same user with 4px vertical gaps; different users with 16px gaps.

**Inputs & Toggles:**
- **Text Inputs:** Use `input-bg` with a subtle 1px border. On focus, the border transitions to Primary Indigo with a 4px outer glow.
- **Toggles:** macOS style. Pill-shaped track with a white circular thumb. Use Primary Indigo for the "On" state.

**Status Indicators:**
- **Online:** Success Green circle with a 10px outer blur glow.
- **Encryption Banner:** A full-width subtle indigo tint (`rgba(99, 102, 241, 0.05)`) at the top of the chat with a lock icon.

**Vault Strength Meter:**
- A segmented bar using Success, Warning, and Danger colors. Unfilled segments should be `rgba(255, 255, 255, 0.1)`.