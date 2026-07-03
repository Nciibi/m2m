---
name: M2M Security Core
colors:
  surface: '#051424'
  surface-dim: '#051424'
  surface-bright: '#2c3a4c'
  surface-container-lowest: '#010f1f'
  surface-container-low: '#0d1c2d'
  surface-container: '#122131'
  surface-container-high: '#1c2b3c'
  surface-container-highest: '#273647'
  on-surface: '#d4e4fa'
  on-surface-variant: '#c7c4d7'
  inverse-surface: '#d4e4fa'
  inverse-on-surface: '#233143'
  outline: '#908fa0'
  outline-variant: '#464554'
  surface-tint: '#c0c1ff'
  primary: '#c0c1ff'
  on-primary: '#1000a9'
  primary-container: '#8083ff'
  on-primary-container: '#0d0096'
  inverse-primary: '#494bd6'
  secondary: '#c5c6cf'
  on-secondary: '#2e3037'
  secondary-container: '#45464e'
  on-secondary-container: '#b4b4bd'
  tertiary: '#c5c6ce'
  on-tertiary: '#2e3037'
  tertiary-container: '#8f9098'
  on-tertiary-container: '#282930'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e1e0ff'
  primary-fixed-dim: '#c0c1ff'
  on-primary-fixed: '#07006c'
  on-primary-fixed-variant: '#2f2ebe'
  secondary-fixed: '#e2e2eb'
  secondary-fixed-dim: '#c5c6cf'
  on-secondary-fixed: '#191b22'
  on-secondary-fixed-variant: '#45464e'
  tertiary-fixed: '#e2e2eb'
  tertiary-fixed-dim: '#c5c6ce'
  on-tertiary-fixed: '#191b22'
  on-tertiary-fixed-variant: '#45464e'
  background: '#051424'
  on-background: '#d4e4fa'
  surface-variant: '#273647'
typography:
  headline-xl:
    fontFamily: Inter
    fontSize: 48px
    fontWeight: '700'
    lineHeight: 56px
    letterSpacing: -0.02em
  headline-lg:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: '600'
    lineHeight: 40px
    letterSpacing: -0.01em
  headline-md:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '600'
    lineHeight: 32px
  body-lg:
    fontFamily: Inter
    fontSize: 18px
    fontWeight: '400'
    lineHeight: 28px
  body-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: 24px
  label-mono:
    fontFamily: Geist
    fontSize: 14px
    fontWeight: '500'
    lineHeight: 20px
    letterSpacing: 0.05em
  label-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '600'
    lineHeight: 16px
  headline-lg-mobile:
    fontFamily: Inter
    fontSize: 28px
    fontWeight: '600'
    lineHeight: 36px
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  unit: 4px
  gutter: 24px
  margin-mobile: 16px
  margin-desktop: 40px
  container-max: 1440px
---

## Brand & Style

The design system is engineered for **M2M**, a secure messenger that prioritizes privacy, technical integrity, and sophisticated minimalism. The brand personality is authoritative yet invisible, evoking a sense of "digital fortress" through high-end materials and precise execution.

The visual style utilizes **Modern Glassmorphism** layered over a deep, monochromatic foundation. It relies on translucency to represent data flow and "spectral" security layers. The aesthetic is high-tech and premium, characterized by expansive dark surfaces, indigo accents that feel like "energy pulses," and a strict adherence to order and clarity.

**Key Principles:**
- **Encrypted Clarity:** Every piece of information must be legible and framed within secure containers.
- **Atmospheric Depth:** Use of blurs and subtle borders to create a multi-dimensional workspace.
- **Technical Precision:** Monospaced accents for metadata to signal accuracy and developer-grade security.

## Colors

The palette is rooted in deep space tones to ensure visual comfort and focus during long periods of use.

- **Primary (Indigo):** Used exclusively for high-priority actions, active states, and verification badges. It acts as the "signal" within the dark void.
- **Surface Foundations:** `#0c0e14` is the absolute base. `#111319` is used for elevated glass panels and navigation sidebars.
- **Overlays:** Use semi-transparent variants of the neutral palette for glass effects (e.g., `rgba(255, 255, 255, 0.03)`).
- **Semantic Colors:** Success (Emerald), Error (Rose), and Warning (Amber) should be used with low saturation to maintain the sophisticated atmosphere.

## Typography

This design system uses a dual-font approach to balance human connection with technical authority.

- **Inter (Sans-Serif):** The primary workhorse for all UI elements, headings, and chat bubbles. It provides a neutral, highly legible canvas.
- **Geist (Monospace):** Reserved for "technical signatures"—encryption keys, timestamps, file sizes, and status logs. It should be used sparingly to denote data that is "system-generated."

**Weight Usage:** Bold weights are used for structural headings; regular weights are preferred for body copy to maintain a light, airy feel against the dark background.

## Layout & Spacing

The layout philosophy follows a **Fluid Grid with Safe Zones**. Messaging interfaces require significant horizontal breathing room to prevent the UI from feeling claustrophobic.

- **Grid:** A 12-column system for desktop, collapsing to a single column for mobile.
- **Rhythm:** All spacing is based on a 4px baseline grid. 
- **Chat Layout:** Message bubbles should have a maximum width of 65% of the container to maintain readability.
- **Sidebars:** Fixed-width navigation (80px) and flexible-width contact lists (320px-400px) provide a stable anchor for the fluid chat window.

## Elevation & Depth

Elevation in this design system is not achieved through traditional drop shadows, but through **Tonal Stacking and Backdrop Blurs**.

1.  **Level 0 (Base):** `#0c0e14` - The canvas.
2.  **Level 1 (Panels):** `#111319` with a 1px solid border at 10% opacity white.
3.  **Level 2 (Glass Overlays):** Semi-transparent surfaces with `backdrop-filter: blur(20px)`.
4.  **Level 3 (Modals/Popovers):** Higher transparency with a subtle "inner glow" (top-edge 1px stroke) to simulate light hitting the edge of a glass pane.

**Borders:** Use thin, 1px borders instead of heavy shadows to define shapes. Borders should be slightly lighter than the surface they contain.

## Shapes

The shape language is extremely approachable to contrast the "cold" nature of security tech.

- **Containers:** Large surfaces like chat windows use `rounded-2xl` (1rem).
- **Interactive Elements:** Buttons and input fields use `rounded-full` (pill-shaped) to invite interaction.
- **Message Bubbles:** Use asymmetric rounding; the corner pointing toward the user's side of the screen is sharper, while the others are fully rounded.

## Components

### Buttons
- **Primary:** Solid Indigo background with white text. High roundedness.
- **Secondary:** Glass-style (transparent with 1px border and blur).
- **Ghost:** Monospace text with no background, used for utility actions.

### Chat Bubbles
- **Sent:** Solid Indigo or deep blue-grey with high-contrast text.
- **Received:** Glass-effect background (low opacity white) with backdrop blur.

### Inputs
- **Search & Message Bar:** Pill-shaped, subtle dark-grey background, monospace placeholder text. On focus, the border glows with a soft Indigo outer stroke.

### Lists
- Contact lists use "active" states indicated by a vertical Indigo line on the far left and a subtle background tint. No dividers; use whitespace and alignment to separate items.

### Chips & Badges
- **Security Status:** Monospace font, pill-shaped, using small icons (e.g., a shield) to denote "End-to-End Encrypted."