---
name: M2M Secure
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
  secondary: '#bac5f0'
  on-secondary: '#242f52'
  secondary-container: '#3a456a'
  on-secondary-container: '#a9b4de'
  tertiary: '#c3c0ff'
  on-tertiary: '#1d00a5'
  tertiary-container: '#8582ff'
  on-tertiary-container: '#180092'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e1e0ff'
  primary-fixed-dim: '#c0c1ff'
  on-primary-fixed: '#07006c'
  on-primary-fixed-variant: '#2f2ebe'
  secondary-fixed: '#dbe1ff'
  secondary-fixed-dim: '#bac5f0'
  on-secondary-fixed: '#0d1a3c'
  on-secondary-fixed-variant: '#3a456a'
  tertiary-fixed: '#e2dfff'
  tertiary-fixed-dim: '#c3c0ff'
  on-tertiary-fixed: '#0f0069'
  on-tertiary-fixed-variant: '#3323cc'
  background: '#111319'
  on-background: '#e2e2ea'
  surface-variant: '#33343b'
typography:
  headline-lg:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: '700'
    lineHeight: 40px
    letterSpacing: -0.02em
  headline-lg-mobile:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '700'
    lineHeight: 32px
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Inter
    fontSize: 20px
    fontWeight: '600'
    lineHeight: 28px
  body-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: 24px
  body-sm:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 20px
  crypto-code:
    fontFamily: Geist
    fontSize: 14px
    fontWeight: '500'
    lineHeight: 20px
    letterSpacing: 0.02em
  label-caps:
    fontFamily: Geist
    fontSize: 12px
    fontWeight: '600'
    lineHeight: 16px
    letterSpacing: 0.05em
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  base: 4px
  container-padding-desktop: 32px
  container-padding-mobile: 16px
  gutter: 16px
  element-gap: 12px
---

## Brand & Style
The design system is engineered for high-stakes privacy and premium utility. It targets a security-conscious audience that values technical transparency and aesthetic sophistication. 

The style utilizes **Glassmorphism** as its core visual metaphor, representing the concept of "digital clarity"—transparency that remains fortified. The UI features deep, multi-layered backgrounds with frosted glass surfaces, subtle indigo glows, and high-precision typography to evoke an emotional response of absolute safety, modern innovation, and exclusivity.

## Colors
The palette is rooted in deep space darkness to emphasize security and minimize eye strain during long sessions. 

- **Primary & Tertiary:** Indigo shades used for actionable elements and brand presence.
- **Accents:** Light indigo is reserved for technical readouts and high-contrast labels.
- **Background:** A foundational dark blue-black, intended to be layered with radial indigo gradients (opacity 10-15%) in the corners to create depth.
- **Surface:** Surfaces are non-opaque. Use backdrop-filter (blur: 12px to 20px) to maintain legibility over the dark background.

## Typography
This design system employs a dual-font strategy. **Inter** provides high legibility for conversational UI and primary navigation, while **Geist** (Monospace) is utilized for cryptographic keys, technical specs, and metadata to reinforce the system's "secure tech" narrative. 

Headlines should use tighter letter spacing to maintain a compact, premium feel. Small labels and technical readouts should use uppercase with increased tracking for a modern, architectural look.

## Layout & Spacing
The layout follows a fluid-to-fixed transition. For desktop, content is contained within a 1200px max-width 12-column grid. For mobile, a single-column layout with 16px side margins is standard.

Spacing follows a 4px scale. Components within glass containers should use generous internal padding (typically 24px) to prevent the "squished" look common in low-contrast glass designs. Use "safe areas" around cryptographic strings to ensure they are visually distinct from the message flow.

## Elevation & Depth
Depth is not achieved through traditional shadows, but through **cumulative opacity and blur**.

- **Level 1 (Base):** Deep black background with indigo radial blurs.
- **Level 2 (Panels):** Glass surfaces with 5% white fill, 16px backdrop-blur, and 1px border (15% white).
- **Level 3 (Modals/Popovers):** Glass surfaces with 10% white fill, 32px backdrop-blur, and a subtle outer glow using `rgba(99, 102, 241, 0.2)`.
- **Level 4 (Actions):** Active buttons utilize a vibrant gradient and a 20% indigo shadow glow to appear "projected" from the surface.

## Shapes
The shape language is sophisticated and modern. 
- **Standard Containers:** 12px to 24px radius depending on the container size.
- **Interactive Elements:** Buttons are set at 18px radius to feel approachable yet precise.
- **Technical Indicators:** Pill-shaped (fully rounded) geometry for badges and status indicators to contrast against the more geometric container shapes.

## Components
- **Buttons:** Primary buttons use a linear gradient (`#6366f1` to `#4f46e5`). Text is white with a subtle drop shadow. Secondary buttons are glass-based with 1px borders.
- **Glass Cards:** Always include a 1px top-left highlight border (white at 20%) and a bottom-right lowlight (white at 5%) to simulate physical thickness.
- **Input Fields:** Semi-transparent backgrounds (10% black) with a 1px border that glows indigo on focus. Use Monospace font for "Secret Key" inputs.
- **Pill Badges:** Small, fully rounded chips with high-contrast text (`#c7d2fe`) and a dark indigo background (20% opacity).
- **Message Bubbles:**
    - *Sent:* Solid indigo gradient with white text.
    - *Received:* Glass surface with light indigo text.
- **Security Indicator:** A constant, subtle pulse effect on "End-to-End Encrypted" labels using the primary indigo glow.