---
name: M2M Design System
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
  secondary-fixed: '#dbe1ff'
  secondary-fixed-dim: '#bac5f0'
  on-secondary-fixed: '#0d1a3c'
  on-secondary-fixed-variant: '#3a456a'
  tertiary-fixed: '#6ffbbe'
  tertiary-fixed-dim: '#4edea3'
  on-tertiary-fixed: '#002113'
  on-tertiary-fixed-variant: '#005236'
  background: '#111319'
  on-background: '#e2e2ea'
  surface-variant: '#33343b'
typography:
  headline-4xl:
    fontFamily: Inter
    fontSize: 1.75rem
    fontWeight: '700'
    lineHeight: '1.2'
  headline-3xl:
    fontFamily: Inter
    fontSize: 1.4rem
    fontWeight: '600'
    lineHeight: '1.3'
  headline-2xl:
    fontFamily: Inter
    fontSize: 1.2rem
    fontWeight: '600'
    lineHeight: '1.4'
  body-xl:
    fontFamily: Inter
    fontSize: 1.05rem
    fontWeight: '400'
    lineHeight: '1.6'
  body-lg:
    fontFamily: Inter
    fontSize: 0.9375rem
    fontWeight: '400'
    lineHeight: '1.6'
  body-md:
    fontFamily: Inter
    fontSize: 0.875rem
    fontWeight: '400'
    lineHeight: '1.5'
  body-base:
    fontFamily: Inter
    fontSize: 0.8125rem
    fontWeight: '400'
    lineHeight: '1.5'
  label-sm:
    fontFamily: Public Sans
    fontSize: 0.75rem
    fontWeight: '500'
    lineHeight: 1rem
  label-xs:
    fontFamily: Public Sans
    fontSize: 0.68rem
    fontWeight: '500'
    lineHeight: 0.875rem
  mono-code:
    fontFamily: JetBrains Mono
    fontSize: 0.8125rem
    fontWeight: '400'
    lineHeight: '1.5'
    letterSpacing: -0.01em
  mono-label:
    fontFamily: Public Sans
    fontSize: 0.75rem
    fontWeight: '500'
    lineHeight: 1rem
    letterSpacing: 0.02em
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  2xl: 32px
  3xl: 48px
  4xl: 64px
  container-max: 1000px
  gutter: 16px
---

## Brand & Style

The design system is engineered for a privacy-first, end-to-end encrypted messaging platform. The aesthetic is a sophisticated blend of **macOS-inspired Glassmorphism** and high-end **Minimalism**, prioritizing user trust through visual precision and premium material effects. 

The interface communicates "security through clarity," utilizing deep tonal depths and subtle atmospheric glows to evoke a sense of a digital vault. Interaction design focuses on responsiveness and fluid transitions, ensuring the heavy security layer feels lightweight and effortless. The target audience is privacy-conscious professionals and tech-savvy individuals who value both cryptographic integrity and uncompromising UI elegance.

## Colors

The palette is anchored by a deep navy background that serves as a canvas for translucent indigo accents. 

- **Primary & Secondary:** Indigo (#6366f1) serves as the functional brand color for actions and active states. The lighter indigo (#c7d2fe) is reserved for high-contrast highlights or secondary accents within complex states.
- **Glass Surfaces:** Backgrounds should rarely be opaque. The `surface` and `elevated` tokens utilize semi-transparent alpha channels to allow content to bleed through, creating a sense of physical layering.
- **Semantic Colors:** Success (Green #10b981), Danger (Red), and Warning (Amber) follow industry standards but are softened to sit harmoniously within the dark environment, often accompanied by a subtle 12% opacity glow of the same hue.

## Typography

This design system utilizes a dual-font approach to balance human-centric messaging with clean, geometric utility.

- **Inter:** The primary workhorse for all UI text, headings, and chat bubbles. It is chosen for its exceptional legibility and neutral, modern character. Use Semibold (600) for section headers and Medium (500) for interactive labels.
- **Public Sans:** Utilized for metadata, labels, session IDs, and administrative details. Its neutral, strong geometric foundations reinforce the "systemic" and "secure" brand pillars without the harshness of a traditional monospace in non-code contexts.
- **JetBrains Mono:** Reserved strictly for code snippets or encryption keys where character differentiation is critical.
- **Type Scale:** The scale is intentionally tight and slightly smaller than standard web scales to mimic the "pro app" feel of macOS utilities. Line heights are generous (1.5x+) to maintain readability against dark, translucent backgrounds.

## Layout & Spacing

The layout is built on a strict **4px base grid**, ensuring mathematical harmony across all components.

- **Hub Layout:** Most top-level views (Hub, Settings, Vault) are contained within a **fixed-width floating card** (max 1000px) centered both horizontally and vertically. This creates a "desktop widget" aesthetic even on larger displays.
- **Chat Layout:** Within the chat view, a flexible sidebar-main layout is used. The sidebar maintains a fixed width (approx 320px) while the chat thread expands to fill the remaining space.
- **Spacing Rhythm:** Use `16px (lg)` for standard internal padding within cards and `24px (xl)` for margins between major layout blocks. Smaller units like `4px` and `8px` are reserved for internal component details (e.g., icon-to-text spacing).

## Elevation & Depth

Depth in this design system is achieved through light and translucency rather than heavy shadows.

- **Glassmorphism:** All primary surfaces use a `24px` backdrop blur with `180%` saturation to maintain color vibrancy of the elements underneath. Use a `48px` blur for high-priority overlays like modals or dropdowns.
- **Edge Lighting:** Cards feature a subtle `1px` inner border at the top (`rgba(255, 255, 255, 0.12)`) to simulate a physical edge light reflecting from above.
- **Accent Glows:** Interactive elements like the "Vault Lock" or active status dots utilize a soft Gaussian blur shadow of their own color (Indigo or Green) at `12%` opacity to simulate an LED emission.
- **Shadows:** Use large, diffused shadows with low opacity for depth. Standard cards use `0 2px 12px rgba(0, 0, 0, 0.2)`, while modals use a much heavier `0 25px 80px rgba(0, 0, 0, 0.7)` to isolate them from the UI background.

## Shapes

The shape language is rounded and friendly yet structured. 

- **Small (4px):** Used for tooltips and internal micro-elements.
- **Medium (8px):** The standard for buttons, inputs, and list items.
- **Large (16px):** Used for chat bubbles and smaller cards.
- **XL (24px):** Reserved for the primary "Hub" container and major modal windows.
- **Pill (Full):** Used for status indicators, tags, and the primary "Send" button.

## Components

- **Buttons:** Primary buttons use an indigo gradient with white text. Secondary buttons use the `elevated` surface with a subtle `1px` border. Hover states should increase the backdrop brightness slightly.
- **Chat Bubbles:** Sent messages use a linear indigo gradient (bottom-left to top-right). Received messages use the `elevated` glass surface. Both feature a `16px` radius, with the tail-side corner slightly sharper (4px) to indicate direction.
- **Input Fields:** Search and message inputs use `rgba(255, 255, 255, 0.05)` background with a `1px` border that glows indigo when focused.
- **Cards:** Must include the "edge light" top border and backdrop blur. For the "Vault" screen, cards should feature a central focal point with a pulse animation.
- **Toggles:** Use macOS-style pill toggles. When 'on', the background is Indigo; when 'off', it is a dark muted grey.
- **Progress Bars:** For file transfers, use a thin `4px` bar with an indigo glow on the leading edge of the progress indicator.
- **Badges:** Use **Public Sans** for badge text to emphasize the clean, geometric nature of system status indicators (e.g., "AES-256", "O3").