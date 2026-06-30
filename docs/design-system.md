# M2M Design System Documentation

## Overview

The M2M design system is a token-based, glassmorphic design language built for zero-trust encrypted messaging. It prioritizes security, accessibility, and visual polish while maintaining a minimal dependency footprint.

**Philosophy:**
- Security-first: No external UI dependencies or CDNs
- Performance: Zero runtime CSS overhead, GPU-accelerated animations
- Accessibility: WCAG 2.1 AA compliant with keyboard navigation and screen reader support
- Consistency: Every visual property uses design tokens (no magic numbers)
- Polish: Premium glassmorphic aesthetic with thoughtful micro-interactions

---

## Design Tokens

All design tokens are defined in `src/styles/tokens.css` using CSS custom properties. Tokens are organized into semantic categories.

### Color System

#### Canvas & Depth
Background colors create depth through layering and transparency.

**Dark Theme (Default):**
```css
--color-bg-dark: #030408           /* Deepest canvas */
--color-bg-surface: rgba(15, 16, 25, 0.75)    /* App surface */
--color-bg-card: rgba(25, 26, 40, 0.55)       /* Card backgrounds */
--color-bg-elevated: rgba(30, 32, 48, 0.65)   /* Modals, dropdowns */
--color-bg-input: rgba(255, 255, 255, 0.04)   /* Input fields */
--color-bg-input-focus: rgba(255, 255, 255, 0.08)
--color-bg-hover: rgba(255, 255, 255, 0.06)   /* Hover states */
--color-bg-active: rgba(255, 255, 255, 0.1)   /* Active states */
--color-bg-overlay: rgba(0, 0, 0, 0.65)       /* Overlays/backdrops */
--color-bg-tooltip: rgba(20, 22, 35, 0.97)
--color-bg-modal-backdrop: rgba(0, 0, 0, 0.6)
```

**Light Theme:**
Applied via `[data-theme="light"]` selector with automatic `prefers-color-scheme` detection.

#### Borders
```css
--color-border-default: rgba(255, 255, 255, 0.06)  /* Standard borders */
--color-border-active: rgba(129, 140, 248, 0.6)    /* Focus/active state */
--color-border-strong: rgba(255, 255, 255, 0.12)   /* Emphasis borders */
--color-border-accent: rgba(129, 140, 248, 0.2)    /* Accent elements */
```

#### Accent Colors (Indigo)
Primary brand color for CTAs, links, and focus states.

```css
--color-accent: #6366f1              /* Primary accent */
--color-accent-bright: #c7d2fe       /* Light variant */
--color-accent-dim: #4f46e5          /* Dark variant */
--color-accent-glow: rgba(99, 102, 241, 0.3)
--color-accent-glow-strong: rgba(99, 102, 241, 0.5)
--color-accent-glow-subtle: rgba(99, 102, 241, 0.1)
--color-accent-gradient: linear-gradient(135deg, #6366f1, #4f46e5)
--color-accent-gradient-warm: linear-gradient(135deg, #818cf8, #6366f1)
```

**Usage:**
- Primary buttons, CTAs
- Focus indicators
- Active navigation states
- Links and interactive elements

#### Text Colors
```css
--color-text-primary: #f8fafc        /* Headings, primary content */
--color-text-secondary: #cbd5e1      /* Body text, descriptions */
--color-text-muted: #64748b          /* Subtle text, metadata */
--color-text-inverse: #0f172a        /* Text on light backgrounds */
--color-text-accent: #a5b4fc         /* Accent text color */
--color-text-placeholder: #475569    /* Input placeholders */
```

**Hierarchy:**
1. Primary: Headings, important labels
2. Secondary: Body text, descriptions
3. Muted: Metadata, timestamps, subtle labels
4. Accent: Links, interactive text

#### Semantic Colors
Used for status indicators, alerts, and feedback.

**Success (Green):**
```css
--color-success: #10b981
--color-success-bright: #a7f3d0
--color-success-glow: rgba(16, 185, 129, 0.25)
--color-success-bg: rgba(16, 185, 129, 0.1)
```
Usage: Successful operations, online status, verified states

**Danger (Red):**
```css
--color-danger: #ef4444
--color-danger-bright: #fca5a5
--color-danger-glow: rgba(239, 68, 68, 0.2)
--color-danger-bg: rgba(239, 68, 68, 0.1)
```
Usage: Errors, destructive actions, critical alerts

**Warning (Amber):**
```css
--color-warning: #f59e0b
--color-warning-bright: #fde68a
--color-warning-glow: rgba(245, 158, 11, 0.2)
--color-warning-bg: rgba(245, 158, 11, 0.1)
```
Usage: Warnings, caution states, important notices

