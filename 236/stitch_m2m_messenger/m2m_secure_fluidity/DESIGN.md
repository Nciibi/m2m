---
name: M2M Secure Fluidity
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
  secondary-fixed: '#6ffbbe'
  secondary-fixed-dim: '#4edea3'
  on-secondary-fixed: '#002113'
  on-secondary-fixed-variant: '#005236'
  tertiary-fixed: '#ffdadb'
  tertiary-fixed-dim: '#ffb2b7'
  on-tertiary-fixed: '#40000d'
  on-tertiary-fixed-variant: '#92002a'
  background: '#111319'
  on-background: '#e2e2ea'
  surface-variant: '#33343b'
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
  headline-lg-mobile:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '600'
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
  mono-code:
    fontFamily: Geist
    fontSize: 13px
    fontWeight: '500'
    lineHeight: 20px
    letterSpacing: 0.02em
  label-caps:
    fontFamily: Inter
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
  gutter: 24px
  margin-mobile: 16px
  margin-desktop: 40px
  container-max: 1440px
---

## Brand & Style
The design system is engineered for a high-security, Machine-to-Machine (M2M) ecosystem where technical precision meets premium aesthetics. The brand personality is authoritative yet ethereal, evoking the feeling of a sophisticated digital vault. 

The visual style is a refined **Glassmorphism**, emphasizing depth through light refraction rather than physical weight. Every interface element should feel like a suspended pane of polished synthetic glass. The emotional response is one of total control and absolute security, targeting CTOs and security architects who require data density without cognitive overload.

## Colors
The palette is centered on a deep, obsidian-like base (#030408) to maximize contrast for luminous accents. 

- **Primary (Indigo):** Used for active states, primary actions, and secure connection indicators.
- **Secondary (Emerald):** Reserved strictly for "Secure," "Verified," and "Active" statuses.
- **Tertiary (Rose):** Reserved for "Breach," "Alert," and "Unauthorized" events.
- **Neutrals:** A range of low-opacity whites (5% to 60%) are used for glass surfaces and secondary text, ensuring the background depth is never fully obscured.
- **Gradients:** Use subtle Indigo-tinted radial glows in the background to prevent "flatness" and provide a sense of atmospheric depth.

## Typography
This design system utilizes **Inter** for all functional and display text to maintain a professional, neutral tone that excels in high-density data environments. 

To handle cryptographic keys, device IDs, and sensor logs, **Geist** (Monospace) is employed. This creates a clear visual distinction between human-readable UI and machine-generated data. 

**Formatting Rules:**
- Titles should use tighter letter spacing to feel "locked-in" and sturdy.
- Use `label-caps` for table headers and secondary navigation items.
- Maintain high contrast (White or Off-White) for headlines, and reduced opacity (60-70%) for body text to sustain hierarchy within glass layers.

## Layout & Spacing
The layout philosophy follows a **Fluid Grid** model with generous margins to allow the glass backgrounds to "breathe."

- **Desktop:** 12-column grid with 24px gutters. Content is centered in a 1440px max-width container.
- **Tablet:** 8-column grid with 20px gutters. 
- **Mobile:** 4-column grid with 16px gutters and margins.

Spacing increments are strictly 4px-based. Use 24px and 32px for internal padding of containers to ensure the glass edges do not feel crowded by the content they hold.

## Elevation & Depth
Depth is created through the stacking of translucent layers rather than shadows. 

1.  **Base Layer:** The Deep Blue-Black (#030408) with subtle radial gradients.
2.  **Surface Layer:** Background Blur (40px / `blur-2xl`) with a 10% white fill. Used for primary dashboard cards.
3.  **Elevation Layer:** Background Blur (24px / `blur-xl`) with a 15% white fill. Used for floating modals, tooltips, and dropdowns.

**Borders:** Every glass surface must have a 1px solid border at 10% white opacity. This "specular edge" defines the boundaries of the glass in a dark environment where shadows are less effective.

## Shapes
The shape language combines geometric discipline with soft, modern curves. 

- **Primary Containers:** 24px corner radius. This creates a "soft tablet" feel for the main dashboard modules.
- **Interactive Elements:** Buttons and input fields use an 18px corner radius, distinguishing them from the structural containers.
- **Small Elements:** Tooltips and tags use an 8px radius.

The consistency of these radii is critical to maintaining the premium, high-fidelity character of the design system.

## Components
### Buttons
- **Primary:** Solid Indigo (#6366f1) fill with white text. 18px radius.
- **Glass/Ghost:** 10% white fill, 40px backdrop blur, 1px white/10 border. 
- **Hover States:** Increase fill opacity by 10% or add a subtle Indigo outer glow (bloom).

### Input Fields
- **Background:** 5% white fill with 1px white/10 border.
- **Active State:** Border transitions to Indigo (#6366f1) with a soft 4px glow.
- **Typography:** Use `mono-code` for fields specifically handling technical strings.

### Cards & Modules
- Use the 24px radius with `blur-2xl`. 
- Content within cards should have a 24px inner padding.

### Status Chips
- Small, 8px rounded corners.
- Use low-opacity background tints of Emerald (Success) or Rose (Error) with high-saturation text of the same color for legibility.

### Cryptographic Details
- Dedicated "Code Block" components using Geist Mono, a slightly darker 20% black background, and a subtle inner shadow to look "recessed" into the glass surface.