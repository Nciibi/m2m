# M2M — UI/UX Design Bible

**Version**: 1.0
**Target**: 10/10 UI · S+ UX · Production-ready · Pixel-perfect · Zero ambiguity
**Last updated**: 2026-07-02

> This document is the single source of truth for the entire M2M interface.
> Another engineering team could build the application from this document alone
> without ever asking a design question.

---

## Table of Contents

1. [Design Language & Tokens](#1-design-language--tokens)
2. [Component Library](#2-component-library)
3. [Screen Specifications](#3-screen-specifications)
4. [User Flows](#4-user-flows)
5. [Animation & Motion](#5-animation--motion)
6. [Accessibility](#6-accessibility)
7. [Responsive Behavior](#7-responsive-behavior)
8. [Security & Privacy Indicators](#8-security--privacy-indicators)
9. [Edge Cases & Anti-Patterns](#9-edge-cases--anti-patterns)

---

## 1. Design Language & Tokens

### 1.1 Design Philosophy

M2M is a secure P2P messenger for high-risk users. The design language communicates:

- **Trust** — Through glass-morphism surfaces, subtle glow effects, and cryptographic visual indicators
- **Security** — Every encryption indicator is visible and unambiguous
- **Simplicity** — One primary action per screen, minimal cognitive load
- **Privacy** — No data leaves the device; the UI reflects this through local-first patterns

The visual language is **dark-first** with an optional light mode. Dark is the default because:
- Reduces eye strain during extended use
- Makes accent glow effects more pronounced
- Communicates "serious tool" rather than "social app"
- Better battery life on OLED displays

### 1.2 Color System

#### 1.2.1 Core Palette

```css
/* Dark Mode (Default — applied to :root) */

/* Canvas & Surface */
--color-bg-dark: #030408;         /* Deepest background — body, app shell backdrop */
--color-bg-surface: rgba(15, 16, 25, 0.75);  /* App shell glass surface */
--color-bg-card: rgba(25, 26, 40, 0.55);     /* Card backgrounds */
--color-bg-elevated: rgba(30, 32, 48, 0.65); /* Elevated panels (modals, dropdowns) */
--color-bg-input: rgba(255, 255, 255, 0.04); /* Input field background */
--color-bg-input-focus: rgba(255, 255, 255, 0.08); /* Input field when focused */
--color-bg-hover: rgba(255, 255, 255, 0.06); /* Hover state overlay */
--color-bg-active: rgba(255, 255, 255, 0.1); /* Active/pressed state overlay */
--color-bg-overlay: rgba(0, 0, 0, 0.65);     /* Modal backdrops */
--color-bg-tooltip: rgba(20, 22, 35, 0.97);  /* Tooltip background */
--color-bg-modal-backdrop: rgba(0, 0, 0, 0.6); /* Modal overlay behind backdrop */

/* Canvas Gradient */
--canvas-gradient:
  radial-gradient(ellipse at 10% -10%, rgba(99, 102, 241, 0.18), transparent 45%),
  radial-gradient(ellipse at 90% 110%, rgba(16, 185, 129, 0.08), transparent 45%),
  radial-gradient(ellipse at 50% 50%, rgba(139, 92, 246, 0.04), transparent 70%);

/* Borders */
--color-border-default: rgba(255, 255, 255, 0.09); /* Default border — 3:1 contrast */
--color-border-active: rgba(129, 140, 248, 0.6);   /* Active/focused border */
--color-border-strong: rgba(255, 255, 255, 0.12);  /* Stronger border for emphasis */
--color-border-accent: rgba(129, 140, 248, 0.2);   /* Accent-colored border */

/* Accent (Indigo) */
--color-accent: #6366f1;          /* Primary accent — buttons, links, active indicators */
--color-accent-bright: #c7d2fe;   /* Bright accent — highlights, glow effects */
--color-accent-dim: #4f46e5;      /* Dim accent — pressed states, dark variants */
--color-accent-glow: rgba(99, 102, 241, 0.3);       /* Subtle accent glow */
--color-accent-glow-strong: rgba(99, 102, 241, 0.5); /* Strong accent glow */
--color-accent-glow-subtle: rgba(99, 102, 241, 0.1); /* Very subtle accent glow */
--color-accent-gradient: linear-gradient(135deg, #6366f1, #4f46e5); /* Button gradient */
--color-accent-gradient-warm: linear-gradient(135deg, #818cf8, #6366f1);

/* Text */
--color-text-primary: #f8fafc;     /* Primary text — 16:1 contrast on dark */
--color-text-secondary: #cbd5e1;   /* Secondary text — 11:1 contrast */
--color-text-muted: #94a3b8;       /* Muted text — 7:1 contrast (WCAG AAA) */
--color-text-inverse: #0f172a;     /* Text on light backgrounds */
--color-text-accent: #a5b4fc;      /* Accent-colored text — links, emphasized values */
--color-text-placeholder: #475569; /* Placeholder text — 4.9:1 contrast (WCAG AA) */

/* Semantic */
--color-success: #10b981;          /* Success — online status, confirmed actions */
--color-success-bright: #a7f3d0;   /* Bright success — glow effects */
--color-success-glow: rgba(16, 185, 129, 0.25);
--color-success-bg: rgba(16, 185, 129, 0.1);

--color-danger: #ef4444;           /* Danger — disconnect, delete, errors */
--color-danger-bright: #fca5a5;
--color-danger-glow: rgba(239, 68, 68, 0.2);
--color-danger-bg: rgba(239, 68, 68, 0.1);

--color-warning: #f59e0b;          /* Warning — caution, timers, unverified peers */
--color-warning-bright: #fde68a;
--color-warning-glow: rgba(245, 158, 11, 0.2);
--color-warning-bg: rgba(245, 158, 11, 0.1);

--color-info: #6366f1;            /* Info — neutral notifications */
--color-info-glow: rgba(99, 102, 241, 0.2);
--color-info-bg: rgba(99, 102, 241, 0.1);
```

```css
/* Light Mode — applied via [data-theme="light"] */

/* Canvas & Surface */
--color-bg-dark: #f1f5f9;
--color-bg-surface: rgba(255, 255, 255, 0.82);
--color-bg-card: rgba(255, 255, 255, 0.7);
--color-bg-elevated: rgba(255, 255, 255, 0.85);
--color-bg-input: rgba(0, 0, 0, 0.04);
--color-bg-input-focus: rgba(0, 0, 0, 0.06);
--color-bg-hover: rgba(0, 0, 0, 0.04);
--color-bg-active: rgba(0, 0, 0, 0.07);
--color-bg-overlay: rgba(0, 0, 0, 0.3);
--color-bg-tooltip: rgba(255, 255, 255, 0.98);
--color-bg-modal-backdrop: rgba(0, 0, 0, 0.35);

/* Canvas Gradient (light) */
--canvas-gradient:
  radial-gradient(ellipse 60% 40% at 5% 0%, rgba(99, 102, 241, 0.06) 0%, transparent 55%),
  radial-gradient(ellipse 50% 35% at 95% 100%, rgba(16, 185, 129, 0.04) 0%, transparent 50%);

/* Borders */
--color-border-default: rgba(0, 0, 0, 0.12);
--color-border-active: rgba(99, 102, 241, 0.4);
--color-border-strong: rgba(0, 0, 0, 0.12);
--color-border-accent: rgba(99, 102, 241, 0.12);

/* Accent */
--color-accent: #4f46e5;
--color-accent-bright: #6366f1;
--color-accent-dim: #4338ca;

/* Text */
--color-text-primary: #0f172a;
--color-text-secondary: #475569;
--color-text-muted: #64748b;         /* 4.9:1 on white — WCAG AA */
--color-text-inverse: #f8fafc;
--color-text-accent: #4f46e5;
--color-text-placeholder: #64748b;   /* 4.9:1 on white — WCAG AA */
```

#### 1.2.2 Semantic Color Usage

| Token | Where to use | Don't use for |
|-------|-------------|---------------|
| `--color-accent` | Primary buttons, active tabs, link text, verified badges | Background fills (use glow variants) |
| `--color-success` | Online indicator, verified fingerprint, success toasts | Interactive elements (use accent) |
| `--color-danger` | Disconnect button, delete actions, error toasts, unverified state | Non-destructive interactive elements |
| `--color-warning` | Timer countdowns, caution banners, disconnect badge | Status indicators that aren't time-sensitive |

### 1.3 Glass Effects

```css
--glass-blur-sm: blur(20px);
--glass-blur: blur(40px);
--glass-blur-lg: blur(60px);
--glass-saturate: saturate(200%);
--edge-light: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.08), transparent);
```

The glass effect is the signature visual of M2M. Every surface panel uses:
- `background: var(--color-bg-surface)` with low opacity
- `backdrop-filter: var(--glass-blur) var(--glass-saturate)`
- A 1px top `--edge-light` highlight for depth
- Bottom accent glow (`--color-accent-glow-subtle`)

### 1.4 Typography Scale

```css
--font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Inter', 'Roboto',
  'Oxygen', 'Ubuntu', 'Cantarell', 'Fira Sans', 'Droid Sans',
  'Helvetica Neue', sans-serif;
--font-mono: 'JetBrains Mono', 'Cascadia Code', 'Fira Code', 'Consolas',
  'Monaco', 'Menlo', monospace;

--font-weight-normal: 400;
--font-weight-medium: 500;
--font-weight-semibold: 600;
--font-weight-bold: 700;

--line-height-tight: 1.25;
--line-height-normal: 1.5;
--line-height-relaxed: 1.7;

--letter-spacing-tight: -0.025em;
--letter-spacing-normal: 0;
--letter-spacing-wide: 0.025em;
--letter-spacing-uppercase: 0.08em;
```

| Token | Size | Weight | Line Height | Where |
|-------|------|--------|-------------|-------|
| `--text-xs` | 0.65rem (10.4px) | 400–600 | 1.25 | Timestamps, badges, secondary metadata |
| `--text-sm` | 0.72rem (11.5px) | 400–500 | 1.5 | Descriptions, hints, secondary labels |
| `--text-base` | 0.78rem (12.5px) | 400 | 1.5 | Default body text, input values |
| `--text-md` | 0.85rem (13.6px) | 500 | 1.5 | Message text, list item titles |
| `--text-lg` | 0.95rem (15.2px) | 600 | 1.5 | Section headers, conversation names |
| `--text-xl` | 1.1rem (17.6px) | 600 | 1.25 | Screen titles, modal headers |
| `--text-2xl` | 1.3rem (20.8px) | 700 | 1.25 | Hero text, primary action labels |
| `--text-3xl` | 1.5rem (24px) | 700 | 1.25 | Welcome screen headers |
| `--text-4xl` | 2rem (32px) | 700 | 1.25 | Display text (rare) |

### 1.5 Spacing Scale

Base unit: 4px

| Token | Value | Typical use |
|-------|-------|-------------|
| `--space-xxs` | 4px | Icon gaps, reaction badges, inline spacing |
| `--space-xs` | 8px | Tight element spacing, avatar margins |
| `--space-sm` | 12px | Input padding, button padding |
| `--space-md` | 16px | Card padding, message bubble padding |
| `--space-lg` | 20px | Section gaps, form field spacing |
| `--space-xl` | 24px | Panel padding, list item padding |
| `--space-2xl` | 32px | Major section separation |
| `--space-3xl` | 48px | Screen edge margins |
| `--space-4xl` | 64px | Hero section spacing |

### 1.6 Border Radius

| Token | Value | Where |
|-------|-------|-------|
| `--radius-xs` | 4px | Small UI elements, inline code, badges |
| `--radius-sm` | 8px | Input fields, small cards |
| `--radius-md` | 12px | Cards, dropdowns, tooltips |
| `--radius-lg` | 18px | Message bubbles, buttons, modals |
| `--radius-xl` | 24px | Vault icon, setup icon, large panels |
| `--radius-2xl` | 32px | App shell container |
| `--radius-full` | 9999px | Avatars, reaction pills, dot indicators |

### 1.7 Shadow System

```css
/* Ambient shadows — depth */
--shadow-sm: 0 2px 4px rgba(0, 0, 0, 0.2);
--shadow-md: 0 8px 16px rgba(0, 0, 0, 0.3);
--shadow-lg: 0 16px 40px rgba(0, 0, 0, 0.4);
--shadow-xl: 0 24px 80px rgba(0, 0, 0, 0.6);

/* Accent glow shadows */
--shadow-accent: 0 4px 12px rgba(99, 102, 241, 0.2);
--shadow-accent-strong: 0 8px 24px rgba(99, 102, 241, 0.4);
--shadow-accent-glow: 0 0 30px rgba(99, 102, 241, 0.15);
--shadow-inner: inset 0 2px 4px rgba(0, 0, 0, 0.1);

/* Contextual */
--shadow-card: 0 4px 20px rgba(0, 0, 0, 0.25);
--shadow-card-hover: 0 12px 40px rgba(0, 0, 0, 0.4);
--shadow-bubble-sent: 0 4px 15px rgba(99, 102, 241, 0.3);
--shadow-bubble-received: 0 4px 15px rgba(0, 0, 0, 0.2);
--shadow-modal: 0 24px 80px rgba(0, 0, 0, 0.6);
--shadow-toast: 0 8px 32px rgba(0, 0, 0, 0.5);

/* App shell — layered shadow */
--shadow-app-shell:
  0 0 0 1px rgba(255, 255, 255, 0.02) inset,
  0 10px 40px -10px rgba(0, 0, 0, 0.7),
  0 0 100px -20px rgba(99, 102, 241, 0.15);
```

### 1.8 Z-Index Scale

```css
--z-base: 1;
--z-dropdown: 100;
--z-modal: 9999;
--z-toast: 10000;
```

### 1.9 Transitions & Easing

```css
--ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);    /* Primary — natural deceleration */
--ease-out-back: cubic-bezier(0.34, 1.56, 0.64, 1); /* Spring — celebratory moments */
--ease-in-out: cubic-bezier(0.4, 0, 0.2, 1);         /* Default — subtle transitions */

--transition-fast: 150ms var(--ease-out-expo);       /* Hover, small state changes */
--transition-base: 300ms var(--ease-out-expo);        /* Standard transitions */
--transition-smooth: 500ms var(--ease-out-expo);      /* Panel slides, page transitions */
--transition-slow: 800ms var(--ease-out-expo);        /* Entrance animations */
--transition-spring: 500ms var(--ease-out-back);      /* Celebration effects */
```

---

## 2. Component Library

### 2.1 Button

**Purpose**: Primary call-to-action element. Available in multiple visual weights.

**Variants**:

| Variant | Background | Text | Border | Use case |
|---------|-----------|------|--------|----------|
| `default` | `--color-accent-gradient` | White | None | Primary actions (Send, Connect, Unlock) |
| `secondary` | `--color-bg-elevated` | `--color-text-primary` | `--color-border-default` | Secondary actions (Cancel, Back) |
| `danger` | `--color-danger` | White | None | Destructive actions (Disconnect, Delete) |
| `ghost` | Transparent | `--color-text-secondary` | None | Minimal actions (text-like buttons) |
| `icon` | Transparent | `--color-text-secondary` | `--color-border-default` | Icon-only buttons |

**States**:

```
Default:    [─── Accent Gradient ───] [─── Secondary ───] [─── Danger ───]
Hover:      translateY(-2px) + shadow-accent-strong
Focus:      outline: 3px solid var(--color-accent-glow)
Active:     translateY(0) + scale(0.98)
Disabled:   opacity: 0.5, cursor: not-allowed, no shadow
Loading:    Show spinner ring, hide text
```

**Specs**:
- Height: 42px (default), 32px (sm), 26px (xs)
- Horizontal padding: `--space-lg` (default), `--space-md` (sm/xs)
- Border-radius: `--radius-lg`
- Font: `--text-md`, `--font-weight-semibold`
- Icon in button: 18px, `--space-xs` gap from text
- Icon-only buttons: 42×42px, no text

**Animations**:
- Hover: `150ms var(--ease-out-expo)` translateY + shadow
- Click: `100ms var(--ease-out-expo)` scale
- Shine sweep: Moving gradient overlay on default variant (first 800ms after mount)

**Accessibility**:
- `aria-label` required on icon-only buttons
- Focus ring visible on keyboard navigation only (use `:focus-visible`)
- `role="button"` on non-button elements

**Anti-patterns**:
- Never use `default` variant for destructive actions
- Never stack two `default` buttons — one CTA per section
- Never disable a button without a tooltip explaining why

### 2.2 Input

**Purpose**: Text input field with optional icon, clear button, and validation state.

**Variants**:
- `default` — Standard text input
- `mono` — Monospace font (for keys, fingerprints, addresses)
- `compact` — Smaller padding (for inline use)

**States**:

```
Default:    [─── bg-input ─── border-default ─── placeholder ───]
Focus:      [─── bg-input-focus ─── border-active ─── accent-glow ───]
Error:      [─── bg-danger-bg ─── border-danger ─── error message below ───]
Disabled:   [─── opacity 0.5 ─── cursor not-allowed ───]
With icon:  [🔍 ─── ─── ─── ─── ─── ─── value ───]
Clearable:  [─── value ─── ✕ ───]  (✕ shows only when value non-empty)
```

**Specs**:
- Height: 44px (default), 36px (compact)
- Border-radius: `--radius-md`
- Padding: `--space-sm` `--space-md`
- Font: `--text-base`, `--font-sans`
- Placeholder: `--color-text-placeholder`
- Icon position: Left, 16px from edge, 18px icon
- Clear button: Right, 44px from right edge (for Eye toggle clearance)
- Error message: 11px below input, `--text-xs`, `--color-danger`

**Interactions**:
- Focus: Border color transition `150ms var(--ease-out-expo)`
- Clear on Escape: Only when input is focused and has value
- Paste: Native support; passphrase inputs get a "📋 Paste" button overlay

**Accessibility**:
- `aria-label` or `aria-labelledby` on every input
- `aria-describedby` linking to error message when in error state
- `aria-invalid="true"` when in error state

### 2.3 Card

**Purpose**: Glass surface container for grouping related content.

**Variants**:
- `default` — Static information card
- `clickable` — Hover elevation + cursor pointer

**Specs**:
```
┌──────────────────────────────────────────────┐
│  🔗  Header Title                    [action] │  ← padding: --space-lg
│                                               │
│  Description text explaining the card's       │
│  purpose and content.                         │
│                                               │
│  [─── primary content area ───]               │
│                                               │
│  [Button]  [Button]                           │  ← padding: --space-md bottom
└──────────────────────────────────────────────┘
```

- Background: `--color-bg-card` with `backdrop-filter: var(--glass-blur-sm)`
- Border: 1px `--color-border-default`
- Border-radius: `--radius-lg`
- Padding: `--space-lg`
- Header gap: `--space-sm` between icon and title
- Title: `--text-lg`, `--font-weight-semibold`
- Description: `--text-sm`, `--color-text-secondary`
- Card shadow: `--shadow-card`

**States**:
- Hover (clickable): `translateY(-4px)`, `--shadow-card-hover`
- Active (clickable): `translateY(-2px)`

**Sizes**:
- Full-width within container (max-width controlled by parent)
- No min-height — grows with content

### 2.4 Modal

**Purpose**: Focused dialog for critical actions, verification, and extended input.

**Specs**:
```
┌────────────────────────────────────────┐
│  [─ backdrop: bg-modal-backdrop ─]     │
│                                        │
│    ┌─── modal surface ─────────────┐   │
│    │  Title                    [✕] │   │  ← padding: --space-xl
│    │                               │   │
│    │  Body content (scrolls if     │   │
│    │  exceeds max-height)          │   │
│    │                               │   │
│    │  [─── Footer ───]             │   │  ← padding: --space-lg
│    │  [Cancel]  [Confirm]          │   │
│    └───────────────────────────────┘   │
└────────────────────────────────────────┘
```

- Width: 480px (default), 90vw (max on small screens)
- Max-height: 80vh
- Border-radius: `--radius-xl`
- Background: `--color-bg-elevated`
- Backdrop: `--color-bg-modal-backdrop`, click to close
- Shadow: `--shadow-modal`
- Animation: `modalFadeIn` 300ms (backdrop) + `modalZoomIn` 300ms (content)

**States**:
- Open: Fade in backdrop + scale up content (0.95 → 1.0)
- Close: Fade out + scale down (1.0 → 0.95)

**Focus management**:
- Traps focus within modal while open
- First focusable element receives focus on open
- Escape key closes modal
- Returns focus to trigger element on close

**Accessibility**:
- `role="dialog"`
- `aria-modal="true"`
- `aria-labelledby` pointing to title
- `aria-describedby` pointing to body

### 2.5 Badge

**Purpose**: Small status indicator — online status, connection state, counts.

**Variants**: `default` | `success` | `danger` | `warning` | `info`

```
[online]  ← success variant with dot
[offline] ← default variant
[3]       ← info variant, count badge
```

**States**:
- `dot: true` — Adds a 6px colored dot before text (for online/offline states)
- `compact: true` — Reduced padding for inline use

**Specs**:
- Height: 22px (default), 18px (compact)
- Padding: `--space-xxs` `--space-sm`
- Border-radius: `--radius-full`
- Font: `--text-xs`, `--font-weight-semibold`
- Dot size: 6px, gap `--space-xxs` from text

**Color mapping**:
```
default → bg: bg-input, text: text-secondary
success → bg: success-bg, text: success, dot: success
danger  → bg: danger-bg, text: danger, dot: danger
warning → bg: warning-bg, text: warning, dot: warning
info    → bg: info-bg, text: accent, dot: accent
```

### 2.6 Toast

**Purpose**: Transient notification for action feedback (success, error, info, warning).

**Specs**:
```
┌─────────────────────────────────────┐
│ ✅  ✓ Vault unlocked      [─╳]     │  ← auto-dismiss countdown bar
└─────────────────────────────────────┘
```

- Position: Bottom-right, 16px from edges
- Width: 360px max
- Height: auto (min 44px)
- Border-radius: `--radius-md`
- Background: `--color-bg-elevated`
- Border-left: 3px solid semantic color
- Shadow: `--shadow-toast`
- Animation: Slide in from right (200ms), progress bar shrinks over duration
- Auto-dismiss: Default 5s (error: 8s)

**Types**:
| Type | Border | Icon | Duration |
|------|--------|------|----------|
| `success` | `--color-success` | ✅ | 4s |
| `error` | `--color-danger` | ❌ | 8s |
| `info` | `--color-accent` | ℹ️ | 5s |
| `warning` | `--color-warning` | ⚠️ | 6s |

**Accessibility**: `role="alert"`, `aria-live="assertive"`

### 2.7 LoadingSpinner

**Purpose**: Indicates loading/processing state.

**Variants**:
- `inline` — 18px ring for buttons, inline use
- `fullscreen` — Centered with optional label

**Specs**:
```
      ╭───╮
    ╱  ╱ ╲  ╲    ← rotating ring
    │  │   │  │
    ╲  ╲ ╱  ╱
      ╰───╯
```

- Size: 18px (inline), 36px (fullscreen)
- Ring: 2px stroke, `currentColor` with 0.3 opacity on trailing arc
- Animation: `spin` 0.6s linear infinite
- Label: `--text-sm`, `--color-text-muted`, `--space-md` below spinner

### 2.8 ProgressBar

**Purpose**: Shows completion progress for file transfers and processes.

**Variants**: `default` | `success` | `danger` | `warning`
**Sizes**: `default` (8px) | `small` (4px)

```
┌──────────────────────────────────────┐
│ ████████████████░░░░░░░░░░  65%      │
└──────────────────────────────────────┘
```

- Border-radius: `--radius-full`
- Track: `--color-bg-input`
- Fill: `--color-accent` (default) or semantic color
- Animation: width transition `300ms var(--ease-out-expo)`
- Optional label + percentage display below bar

### 2.9 Select

**Purpose**: Styled dropdown selector.

**Specs**:
- Height: 44px (default), 32px (compact)
- Border-radius: `--radius-md`
- Background: `--color-bg-input`
- Font: `--text-base`
- Chevron: Down arrow 16px, `--color-text-muted`
- Focus: Same as Input

### 2.10 Conversation Item

**Purpose**: Single row in the conversation list. Shows avatar, name, preview, status.

```
┌────────────────────────────────────────────┐
│  ┌────┐                                   │
│  │ AB │  Alice                   2m ago    │  ← clickable
│  └────┘  Hey, are you there?               │     border-radius: --radius-lg
│           ●                                │     padding: --space-md --space-lg
└────────────────────────────────────────────┘
```

**Layout**:
- Avatar: 48×48px, `--radius-lg`, dynamic gradient from `hashToColor()`
- Name: `--text-md`, `--font-weight-semibold`
- Time: `--text-xs`, `--color-text-muted`, right-aligned
- Preview: `--text-sm`, `--color-text-secondary`, single-line truncated
- Online dot: 8px green dot, top-right of avatar

**States**:
```
Default: bg: rgba(255,255,255,0.02)
Hover:   bg: rgba(255,255,255,0.05), translateY(-2px), shadow-md + accent-glow
Active:  translateY(-1px)
Selected: bg: accent-glow-subtle, border-color: border-accent
```

**Actions** (hover-reveal):
- ⭐ Favorite / ★ Unfavorite
- 📁 Archive / 📂 Unarchive
- 🔇 Mute / 🔔 Unmute
- 🗑️ Delete

### 2.11 Message Bubble

**Purpose**: Individual message display — sent or received.

**Sent** (right-aligned):
```
                                    ┌──────────────────────┐
                                    │  Hello! How are you?  │
                                    │           12:30 PM  ✓ │
                                    └──────────────────────┘
```

**Received** (left-aligned):
```
┌──────────────────────┐
│  I'm doing great!    │
│  Hey there!           │  ← sender label (group only)
│           12:31 PM    │
└──────────────────────┘
```

**Specs**:
- Max-width: 75% of container
- Padding: `--space-sm` `--space-md`
- Border-radius: `--radius-lg`
- Bottom corner: 4px (opposite direction of alignment)
- Gap between consecutive: `--space-xxs` (4px)
- Gap between groups: `--space-sm` (12px)

**Sent bubble**:
- Background: `--color-accent-gradient`
- Text: White
- Shadow: `--shadow-bubble-sent`
- Footer: White at 0.7 opacity

**Received bubble**:
- Background: `--color-bg-elevated`
- Text: `--color-text-primary`
- Shadow: `--shadow-bubble-received`
- Footer: `--color-text-secondary` at 0.5 opacity

**Footer row** (always present):
- Time: `--text-xs`, right-aligned
- Status icon (sent): ✓ (sent), ✓✓ (delivered), ⏳ (sending)
- Edited badge: "edited" in `--text-xs`, italic, muted
- Self-destruct timer: 🔥 M:SS in `--text-xs`, warning color
- Read receipt: ✓✓ in small text

**Reactions** (below bubble):
```
  [👍 2]  [❤️ 1]  [😂 3]    ← pill buttons
```
- `--radius-full`, `--text-xs`, `--space-xxs` gap
- Self-reaction: accent border + tinted background
- Hover: bring up emoji picker

**Context menu** (right-click):
```
  ┌──────────┐
  │ Edit     │
  │ Delete   │  ← danger color
  └──────────┘
```
- Position: Below bubble, right-aligned for sent, left-aligned for received
- Animation: Fade in 100ms

**Animations**:
- Sent: `msgSlide` 400ms — rises 8px + fades in
- Received: `msgReceived` 500ms — rises 10px + accent glow flash
- Stagger: `animation-delay: i * 0.05s` for consecutive messages

### 2.12 Emoji Picker

**Purpose**: Insert emoji into message text.

**Specs**:
```
┌──────────────────────────────────────┐
│ [😀][😁][😂][🤣][😊][😉][😍][🥰]  │
│ [😘][😜][😎][🤩][👍][👎][✌️][🤞] │
│ [👊][💪][🙌][👏][🤝][🔥][⭐][💯] │
│ [❤️][🧡][💛][💚][💙][💜][🖤][🤍] │
│ [💔][💖][✨][🎉][🙏][💀][☠️][👋] │
│ [🫂][🤗][😤][😭][😱][🤔][🙄][😴] │
│ [✅][❌][❗][❓][➕][➖][🚀][🎂]  │
│ [🎁][💰][🔒][🔓]                    │
└──────────────────────────────────────┘
```

- 8-column grid
- Gap: 2px
- Button size: 32×32px
- Hover: `scale(1.3)` + tinted background
- Position: Above the emoji button in the input toolbar
- Close: Click outside or select emoji

### 2.13 Typing Indicator

**Purpose**: Shows when a peer is actively typing.

```
● ● ●  Peer is typing…
```

- 3 dots, 5px each, `--color-accent-bright`
- Animation: `dotBounce` 1.4s with 0.2s stagger delay
- Font: `--text-xs`, italic, `--color-text-muted`
- Position: Between message area and input, 8px padding
- Auto-hide: After 3s of no typing packets received

### 2.14 Drop Zone

**Purpose**: Drag-and-drop file attachment overlay.

```
┌─── ─── ─── ─── ─── ─── ─── ───┐
│                                 │
│      Drop files here to send    │
│                                 │
│  [─── dashed accent border ───] │
└─────────────────────────────────┘
```

- Dashed border: 2px `--color-accent`
- Background: `--color-accent-glow-subtle`
- Text: `--text-lg`, `--color-accent`
- Visible only during drag-over
- Z-index: 50 (above input area, below modals)

### 2.15 Update Banner

**Purpose**: Non-blocking notification when a new version is available.

```
┌──────────────────────────────────────────────────────┐
│ 📦  Update available: v1.2.3  [✓ Update Now]  [✕]  │
└──────────────────────────────────────────────────────┘
```

- Position: Fixed bottom-right, 16px from edges
- Background: `--color-bg-elevated`
- Border: 1px `--color-border-accent`
- Border-radius: `--radius-lg`
- Shadow: `--shadow-lg`
- Animation: Slide up 200ms
- Dismiss: X button or update installed

---

## 3. Screen Specifications

### 3.1 SetupView (Loading Splash)

**Purpose**: Shown during initial key generation and first-run onboarding.

**User goal**: Wait for identity generation, or learn about M2M.

**Layout**:
```
┌────────────────────────────────────────┐
│                                        │
│                                        │
│             ┌──────────┐               │
│             │   🔑/🚀  │               │  ← 80×80px, glass icon container
│             │   glow   │               │     with sonar ring animation
│             └──────────┘               │
│                                        │
│      Initializing Secure Enclave       │  ← --text-2xl, centered
│                                        │
│   Generating Ed25519 identity keys.    │  ← --text-md, --color-text-secondary
│   They never leave your device.        │     6px line-height
│                                        │
│           ●  ●  ●                      │  ← loading dots
│                                        │
│   ┌──────────────────────────────┐    │
│   │ Ed25519 · X25519 · XChaCha  │    │  ← crypto badge
│   └──────────────────────────────┘    │
│                                        │
└────────────────────────────────────────┘
```

**States**:
| State | Visual | Duration |
|-------|--------|----------|
| Loading keys | Sonar ring animating, dots bouncing | 2-3s |
| First run (step 1/4) | Welcome message, "Get Started" button | User-paced |
| First run (step 2/4) | Identity explanation, "Next" button | User-paced |
| First run (step 3/4) | Encryption info, "Next" button | User-paced |
| First run (step 4/4) | Ready message, "Start Messaging" button | User-paced |
| Existing user | Skip wizard, show loading, navigate to vault | 2-3s |

**Onboarding wizard steps**:

| Step | Title | Icon | Description |
|------|-------|------|-------------|
| 1 | Welcome to M2M | 🚀 | "A private, end-to-end encrypted messenger. No servers, no accounts, no tracking." |
| 2 | Your Identity is Local | 🔑 | "Your keys are generated on this device and never leave it." |
| 3 | End-to-End Encrypted | 🔒 | "Messages use X3DH + Double Ratchet (Signal protocol)." |
| 4 | Ready to Go! | ✅ | "Share your invite link with a trusted peer to start chatting." |

**Step indicator**:
```
  ●  ○  ○  ○    ← active: accent fill, done: success fill, next: outlined
```
- 24×24px dots, `--space-sm` gap
- Done dots show checkmark

**Responsive**: Centered, max-width 480px, no layout change.

### 3.2 VaultView (Passphrase Entry)

**Purpose**: Create or unlock the local encrypted vault.

**User goals**: Set a passphrase on first use; unlock the vault on subsequent launches.

**Layout**:
```
┌────────────────────────────────────────┐
│                                        │
│                                        │
│             ┌──────────┐               │
│             │  🔒/🔓   │               │  ← 80×80px, glow breathe animation
│             │   glow   │               │     bounce on unlock
│             └──────────┘               │
│                                        │
│     Set Up Your Vault / Unlock Vault   │  ← --text-xl
│                                        │
│   Choose a strong passphrase to...     │  ← --text-sm, secondary
│   Min 12 chars · Argon2id              │
│                                        │
│   ┌──────────────────────────────┐     │
│   │  Passphrase            👁 📋 │     │  ← Input, mono font
│   └──────────────────────────────┘     │     Eye toggle, Paste button
│   ████████░░░░░░░░░░░░░░ 32 bits      │  ← strength bar (hidden until typing)
│                                        │
│   ┌──────────────────────────────┐     │  ← only on first time
│   │  Confirm passphrase          │     │
│   └──────────────────────────────┘     │
│   ✓ Passphrases match                 │  ← shown when match
│                                        │
│   What makes a strong passphrase?      │  ← toggle tips
│                                        │
│   ┌──────────────────────────────┐     │
│   │       Create Vault / Unlock │     │  ← accent button, full width
│   └──────────────────────────────┘     │
│                                        │
│   This vault belongs to a1b2c3d4...   │  ← fingerprint hint (returning users)
│                                        │
└────────────────────────────────────────┘
```

**States**:
| State | Visual | Behavior |
|-------|--------|----------|
| Idle | Lock icon, pulse animation | Waiting for input |
| Typing | Strength bar updates in real-time | Entropy calculated every keystroke |
| Match (first time) | Green checkmark + "Passphrases match" | Below confirm field |
| Mismatch (first time) | Red error "Passphrases do not match" | Below confirm field |
| Too short (<12 chars) | Strength bar red, "Too short (min 12)" | Submit disabled |
| Weak (<40 bits) | Strength bar red/orange, "Weak — ~32 bits" | Submit shows error |
| Fair (40-60 bits) | Strength bar yellow, "Fair — ~52 bits" | Submit allowed |
| Strong (60-80 bits) | Strength bar green, "Strong — ~72 bits" | Submit allowed |
| Very strong (>80 bits) | Strength bar cyan, "Very Strong — ~96 bits" | Submit allowed |
| Loading | Lock → Unlock animation, button shows spinner | Wait for vault operation |
| Error | Shake animation on form, red error text below | Auto-clears on next keystroke |
| Tips open | Expanded tips box with diceware advice | Toggle |

**Strength bar colors**:
```css
weak: #ef4444 (danger)
fair: #f59e0b (warning)
strong: #10b981 (success)
very-strong: #22d3ee (cyan)
```

**Responsive**: Max-width 380px for input, centered.

### 3.3 HubView (Main Screen)

**Purpose**: Central navigation hub — connect to peers, manage conversations, discover nearby users.

**User goals**: Start a new chat, browse existing conversations, manage contacts.

**Layout**:
```
┌──────────────────────────────────────────────┐
│  [M2M logo] M2M          [● Online] [⚙️]    │  ← header
├──────────────────────────────────────────────┤
│  [🔗 Connect] [💬 Chats 3] [📡 Nearby] [🏠 Family] │  ← tab bar
├──────────────────────────────────────────────┤
│                                              │
│  [─── Tab Content ───]                       │
│                                              │
│  (see sub-sections below)                    │
│                                              │
│                                              │
└──────────────────────────────────────────────┘
```

**Header**:
- Logo: 20×20px rounded image
- Title: "M2M" in `--text-lg`, `--font-weight-bold`
- Connection badge: `--text-sm`, shows current connection state
- Settings gear: Icon button, opens SettingsView
- Right-aligned, `--space-lg` horizontal padding

**Tab bar**:
- 4 tabs: Connect, Chats, Nearby, Family
- Height: 44px
- Tab padding: `--space-md` `--space-lg`
- Active indicator: bottom border 2px `--color-accent`
- Badge count: `--radius-full`, `--color-accent` background, white text, `--text-xs`
- Horizontal scroll on small screens

#### 3.3a Connect Tab

```
┌──────────────────────────────────────────────┐
│  ● Listening for incoming connections        │  ← only when listening
│                                              │
│  ┌─── Host a Connection ──────────────────┐  │
│  │  Generate a one-time signed invite...  │  │
│  │                                        │  │
│  │  [Generate Invite Link]                │  │  ← or invite output
│  │  ┌────────────────────────┐  [📋]      │  │
│  │  │ m2m://a1b2c3d4e5f6...  │  ✓        │  │
│  │  └────────────────────────┘           │  │
│  │  🔥 Expires in 59:32                  │  │  ← countdown timer
│  │                                        │  │
│  │  Recent Invites                        │  │  ← history, last 5
│  │  m2m://a1b2c3d4e5...        [📋]      │  │
│  │  m2m://f6e5d4c3b2a1...      [📋]      │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ┌─── Join a Connection ─────────────────┐   │
│  │  Paste an invite link from a peer...   │   │
│  │                                        │   │
│  │  [m2m://.................] [Connect]   │   │
│  │                                        │   │
│  │  ✓ Valid Invite Found                  │   │
│  │  Your Name  [________________]         │   │
│  │  Their Name [________________]         │   │
│  └────────────────────────────────────────┘  │
│                                              │
│  ──────────────────────────────────────       │
│                                              │
│  Your Identity Fingerprint                   │
│  a1b2:c3d4:e5f6:g7h8:i9j0:k1l2:m3n4:o5p6 [📋] │
└──────────────────────────────────────────────┘
```

**States**:
| State | Visual |
|-------|--------|
| Not listening | No green dot, Generate button available |
| Listening | Green pulsing dot + "Listening for incoming connections" |
| Invite generated | Invite output field + copy button + countdown |
| Countdown expired | Countdown shows 00:00, invite no longer valid |
| Valid invite pasted | Green checkmark + naming fields appear |
| Invalid invite | No checkmark, Connect button disabled |
| Connecting | Button shows spinner |
| Tor warning | Yellow warning box below invite |

#### 3.3b Chats Tab

```
┌──────────────────────────────────────────────┐
│  [🔍 Search conversations…]                  │  ← search bar
│                                              │
│  ⭐ Alice B.                   ★  📂  🔇 🗑│  ← favorite conversation
│     Hey, are you there?        2m ago        │
│  ●                                            │
│                                              │
│  ⭐ Charlie                     ★  📂  🔔   │
│     See you tomorrow!          1h ago         │
│  ●                                            │
│                                              │
│  Dave                          ☆  📂  🔔 🗑│  ← regular conversation
│     No messages yet.           3d ago         │
│  ○                                            │
│                                              │
│  ┌────────────────────────────────────┐       │
│  │  📁 Archived                      │       │  ← archived section
│  │  Eve (archived)           ☆ 📂    │       │
│  └────────────────────────────────────┘       │
│                                              │
│  ── or empty state ──                        │
│                                              │
│  💬                                          │
│  No conversations yet                        │
│  Generate an invite link to host a...        │
│  [Get Started]                               │
└──────────────────────────────────────────────┘
```

**Sorting order**:
1. Active, non-archived conversations first
2. Favorites (★) first within active
3. By `last_message_at` descending
4. Archived conversations last

**Search**: Filters by name, peer name, and message preview — case-insensitive, live as you type.

#### 3.3c Nearby Tab

```
┌──────────────────────────────────────────────┐
│  Discovery Not Active — OR — No Peers Found  │
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │ 📡  LAN Peer           2m ago           │  │
│  │     192.168.1.42:38553                 │  │
│  │     a1b2c3d4...           [Connect]    │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │ 🌐  DHT Peer           5m ago           │  │
│  │     203.0.113.42:38553                 │  │
│  │     f6e5d4c3...           [Connect]    │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

**States**:
| State | Visual |
|-------|--------|
| Discovery off | Explanation text + "Open Settings" button |
| No peers found | Explanation + "Refresh" button |
| Peers found | List of discovered peers with Connect button |

#### 3.3d Family Tab

See FamilyTab component — list of trusted family members with connect/remove actions.

### 3.4 ChatView (Messaging)

**Purpose**: Send and receive encrypted messages in a 1:1 or group conversation.

**User goals**: Communicate securely, send files, verify peer identity.

**Layout**:
```
┌──────────────────────────────────────────────┐
│  [🛡/✓] Encrypted Session  [← Hub] [●] [Disconnect] │
├──────────────────────────────────────────────┤
│  [── File request banners ──]               │
│                                              │
│  [── File transfer progress bars ──]         │
│                                              │
│  [── Ctrl+F search bar ──]                   │  ← toggled with Ctrl+F
│                                              │
│  [── Typing indicator ──]                    │  ← shown when peer types
│                                              │
│  ┌──────────────────────────────────────┐    │
│  │ 🔒 End-to-end encrypted session...   │    │  ← session banner
│  │ a1b2:c3d4:e5f6:...                   │    │
│  └──────────────────────────────────────┘    │
│                                              │
│  ┌── Retention Policy ───────────────────┐   │
│  │ [No Expiration ▼] [1 Hour ▼] [Export] │   │
│  └────────────────────────────────────────┘   │
│                                              │
│  ─── Today ───                               │  ← date separator
│                                              │
│              ┌──────────────────────┐        │
│              │  Hey, how are you?   │  ✓     │  ← sent message
│              │             12:30 PM │        │
│              └──────────────────────┘        │
│                                              │
│  ┌──────────────────────────────┐            │
│  │  I'm doing great! You?      │            │  ← received message
│  │                   12:31 PM   │            │
│  └──────────────────────────────┘            │
│                                              │
│     [👍 1]  [❤️ 1]                          │  ← reactions
│                                              │
│  ─── Yesterday ───                           │
│                                              │
│  [── older messages... ──]                  │
│                                              │
│  ┌──╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┐                    │
│  ╎  Drop files here      ╎                  │  ← drag zone (visible on drag)
│  └──╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┘                    │
│                                              │
│  [📎] [😊] [message text...] [⏱️ 30s ▼] [➤] │  ← input area
│                                              │
│  End-to-end encrypted     Ctrl+Enter to send │  ← footer
│                                              │
│  ┌──⬇──┐                                    │  ← scroll-to-bottom FAB
│  └─────┘                                    │     (shows when scrolled up)
└──────────────────────────────────────────────┘
```

**Key interactions**:
- `Ctrl+Enter` or click ➤ to send
- `Shift+Enter` newline
- `Esc` back to hub (when input empty)
- `Ctrl+F` toggle search bar
- `Ctrl+K` open settings
- Right-click message → context menu (Edit/Delete)
- Hover message → reaction picker
- Scroll up → load older messages (infinite scroll)
- Click shield icon → fingerprint verification modal
- Click peer name/icon → peer info modal

**Date separators**:
```
─── Today ───          ← if message date matches today
─── Yesterday ───      ← if message date matches yesterday
─── Monday, June 22 ─── ← any other date
```

**Empty state**:
```
  ✉️
  Start the conversation
  Send a message below to begin your encrypted conversation.
  All messages are protected with end-to-end encryption.
```

**Loading older messages**:
- "Loading older messages…" centered, italic, muted
- "Beginning of conversation" when all loaded

**File request banner**:
```
┌──────────────────────────────────────┐
│ 📄  report.pdf            [Accept] [Reject] │
│      2.4 MB                             │
└──────────────────────────────────────┘
```

**File transfer progress**:
```
┌──────────────────────────────────────┐
│ 📄  photo.jpg              4.2 MB    │
│ ████████████████░░░░░░  65%          │  ← ProgressBar
│ transferring      2.1 MB/s · 12s remaining │
└──────────────────────────────────────┘
```

**Input area**:
- Multi-line textarea (auto-grows to 120px max)
- Buttons: [Attach] [Emoji] [text input] [Timer ▼] [Send]
- Character count shown at >90% of 64KB limit

**States**:
| State | Visual | Behavior |
|-------|--------|----------|
| Connected | Input enabled, send button accent | Normal operation |
| Disconnected (verified) | "Reconnect" button, input disabled | Can attempt reconnect |
| Disconnected (unverified) | Auto-navigate to hub | No reconnect possible |
| Reconnecting | "Reconnecting (2/5)…" badge | Exponential backoff |
| Sending | Send button shows spinner | Input disabled |
| Peer typing | Typing indicator with animated dots | Auto-hides after 3s |

### 3.5 SettingsView

**Purpose**: Configure all app settings — network, security, appearance, identity.

**User goals**: Manage STUN, Tor, private mode, security features, theme.

**Layout**:
```
┌──────────────────────────────────────────────┐
│  [⚙️] Settings                  [← Hub]      │
├──────────────────────────────────────────────┤
│                                              │
│  ─── Identity ───                            │
│  ┌────────────────────────────────────────┐  │
│  │ Fingerprint            a1b2:c3d4... 📋 │  │
│  │ Public Key             0xabcd...       │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ─── Theme ───                                │
│  ┌────────────────────────────────────────┐  │
│  │ Appearance     [☀️] [🌙] [🖥️]         │  │
│  │                Current: dark           │  │
│  │ ─────────────────────────────────────  │  │
│  │ Accent Color   [■] #6366f1  [Reset]   │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ─── Network ───                              │
│  ┌────────────────────────────────────────┐  │
│  │ Public IP          203.0.113.42  📋    │  │
│  │                     [Discover via STUN]│  │
│  │ NAT Type           [RestrictedCone]    │  │
│  │ STUN Servers       3/4 reachable       │  │
│  │ ─────────────────────────────────────  │  │
│  │ Private Mode       [⬜] Hide IP from invites │
│  │ Tor                [⬜] Route via Tor  │  │
│  │                     [Test Tor]         │  │
│  │ ─────────────────────────────────────  │  │
│  │ Connectivity       [Check]            │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ─── Discovery ───                            │
│  ┌────────────────────────────────────────┐  │
│  │ 📡 LAN Discovery      [⬜]             │  │
│  │ 🌐 DHT Discovery      [⬜]             │  │
│  │ Discovered Peers      3 found [Refresh]│  │
│  │ ⚠️ Both OFF by default for privacy    │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ─── Security ───                             │
│  ┌────────────────────────────────────────┐  │
│  │ 👁 Screen Capture Protection  [⬜]     │  │
│  │ Clipboard Auto-Clear      [30s ▼]      │  │
│  │ Idle Vault Lock           [5m ▼]       │  │
│  │ ─────────────────────────────────────  │  │
│  │ 🔒 Vault         [Lock Now] [Clear Clipboard]│
│  └────────────────────────────────────────┘  │
│                                              │
│  ─── STUN Servers ───                         │
│  ┌────────────────────────────────────────┐  │
│  │ [OK] stun.l.google.com:19302    12ms ✕ │  │
│  │ [OK] stun1.l.google.com:19302   18ms ✕ │  │
│  │ [FAIL] stun.custom.com:3478           ✕ │  │
│  │                                        │  │
│  │ [host:port_________] [Add] [Reset]    │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ─── About ───                                │
│  ┌────────────────────────────────────────┐  │
│  │ Version              2.5.x             │  │
│  │ Crypto    Ed25519 · X25519 · XChaCha20 │  │
│  └────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

**States**:
| Element | State | Visual |
|---------|-------|--------|
| STUN discover | Loading | Button shows spinner |
| STUN discover | Complete | IP shown, diagnostics updated |
| Tor toggle | On | Checkbox filled, proxy active |
| Tor toggle | Testing | "Testing Tor…" toast |
| Screen capture | On/Off | Toggle slider |
| Copy fingerprint | Copied | Icon switches to checkmark for 2s |
| Reset accent | Clicked | Color resets to #6366f1 |

---

## 4. User Flows

### 4.1 First Launch Flow

```
1. App starts → SetupView (loading splash)
   ↓
2. First run? → Yes → Onboarding wizard (4 steps)
   │                    ↓
   │               "Start Messaging" → sets first_run_complete
   │                    ↓
3. Check identity exists → No
   ↓
4. Navigate to VaultView (first time)
   ↓
5. User enters passphrase (12+ chars, 40+ bits entropy)
   ↓
6. User confirms passphrase
   ↓
7. "Create Vault" → generates Ed25519 keypair
   ↓
8. Vault created → navigate to HubView (Connect tab)
```

### 4.2 Returning User Flow

```
1. App starts → SetupView (loading splash, 2-3s)
   ↓
2. First run? → No (short circuit to loading)
   ↓
3. Check vault → initialized + locked
   ↓
4. Navigate to VaultView (unlock mode)
   ↓
5. User enters passphrase
   ↓
6. "Unlock" → decrypts identity key
   ↓
7. Vault unlocked → navigate to HubView (Chats tab)
```

### 4.3 Connect to Peer Flow

```
1. User on HubView → Connect tab
   ↓
2. Option A: User clicks "Generate Invite Link"
   │   ↓
   │  Invite appears: "m2m://a1b2c3..." 
   │  → Copy to clipboard
   │  → Share via out-of-band channel (Signal, email, etc.)
   │  → Countdown starts (60 min)
   ↓
3. Option B: User pastes invite from peer
   │   ↓
   │  "Valid Invite Found" appears
   │  → Optional: Set display names
   │  → Click "Connect"
   ↓
4. Connection attempt → "Connecting…" spinner
   ↓
5. Success → Navigate to ChatView
   ↓
6. Verify peer fingerprint via out-of-band channel
```

### 4.4 Messaging Flow

```
1. User in ChatView → text area focused
   ↓
2. Type message → typing indicator sent to peer
   ↓
3. Press Ctrl+Enter or click Send
   ↓
4. Message encrypts + sends → "sending" status (clock icon)
   ↓
5. Message delivered → "sent" status (✓)
   ↓
6. Peer receives → decrypts → displays
   ↓
7. Peer can react (hover → emoji picker)
   ↓
8. Peer can reply → flow repeats
```

### 4.5 File Transfer Flow

```
Sender:
1. Click Attach or drag file to drop zone
   ↓
2. File dialog opens → select file
   ↓
3. File transfer begins → progress bar appears
   ↓
4. Chunks sent with per-chunk hashes + ACKs
   ↓
5. Transfer complete → confirmation

Receiver:
1. File request banner appears
   ↓
2. Accept → save dialog opens
   ↓
3. File downloads with progress bar
   ↓
4. Transfer complete → file saved to chosen location
```

### 4.6 Group Chat Flow

```
1. User in HubView → Create group (via command or future UI)
   ↓
2. Group created → Sender Keys generated for each member
   ↓
3. Invites sent to initial members via 1:1 DR sessions
   ↓
4. Each member receives group invite → accepts
   ↓
5. Group appears in conversation list (future UI)
   ↓
6. Members send messages → encrypted with Sender Key
   ↓
7. Messages decrypted by all members via receiver chains
```

### 4.7 Security Verification Flow

```
1. User in ChatView → click shield icon
   ↓
2. Fingerprint modal opens
   ↓
3. Shows: Local fingerprint + Peer fingerprint side-by-side
   ↓
4. User compares fingerprints via out-of-band channel
   ↓
5. If match → click "Confirm Match & Verify"
   ↓
6. Peer marked as verified → shield turns green
   ↓
7. Future connections auto-show "Verified" badge
```

### 4.8 Reconnection Flow

```
1. Connection drops → header shows "disconnected"
   ↓
2. If peer was verified → "Reconnect" button appears
   ↓
3. User clicks Reconnect → exponential backoff (1s, 2s, 4s, ...30s max)
   ↓
4. Each attempt shows "Reconnecting (2/5)…"
   ↓
5a. Success → "established" status, missed messages synced
   ↓
5b. All 5 attempts fail → "Reconnection failed" error
   ↓
6. User must re-share invite link
```

### 4.9 Theme Change Flow

```
1. User opens Settings → Theme section
   ↓
2. Click ☀️ (light), 🌙 (dark), or 🖥️ (system)
   ↓
3. `data-theme` attribute updates immediately
   ↓
4. Preference persisted via `set_theme_preference`
   ↓
5. Optional: Click accent color picker → choose color
   ↓
6. `--color-accent` CSS variable updates instantly
   ↓
7. Preference persisted
```

---

## 5. Animation & Motion

### 5.1 Animation Registry

| Animation | Duration | Easing | Property | Trigger |
|-----------|----------|--------|----------|---------|
| `appEntrance` | 800ms | `ease-out-expo` | translateY + opacity | App mount |
| `msgSlide` | 400ms | `ease-out-expo` | translateY + opacity | Message sent |
| `msgReceived` | 500ms | `ease-out-expo` | translateY + opacity + box-shadow | Message received |
| `dotBounce` | 1.4s | `ease-in-out` | scale + opacity | Loading/typing dots |
| `modalFadeIn` | 300ms | `ease-out-expo` | opacity | Modal backdrop |
| `modalZoomIn` | 300ms | `ease-out-expo` | scale + opacity | Modal content |
| `shake` | 400ms | `ease-out-expo` | translateX | Error state |
| `pulseRing` | 3s | `ease-in-out` | scale + opacity | Listening indicator |
| `sonarRing` | 2.5s | `ease-out-expo` | scale + opacity | Setup icon glow |
| `spin` | 0.6s | `linear` | rotate | Loading spinner |
| `glowBreathe` | 3s | `ease-in-out` | box-shadow | Vault/security icons |
| `fadeIn` | 150ms | `ease-out-expo` | opacity | Tooltips, context menus |
| `unlockBounce` | 0.6s | `ease-out-back` | scale + rotate | Vault unlock |
| `slideInRight` | 500ms | `ease-out-expo` | translateX | View transitions |
| `fabAppear` | 300ms | `ease-out-expo` | scale + opacity | Scroll-to-bottom FAB |

### 5.2 Performance Rules

1. **Animate only `transform` and `opacity`** — never `width`, `height`, `top`, `left`, `margin`, `padding`
2. **Use `will-change: transform`** on elements that animate frequently (message bubbles, modals)
3. **Never use `will-change`** on hover targets (memory cost)
4. **Respect `prefers-reduced-motion`** — wrap all animations:
```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## 6. Accessibility

### 6.1 Contrast Requirements

| Token | Dark BG | Light BG | WCAG Level |
|-------|---------|----------|------------|
| `--color-text-primary` | #f8fafc on #030408 → 16:1 | #0f172a on #ffffff → 14:1 | **AAA** |
| `--color-text-secondary` | #cbd5e1 on #030408 → 11:1 | #475569 on #ffffff → 6.8:1 | **AAA** |
| `--color-text-muted` | #94a3b8 on #030408 → 7:1 | #64748b on #ffffff → 4.9:1 | **AAA / AA** |
| `--color-text-placeholder` | #475569 on #030408 → 4.9:1 | #64748b on #ffffff → 4.9:1 | **AA** |

### 6.2 Focus Management

- All interactive elements must have visible focus ring: `outline: 2px solid var(--color-accent); outline-offset: 2px`
- Use `:focus-visible` to show ring only during keyboard navigation (not mouse click)
- Modals must trap focus — Tab cycles through modal elements only
- First focusable element in modal receives focus on open
- Return focus to trigger element on modal close

### 6.3 ARIA Requirements

| Element | Attribute | Value |
|---------|-----------|-------|
| Icon-only buttons | `aria-label` | Descriptive text |
| Modal | `role` | `"dialog"` |
| Modal | `aria-modal` | `"true"` |
| Modal title | `aria-labelledby` | ID of title element |
| Modal body | `aria-describedby` | ID of body element |
| Tab list | `role` | `"tablist"` |
| Tab | `role` | `"tab"` |
| Active tab | `aria-selected` | `"true"` |
| Toast | `role` | `"alert"` |
| Toast container | `aria-live` | `"assertive"` |
| New messages | `aria-live` | `"polite"` |
| Input error | `aria-describedby` | ID of error message |
| Input error state | `aria-invalid` | `"true"` |
| Decorative images | `role` | `"presentation"` or `aria-hidden="true"` |
| Loading state | `role` | `"status"` |
| Conversation list | `role` | `"list"` |
| Conversation item | `role` | `"listitem"` |

### 6.4 Keyboard Navigation

| Key | Context | Action |
|-----|---------|--------|
| `Tab` | Global | Move to next focusable element |
| `Shift+Tab` | Global | Move to previous focusable element |
| `Enter` | Global | Activate focused element |
| `Space` | Global | Toggle focused checkbox/switch |
| `Esc` | ChatView | Back to Hub |
| `Esc` | Modal | Close modal |
| `Esc` | Search bar | Close search |
| `Ctrl+Enter` | ChatView input | Send message |
| `Shift+Enter` | ChatView input | New line |
| `Ctrl+F` | ChatView | Toggle search |
| `Ctrl+K` | Global | Open settings |
| `Ctrl+N` | HubView | Switch to Connect tab |
| `Ctrl+,` | Global | Open settings |
| `?` | Global | Toggle shortcut help |
| `ArrowUp` | Conversation list | Previous item |
| `ArrowDown` | Conversation list | Next item |

---

## 7. Responsive Behavior

### 7.1 Breakpoints

| Breakpoint | Target | Behavior |
|------------|--------|----------|
| > 1000px | Desktop | Full layout — app shell 1000px max-width, centered |
| 600-1000px | Tablet | Reduced padding (24px → 16px), sidebar collapses |
| < 600px | Mobile | Full-bleed container, bottom tab bar, reduced padding |

### 7.2 Desktop (> 1000px)

```
┌──────────────────────────────────────────────┐
│              App Shell (1000px)               │
│    ↑ 16px margin from window edges           │
│    94vh height, 800px max                    │
└──────────────────────────────────────────────┘
```

### 7.3 Tablet (600-1000px)

- `.app-shell`: `max-width: 100%`, `margin: 0`, `border-radius: 0`
- Pad edges at `--space-xl` instead of `--space-2xl`
- Reduced header padding

### 7.4 Mobile (< 600px)

- `body { padding: 0 }`, `#root { padding: 0 }`
- App shell: `100vh`, no border-radius
- Settings rows: Stack vertically
- Fingerprint grid: 2 columns instead of 4
- Conversation items: Reduced padding

---

## 8. Security & Privacy Indicators

### 8.1 Visual Security Language

| Element | Visual | Meaning |
|---------|--------|---------|
| Shield icon (gray) | 🛡️ `--color-warning` | Peer not verified |
| Shield icon (green) | ✅ `--color-success` | Peer verified |
| Lock icon | 🔒 `--color-accent-bright` | Session encrypted |
| Connection badge (green) | `●` `--color-success` | Connection established |
| Connection badge (red) | `●` `--color-danger` | Connection disconnected |
| Reconnecting badge | "Reconnecting (2/5)…" | Auto-reconnect in progress |
| Timer icon | 🔥 M:SS `--color-warning` | Message self-destruct active |
| Double checkmark | ✓✓ `--color-accent-bright` | Message read |
| Single checkmark | ✓ `--color-text-muted` | Message sent |
| Eye icon | 👁️ | Password visible |
| Eye-off icon | 👁️‍🗨️ | Password hidden |

### 8.2 Privacy-First Defaults

All discovery and tracking features are OFF by default:
- LAN discovery: OFF
- DHT discovery: OFF
- Screen capture protection: OFF
- Clipboard auto-clear: OFF (0 seconds)
- Idle vault lock: OFF (0 seconds)

The privacy notice appears in the Discovery settings panel:
```
⚠️ Both are OFF by default for privacy. When enabled,
your IP address is visible to observers on the discovery channel.
Ephemeral IDs are used (not your permanent identity key) and
rotate periodically.
```

### 8.3 Security Banners

**Unverified peer warning** (ChatView header):
```
[⚠️ Verify Peer] icon in warning color
Click to open fingerprint comparison modal
```

**Tor warning** (Connect tab, when Tor enabled + invite generated):
```
⚠️ Tor Inbound Warning
Tor is enabled for outbound connections, but this invite
contains your real IP address.
```

---

## 9. Edge Cases & Anti-Patterns

### 9.1 Empty States

| Screen | Empty State |
|--------|-------------|
| ChatView (no messages) | "Start the conversation" with send icon |
| HubView Chats (no conversations) | "No conversations yet" + "Get Started" button |
| HubView Chats (search no results) | "No conversations found" + "Try adjusting your search" |
| HubView Nearby (discovery off) | "Discovery Not Active" + "Open Settings" |
| HubView Nearby (no peers) | "No Peers Found" + explanation |
| HubView Family (no family) | Empty family list (handled by FamilyTab) |

### 9.2 Error States

| Scenario | Error Display |
|----------|--------------|
| STUN discovery fails | Toast: "STUN failed: [error]" |
| Connection attempt fails | Toast: "Connection failed: [error]" |
| Send message fails | Message stays in "sending" state, toast on failure |
| File transfer fails | Progress bar turns red, "Failed" label |
| Passphrase too weak | Inline error in VaultView with shake animation |
| Passphrase mismatch | Inline error in VaultView |
| Reconnect fails (all 5) | Toast: "Reconnection failed after max attempts" |
| Export conversation fails | Toast: "Export failed: [error]" |
| Vault lock fails | Toast: "Failed to lock vault: [error]" |
| Tor toggle fails | Toast: "Tor toggle failed: [error]" |

### 9.3 Loading States

| Scenario | Loading Indicator |
|----------|-------------------|
| App initialization | SetupView loading dots |
| STUN discovery | Button spinner |
| Connect to peer | Button spinner + "Connecting…" |
| Reconnecting | Badge with attempt counter |
| Sending message | Send button spinner |
| Loading older messages | "Loading older messages…" text |
| File transfer | ProgressBar with speed/ETA |
| Identity export | Button loading state |

### 9.4 Anti-Patterns (Never Do)

1. **Never auto-reconnect without user action** — The user must click "Reconnect"
2. **Never expose the private key in any UI** — Only fingerprints and public keys
3. **Never log message content** — Tracing is redacted: `tracing::warn!(error = %e)`
4. **Never show both primary buttons** — One CTA per card/section
5. **Never disable a button without explaining why** — Use tooltip or adjacent text
6. **Never animate layout properties** — Only `transform` and `opacity`
7. **Never store secrets in `localStorage`** — All persistent data goes through Tauri backend
8. **Never forget the `prefers-reduced-motion` query** — Always wrap animations
9. **Never leave icon-only buttons without `aria-label`**
10. **Never hardcode strings** — Use semantic CSS variables for all colors/spacing

---

*This document is the single source of truth for M2M UI/UX. Any implementation that deviates from this specification should be considered a bug.*