**Info (Indigo):**
```css
--color-info: #6366f1
--color-info-glow: rgba(99, 102, 241, 0.2)
--color-info-bg: rgba(99, 102, 241, 0.1)
```
Usage: Informational messages, tips, neutral alerts

---

### Spacing Scale

Based on a 4px grid system for consistent rhythm and alignment.

```css
--space-xxs: 4px    /* Tight spacing, icon gaps */
--space-xs: 8px     /* Compact elements */
--space-sm: 12px    /* Related content */
--space-md: 16px    /* Default spacing */
--space-lg: 20px    /* Section spacing */
--space-xl: 24px    /* Large gaps */
--space-2xl: 32px   /* Major sections */
--space-3xl: 48px   /* Page-level spacing */
--space-4xl: 64px   /* Hero sections */
```

**Guidelines:**
- Use `--space-md` (16px) as default for most layouts
- Use `--space-xs` (8px) for icon-text gaps
- Use `--space-xl` and above for major sections
- Maintain 4px multiples for custom spacing

---

### Typography Scale

Modular scale optimized for desktop messaging interfaces.

```css
--text-xs: 0.65rem     /* 10.4px - Tiny labels */
--text-sm: 0.72rem     /* 11.5px - Metadata */
--text-base: 0.78rem   /* 12.5px - Body text */
--text-md: 0.85rem     /* 13.6px - Emphasized body */
--text-lg: 0.95rem     /* 15.2px - Subheadings */
--text-xl: 1.1rem      /* 17.6px - Section headings */
--text-2xl: 1.3rem     /* 20.8px - Page titles */
--text-3xl: 1.5rem     /* 24px - Large titles */
--text-4xl: 2rem       /* 32px - Hero text */
```

#### Font Families
```css
--font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Inter', 'Roboto', ...
--font-mono: 'JetBrains Mono', 'Cascadia Code', 'Fira Code', 'Consolas', ...
```

**Usage:**
- Sans: UI elements, body text, buttons
- Mono: Fingerprints, public keys, technical data

#### Font Weights
```css
--font-weight-normal: 400     /* Body text */
--font-weight-medium: 500     /* Emphasized text */
--font-weight-semibold: 600   /* Buttons, headings */
--font-weight-bold: 700       /* Strong emphasis */
```

#### Line Heights
```css
--line-height-tight: 1.25     /* Headings */
--line-height-normal: 1.5     /* Body text */
--line-height-relaxed: 1.7    /* Long-form content */
```

#### Letter Spacing
```css
--letter-spacing-tight: -0.025em      /* Large headings */
--letter-spacing-normal: 0            /* Default */
--letter-spacing-wide: 0.025em        /* Small text */
--letter-spacing-uppercase: 0.08em    /* ALL CAPS */
```

---

### Border Radius

Rounded corners for modern, friendly aesthetic.

```css
--radius-xs: 4px       /* Subtle rounding */
--radius-sm: 8px       /* Small elements */
--radius-md: 12px      /* Cards, inputs */
--radius-lg: 18px      /* Buttons, large cards */
--radius-xl: 24px      /* Modals */
--radius-2xl: 32px     /* Extra large surfaces */
--radius-full: 9999px  /* Pills, circles */
```

**Guidelines:**
- Buttons: `--radius-lg` (18px)
- Cards: `--radius-md` (12px)
- Inputs: `--radius-md` (12px)
- Badges: `--radius-full` for pills
- Modals: `--radius-xl` (24px)

---

### Shadows

Elevation system using layered shadows for depth.

#### Base Shadows
```css
--shadow-sm: 0 2px 4px rgba(0, 0, 0, 0.2)
--shadow-md: 0 8px 16px rgba(0, 0, 0, 0.3)
--shadow-lg: 0 16px 40px rgba(0, 0, 0, 0.4)
--shadow-xl: 0 24px 80px rgba(0, 0, 0, 0.6)
```

#### Semantic Shadows
```css
--shadow-accent: 0 4px 12px rgba(99, 102, 241, 0.2)
--shadow-accent-strong: 0 8px 24px rgba(99, 102, 241, 0.4)
--shadow-card: 0 4px 20px rgba(0, 0, 0, 0.25)
--shadow-card-hover: 0 12px 40px rgba(0, 0, 0, 0.4)
--shadow-modal: 0 24px 80px rgba(0, 0, 0, 0.6)
--shadow-toast: 0 8px 32px rgba(0, 0, 0, 0.5)
```

**Elevation Levels:**
1. **Level 0:** Flat (no shadow) - Base surface
2. **Level 1:** `--shadow-sm` - Subtle hover states
3. **Level 2:** `--shadow-card` - Cards, panels
4. **Level 3:** `--shadow-md` - Dropdowns, tooltips
5. **Level 4:** `--shadow-modal` - Modals, dialogs
6. **Level 5:** `--shadow-toast` - Toasts, notifications

