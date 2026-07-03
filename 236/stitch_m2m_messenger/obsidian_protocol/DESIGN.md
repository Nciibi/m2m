---
name: Obsidian Protocol
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
  secondary: '#4de082'
  on-secondary: '#003919'
  secondary-container: '#00b55d'
  on-secondary-container: '#003e1c'
  tertiary: '#ffb2b7'
  on-tertiary: '#67001b'
  tertiary-container: '#ff516a'
  on-tertiary-container: '#5b0017'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#e1e0ff'
  primary-fixed-dim: '#c0c1ff'
  on-primary-fixed: '#07006c'
  on-primary-fixed-variant: '#2f2ebe'
  secondary-fixed: '#6dfe9c'
  secondary-fixed-dim: '#4de082'
  on-secondary-fixed: '#00210c'
  on-secondary-fixed-variant: '#005227'
  tertiary-fixed: '#ffdadb'
  tertiary-fixed-dim: '#ffb2b7'
  on-tertiary-fixed: '#40000d'
  on-tertiary-fixed-variant: '#92002a'
  background: '#111319'
  on-background: '#e2e2ea'
  surface-variant: '#33343b'
typography:
  headline-lg:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: '700'
    lineHeight: '1.2'
    letterSpacing: -0.02em
  headline-md:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '600'
    lineHeight: '1.3'
    letterSpacing: -0.01em
  body-lg:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: '1.6'
    letterSpacing: '0'
  body-sm:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '400'
    lineHeight: '1.5'
    letterSpacing: '0'
  mono-label:
    fontFamily: JetBrains Mono
    fontSize: 13px
    fontWeight: '500'
    lineHeight: '1'
    letterSpacing: 0.05em
  mono-data:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '400'
    lineHeight: '1.4'
    letterSpacing: '0'
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  unit: 4px
  container-padding: 24px
  gutter: 16px
  stack-sm: 8px
  stack-md: 16px
  stack-lg: 32px
---

## Brand & Style

The design system is engineered for a machine-to-machine (M2M) and high-security communication environment. It targets security engineers, infrastructure architects, and privacy-conscious professionals who require a UI that reflects technical precision and absolute data integrity.

The visual style is **Premium Glassmorphism**. It utilizes a deep, layered architecture where information floats on translucent panes over a dark, infinite void. The aesthetic is "Cyber-Noir Professional"—eschewing flashy neon for a more disciplined, high-fidelity technical interface. Every element should feel like a piece of high-end hardware software: heavy, secure, and light-refractive.

## Colors

This design system is natively dark. The foundation is an absolute deep-space blue-black (#030408) which provides the high-contrast base necessary for glass effects to thrive.

- **Primary (Indigo):** Used for cryptographic signatures, active connection states, and primary actions. It represents the "pulse" of the secure channel.
- **Secondary (Emerald):** Reserved exclusively for "Verified," "Secure," and "Encrypted" status indicators.
- **Tertiary (Rose):** Used for security breaches, disconnected nodes, and critical system overrides.
- **Neutral/Surface:** A scale of semi-transparent whites (1% to 12% opacity) used to define the glass layers.

## Typography

The typography system strikes a balance between executive authority and technical utility. 

**Inter** is the workhorse, used for all interface elements to ensure maximum legibility and a modern, professional tone. Headlines use tighter letter spacing and bold weights to ground the ethereal glass surfaces.

**JetBrains Mono** is utilized for technical metadata: IP addresses, public keys, timestamps, and log data. This creates a clear visual distinction between human-readable messages and machine-generated data. All mono text should be rendered in uppercase when used as a label to enhance the "instrument panel" feel.

## Layout & Spacing

The layout follows a strict **4px baseline grid** to maintain technical precision. 

- **Desktop:** Utilizes a sidebar-centric fixed layout. The main viewport is divided into functional panes (e.g., Node List, Chat, Metadata Inspector) with a 1px gap or 16px gutter.
- **Glass Panes:** Content is grouped into translucent containers. Margin should be consistent within containers (usually 20px or 24px) to ensure text does not crowd the glass edges.
- **Responsive:** On mobile, the multi-pane layout collapses into a stack. Glass transparency is reduced by 5% on mobile to maintain legibility under varying outdoor lighting conditions.

## Elevation & Depth

Depth in this design system is achieved through physical optical properties rather than traditional drop shadows.

1.  **Backdrop Blur:** Every surface must use a `backdrop-filter: blur(40px)`. This creates the "2xl" frosted glass effect that separates foreground content from the background noise.
2.  **Surface Tiers:**
    *   **Level 0 (Background):** #030408.
    *   **Level 1 (Panels):** White at 3% opacity. 1px border (White/10%).
    *   **Level 2 (Modals/Popovers):** White at 8% opacity. 1px border (White/20%).
3.  **Borders as Light:** Instead of shadows, use "Inner Glow" or subtle top-weighted borders. A 1px border with a linear gradient (White/15% to White/5%) simulates a light source hitting the edge of the glass.

## Shapes

The shape language is "Calculated Softness." Elements use a consistent 0.5rem (8px) corner radius to feel approachable yet structural. 

- **Containers:** Standard panels use `rounded-lg` (16px) to create a soft, premium frame for the content.
- **Interactive Elements:** Buttons and inputs use `rounded-md` (8px). 
- **Status Pills:** Status indicators and tags use a fully rounded (pill) shape to distinguish them from actionable buttons.

## Components

### Buttons
- **Primary:** Solid Indigo (#6366f1) with white text. No transparency.
- **Glass Action:** White/10% background, backdrop-blur, 1px border (White/20%). On hover, increase background to White/20%.

### Input Fields
- Background: Black/20% (sunken look).
- Border: 1px White/10% default, 1px Indigo/50% on focus.
- Text: Inter, 14px.

### Cards & Panes
- Always utilize backdrop blur.
- Content should have a 24px internal padding.
- Headers within cards should have a subtle bottom border (1px White/5%).

### Status Indicators
- **Secure Node:** Emerald dot with a soft outer glow.
- **Encryption Key:** Displayed in JetBrains Mono inside a pill-shaped glass container.

### Messaging Bubbles
- **Sent:** Indigo background at 80% opacity to maintain some glass texture.
- **Received:** White/10% background with heavy backdrop blur.
- **Metadata:** Timestamps and "Read" receipts always in JetBrains Mono at 10px.