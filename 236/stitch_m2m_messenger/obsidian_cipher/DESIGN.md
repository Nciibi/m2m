---
name: Obsidian Cipher
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
  secondary: '#89ceff'
  on-secondary: '#00344d'
  secondary-container: '#00a2e6'
  on-secondary-container: '#00344e'
  tertiary: '#4edea3'
  on-tertiary: '#003824'
  tertiary-container: '#00885d'
  on-tertiary-container: '#000703'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e1e0ff'
  primary-fixed-dim: '#c0c1ff'
  on-primary-fixed: '#07006c'
  on-primary-fixed-variant: '#2f2ebe'
  secondary-fixed: '#c9e6ff'
  secondary-fixed-dim: '#89ceff'
  on-secondary-fixed: '#001e2f'
  on-secondary-fixed-variant: '#004c6e'
  tertiary-fixed: '#6ffbbe'
  tertiary-fixed-dim: '#4edea3'
  on-tertiary-fixed: '#002113'
  on-tertiary-fixed-variant: '#005236'
  background: '#111319'
  on-background: '#e2e2ea'
  surface-variant: '#33343b'
typography:
  headline-xl:
    fontFamily: Inter
    fontSize: 48px
    fontWeight: '800'
    lineHeight: 56px
    letterSpacing: -0.02em
  headline-lg:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: '700'
    lineHeight: 40px
    letterSpacing: -0.01em
  headline-lg-mobile:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '700'
    lineHeight: 32px
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
  label-code:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '500'
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
  xs: 8px
  sm: 16px
  md: 24px
  lg: 40px
  xl: 64px
  gutter: 20px
  margin: 24px
---

## Brand & Style

This design system is built for a highly secure, machine-to-machine and peer-to-peer messaging environment. The aesthetic is rooted in **Deep Glassmorphism**, prioritizing a sense of digital depth, high-tech transparency, and impenetrable security. 

The target audience consists of developers, security-conscious professionals, and automated systems that require a UI reflecting both cutting-edge cryptography and premium usability. The emotional response is one of "Safe Sophistication"—the UI should feel like a high-end command center: dark, focused, and immersive. 

Key visual drivers include:
- **Depth through Transparency:** Utilizing layered glass panels to separate information hierarchy.
- **Luminous Accents:** Using vibrant indigo glows to draw attention to critical actions and encryption statuses.
- **Tactile Digitalism:** Elements should feel like physical glass overlays on a dark void.

## Colors

The palette is anchored by a "Void Black" (#030408) base, providing the necessary contrast for glass effects to thrive. 

- **Primary (Indigo):** Used for primary actions, active states, and secure connection indicators.
- **Secondary (Cyan):** Reserved for secondary data visualizations and information-heavy badges.
- **Tertiary (Emerald):** Exclusively for "Verified" states, successful encryption handshakes, and system health.
- **Glass System:** Background surfaces use a highly desaturated white at 3% opacity to create the "frosted" look, while borders use 8% opacity to catch "light" at the edges of panels.

## Typography

This design system employs a dual-font strategy. **Inter** handles all primary interface elements, providing a clean, neutral, and highly legible experience across all scales. **JetBrains Mono** is introduced for labels, cryptographic hashes, and technical metadata to reinforce the machine-to-machine (M2M) narrative.

Headlines should be set with tight letter-spacing and heavy weights to create a "locked-in" feel. Body text maintains generous line-height to ensure readability against semi-transparent backgrounds.

## Layout & Spacing

The layout follows a **Fluid Glass Grid**. Containers do not sit on a flat plane but are perceived as floating. 

- **Grid:** Use a 12-column grid for desktop with 20px gutters. 
- **Padding:** Internal glass containers should never have less than 16px (`sm`) of padding to keep content away from the "frosted" edges.
- **Mobile:** On mobile, the margin is fixed at 24px. Multi-column cards collapse into a single vertical stack of glass tiles.
- **M2M Density:** For technical views, use a "Compact" mode where spacing scales down by 25% (e.g., 24px becomes 18px) to maximize data density.

## Elevation & Depth

Depth is not communicated via shadows, but through **Backdrop Refraction and Border Highlights**.

1.  **Level 0 (Base):** The solid #030408 background.
2.  **Level 1 (Surface):** Backdrop blur of 12px. Surface color at 3% white. 1px solid border at 8% white.
3.  **Level 2 (Floating/Modals):** Backdrop blur of 24px. Surface color at 6% white. 1px solid border at 12% white. Add a very subtle Indigo (#6366f1) outer glow (spread 20px, opacity 5%) to simulate light emission.

**Z-axis rule:** As an object "rises" toward the user, the blur intensity increases and the border becomes slightly more opaque.

## Shapes

The design system uses a consistent **12px (0.75rem)** corner radius for all primary containers and cards. 

- **Containers:** 12px.
- **Buttons:** 8px (to sit nested comfortably within containers).
- **Badges/Chips:** Full pill-shape (999px) to contrast against the structured rectangular grid.
- **Inputs:** 8px.

Avoid sharp corners entirely to maintain the "liquid glass" aesthetic.

## Components

### Buttons
- **Primary:** Solid Indigo gradient (from #6366f1 to #4f46e5). No transparency. High-contrast white text.
- **Glass Action:** Transparent background, 1px Indigo border, 12px backdrop blur. 

### Cryptographic Badges
Small, pill-shaped chips using `label-code` typography. They should have a subtle pulsing animation on the border when a live verification is occurring.

### Input Fields
Darker than the surface (rgba 0,0,0,0.3) with a 1px border that glows Indigo on focus. Placeholder text should be 40% opacity white.

### Glass Cards
The core unit of the UI. Must include:
1. `backdrop-filter: blur(16px);`
2. `background: rgba(255, 255, 255, 0.03);`
3. `border: 1px solid rgba(255, 255, 255, 0.08);`

### Animated Loading States
Use "Shimmer" effects that move across the glass surfaces rather than traditional spinners. The shimmer should look like a beam of light refracting through the panel.

### Lists
List items are separated by 1px semi-transparent lines (8% white). Hover states should increase the backdrop-blur intensity rather than changing the background color significantly.