---

### Glass Effects

Glassmorphic aesthetic using backdrop blur and transparency.

```css
--glass-blur-sm: blur(20px)
--glass-blur: blur(40px)
--glass-blur-lg: blur(60px)
--glass-saturate: saturate(200%)
```

**Application:**
```css
.glass-surface {
  background: var(--color-bg-card);
  backdrop-filter: var(--glass-blur) var(--glass-saturate);
  border: 1px solid var(--color-border-default);
}
```

**Edge Lighting:**
```css
--edge-light: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.08), transparent);
```

Used to add subtle top highlight on cards for depth.

---

### Transitions

Easing functions and durations for smooth animations.

#### Easing Functions
```css
--ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1)        /* Smooth deceleration */
--ease-out-back: cubic-bezier(0.34, 1.56, 0.64, 1)    /* Spring effect */
--ease-in-out: cubic-bezier(0.4, 0, 0.2, 1)           /* Material Design */
```

#### Transition Presets
```css
--transition-fast: 150ms var(--ease-out-expo)      /* Micro-interactions */
--transition-base: 300ms var(--ease-out-expo)      /* Standard transitions */
--transition-smooth: 500ms var(--ease-out-expo)    /* Page transitions */
--transition-slow: 800ms var(--ease-out-expo)      /* App entrance */
--transition-spring: 500ms var(--ease-out-back)    /* Playful animations */
```

**Guidelines:**
- Hover states: `--transition-fast` (150ms)
- State changes: `--transition-base` (300ms)
- View transitions: `--transition-smooth` (500ms)
- Respect `prefers-reduced-motion`

---

### Z-Index Scale

Layering system for stacking context management.

```css
--z-base: 1          /* Base elements */
--z-dropdown: 100    /* Dropdowns, tooltips */
--z-modal: 9999      /* Modals, dialogs */
--z-toast: 10000     /* Toasts (highest) */
```

**Guidelines:**
- Never use arbitrary z-index values
- Use semantic token names
- Modals should always appear above dropdowns
- Toasts should always be on top

---

## Theming

### Dark Theme (Default)
Primary theme optimized for low-light environments and reduced eye strain.

**Activation:** Default, no attribute required.

### Light Theme
Automatic detection via `prefers-color-scheme: light` or manual toggle.

**Activation:**
```html
<html data-theme="light">
```

**Token Overrides:** Defined in `src/styles/theme.css`

**Key Differences:**
- Lighter backgrounds with reduced opacity
- Higher contrast text colors
- Softer shadows (lower opacity)
- Adjusted accent color for readability

---

## Accessibility

### Color Contrast

**WCAG 2.1 AA Requirements:**
- Normal text (< 18px): 4.5:1 minimum
- Large text (≥ 18px): 3:1 minimum
- Interactive elements: 3:1 minimum

**Current Compliance:**
- Dark theme: ✓ Compliant
- Light theme: ⚠️  Needs audit (see Phase 1 tasks)

### Keyboard Navigation

All interactive elements support keyboard access:
- `Tab` / `Shift+Tab`: Navigate between elements
- `Enter` / `Space`: Activate buttons, links
- `Escape`: Close modals, clear focus
- Custom shortcuts: Documented in `ShortcutHelp.tsx`

### Focus Indicators

Visible focus rings on all interactive elements:
```css
:focus-visible {
  outline: 2px solid var(--color-border-active);
  outline-offset: 2px;
}
```

### Screen Readers

- Semantic HTML elements (`<button>`, `<input>`, `<nav>`)
- ARIA labels on icon-only buttons
- ARIA roles for custom interactive elements
- Live regions for toast notifications

### Reduced Motion

All animations respect user preferences:
```css
@media (prefers-reduced-motion: reduce) {
  * {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

---

## Responsive Breakpoints

Mobile-first responsive design with three breakpoints.

```css
/* Mobile: < 600px (default) */
/* Tablet: 600px - 1000px */
/* Desktop: > 1000px */
```

**Media Query Pattern:**
```css
/* Mobile-first approach */
.element {
  /* Mobile styles (default) */
}

@media (min-width: 600px) {
  .element {
    /* Tablet styles */
  }
}

@media (min-width: 1000px) {
  .element {
    /* Desktop styles */
  }
}
```

**Touch Targets:**
- Minimum size: 44x44px (iOS guidelines)
- Recommended: 48x48px (Material Design)
- Current: 42px buttons (needs adjustment in Phase 5)

---

## Browser Support

**Supported:**
- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

**Required Features:**
- CSS Custom Properties
- CSS Grid / Flexbox
- `backdrop-filter` (graceful degradation)
- ES2020+ JavaScript

**Graceful Degradation:**
- Backdrop blur: Falls back to solid backgrounds
- CSS gradients: Falls back to solid colors
- Modern CSS: No IE11 support (Tauri requirement)

---

## Performance Guidelines

### CSS Best Practices

1. **Use transforms for animations** (GPU-accelerated):
   ```css
   /* ✓ Good */
   transform: translateY(-2px);
   
   /* ✗ Avoid */
   top: -2px;
   ```

2. **Animate opacity and transform only**:
   ```css
   /* ✓ Good */
   transition: transform 300ms, opacity 300ms;
   
   /* ✗ Avoid */
   transition: all 300ms;
   ```

3. **Use `will-change` sparingly**:
   ```css
   /* Only on elements that will animate */
   .animating {
     will-change: transform;
   }
   ```

### Component Guidelines

1. **Avoid inline styles** - Use CSS classes
2. **Minimize re-renders** - Use React.memo where appropriate
3. **Lazy load heavy components** - Code splitting for modals
4. **Optimize images** - Use WebP, proper sizing
5. **Tree-shake icons** - Import only used icons

---

## File Structure

```
src/styles/
├── reset.css          # Minimal CSS reset
├── tokens.css         # Design token definitions
├── theme.css          # Light theme overrides
├── layout.css         # App shell, grid, structure
├── components.css     # Component styles (needs splitting)
└── animations.css     # Keyframe animations
```

**Component CSS Organization** (to be refactored in Phase 2):
```
src/styles/components/
├── button.css
├── input.css
├── card.css
├── badge.css
├── modal.css
├── toast.css
└── ...
```

---

## Design Principles

### 1. Security First
- No external dependencies for UI
- CSP-compliant (no inline styles in production)
- No CDN links for fonts or icons
- Minimal attack surface

### 2. Performance
- Zero runtime CSS overhead
- GPU-accelerated animations
- Tree-shakeable icon system
- Sub-10MB bundle size

### 3. Consistency
- Token-based design (no magic numbers)
- Reusable component system
- Predictable naming conventions
- Semantic HTML

### 4. Accessibility
- WCAG 2.1 AA compliance target
- Keyboard navigation
- Screen reader support
- Reduced motion respect

### 5. Polish
- Thoughtful micro-interactions
- Smooth transitions
- Glassmorphic aesthetic
- Attention to detail

---

## Component Philosophy

### Composition Over Configuration

Components are built for composition:

```tsx
// ✓ Good - Composable
<Card header={{ icon: <LockIcon />, title: "Security" }}>
  <p>Content here</p>
</Card>

// ✗ Avoid - Too many props
<Card 
  showHeader 
  headerIcon="lock" 
  headerTitle="Security"
  content="Content here"
/>
```

### Controlled Components

All form inputs are controlled:

```tsx
// ✓ Good
<Input 
  value={state} 
  onChange={(e) => setState(e.target.value)} 
/>

// ✗ Avoid uncontrolled
<Input defaultValue="..." />
```

### TypeScript Strict Mode

All components use strict TypeScript:
- No `any` types
- Explicit prop interfaces
- Proper null/undefined handling

---

## Common Patterns

### Focus Management

```tsx
import { useRef } from 'react';

const inputRef = useRef<HTMLInputElement>(null);

// Programmatic focus
inputRef.current?.focus();
```

### Keyboard Shortcuts

```tsx
useEffect(() => {
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      // Handle escape
    }
  };
  
  window.addEventListener('keydown', handleKeyDown);
  return () => window.removeEventListener('keydown', handleKeyDown);
}, []);
```

### Conditional Classes

```tsx
const classes = [
  'base-class',
  variant && `base-class--${variant}`,
  active && 'base-class--active',
  className,
]
  .filter(Boolean)
  .join(' ');
```

---

## Next Steps

### Phase 1 Remaining Tasks:
1. ✓ Design token documentation (this file)
2. Component usage guide with examples
3. WCAG color contrast audit
4. Icon system documentation

### Phase 2: Component Refinement
- Split `components.css` into modular files
- Standardize icon sizing system
- Improve component prop APIs

### Phase 3: UX Improvements
- First-time user onboarding
- Enhanced empty states
- Real-time form validation
- File transfer progress indicators

---

## Resources

- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [Material Design Motion](https://material.io/design/motion)
- [Inclusive Components](https://inclusive-components.design/)
- [CSS Tricks: Custom Properties](https://css-tricks.com/a-complete-guide-to-custom-properties/)

---

**Last Updated:** Phase 1 Implementation
**Maintained By:** M2M Development Team
**Status:** Living Document
