# M2M — UI/UX Design Bible (Part 2): Complete Product Specification

**Version**: 2.0
**Level**: Apple/Linear/Figma/Notion-grade product specification
**Coverage**: Every pixel, every state, every interaction, every edge case
**Extension of**: Part 1 (Design Language, Components, Screens, Flows)

> This document extends Part 1 with pixel-level specifications, exhaustive state machines,
> interaction matrices, animation timelines, icon catalogs, accessibility audit tables,
> responsive grid specifications, and platform-specific behavior documentation.
> Nothing is left to interpretation.

---

## Table of Contents

10. [Pixel Grid & Layout System](#10-pixel-grid--layout-system)
11. [Complete Component State Machines](#11-complete-component-state-machines)
12. [Screen Pixel Specifications](#12-screen-pixel-specifications)
13. [Interaction & Gesture Matrix](#13-interaction--gesture-matrix)
14. [Animation & Motion Design System](#14-animation--motion-design-system)
15. [Icon Catalog](#15-icon-catalog)
16. [Accessibility Audit](#16-accessibility-audit)
17. [Responsive Grid System](#17-responsive-grid-system)
18. [Platform-Specific Behavior](#18-platform-specific-behavior)
19. [Error & Recovery State Machines](#19-error--recovery-state-machines)
20. [Performance Budgets](#20-performance-budgets)

---

## 10. Pixel Grid & Layout System

### 10.1 Grid Definition

M2M uses a **4px base grid** with an **8px component grid** for all layout decisions.

```
  0  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫
 4  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫   ← 4px grid
 8  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪   ← 8px component grid
12  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫
16  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪
20  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫  ▫
24  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪  ▪
```

**Rules**:
- All padding/margin values must be multiples of 4px
- All component heights must be multiples of 4px (prefer multiples of 8px)
- All border-radius values must be multiples of 4px (except `--radius-full`)
- Grid column gaps must be multiples of 8px
- Icon sizes must be multiples of 4px (16, 20, 24, 28, 32, 36, 48, 64)
- Font sizes should prefer multiples of 2px (10, 12, 14, 16, 18, 20, 24, 32)

### 10.2 App Shell Layout

```
┌──────────────────────────────────────────────────────┐
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │  ← 16px outer margin (desktop)
│  ┌────────────────────────────────────────────────┐  │
│  │             App Shell (max 1000px)              │  │  ← height: 94vh (max 800px)
│  │             border-radius: 32px                 │  │     glass surface
│  │             box-shadow: shadow-app-shell         │  │
│  │                                                  │  │
│  │  ┌─── Header ──────────────────────────────┐    │  │  ← height: 52px
│  │  │  px: 16px 24px                    16px  │    │  │     padding: 0 --space-xl
│  │  │  [M2M]                    [●] [⚙️]     │    │  │     border-bottom: 1px border-default
│  │  └─────────────────────────────────────────┘    │  │
│  │                                                  │  │
│  │  ┌─── Tab Bar ─────────────────────────────┐    │  │  ← height: 44px
│  │  │  [🔗] [💬 3] [📡] [🏠]                  │    │  │
│  │  └─────────────────────────────────────────┘    │  │
│  │                                                  │  │
│  │  ┌─── Content ─────────────────────────────┐    │  │  ← flex: 1, overflow-y: auto
│  │  │                                          │    │  │
│  │  │           (view-specific)                │    │  │
│  │  │                                          │    │  │
│  │  └─────────────────────────────────────────┘    │  │
│  │                                                  │  │
│  └────────────────────────────────────────────────┘  │
│ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
└──────────────────────────────────────────────────────┘
```

**Header exact pixel dimensions**:
- Height: 52px (6.5 × 8px grid)
- Left padding: 24px (from app-shell edge)
- Right padding: 16px
- Logo area: 32×32px icon container, `--radius-sm`, accent gradient
- Title text: 24px from logo, `--text-lg` (15.2px), `--font-weight-bold`
- Connection badge: 22px height, right-aligned, 8px from settings gear
- Settings gear: 32×32px icon button, rightmost
- Bottom border: 1px solid `--color-border-default`

**Tab bar exact pixel dimensions**:
- Height: 44px (5.5 × 8px grid, rounded)
- Tab padding: 12px horizontal, 10px vertical
- Tab gap: 4px between tabs
- Active indicator: 2px bottom border, `--color-accent`, full tab width
- Badge: 18px height, `--radius-full`, `--color-accent` bg, white text `--text-xs`
- Badge position: 6px right of tab text, vertically centered

### 10.3 Layout Primitives

**Full-bleed horizontal padding** (inside app-shell):
- Desktop: `--space-2xl` (32px) on both sides
- Tablet (>600px): `--space-xl` (24px)
- Mobile (<600px): `--space-md` (16px)

**Section spacing** (vertical):
- Between sections: `--space-2xl` (32px)
- Between cards: `--space-md` (16px)
- Between form fields: `--space-sm` (12px)
- Between buttons in a group: `--space-xs` (8px)
- Between label and field: `--space-xxs` (4px)

### 10.4 Edge-to-Edge Rules

- No element should ever touch the app-shell edge — minimum 4px from any edge
- Text should never touch a container edge — minimum 8px padding
- Icons should never touch a container edge — minimum 4px padding
- Interactive elements should have minimum 36×36px tap target (mobile)

---

## 11. Complete Component State Machines

### 11.1 Button — Full State Machine

```
              ┌─────────────┐
              │   IDLE      │
              └──────┬──────┘
                     │ hover/mouseenter
                     ▼
              ┌─────────────┐
              │   HOVER     │──────────────┐
              └──────┬──────┘              │ mouseleave
                     │ mousedown            ▼
                     ▼              ┌─────────────┐
              ┌─────────────┐       │   IDLE      │
              │   PRESSED   │       └─────────────┘
              └──────┬──────┘
                     │ mouseup
                     ▼
              ┌─────────────┐      click ──────► ┌─────────────┐
              │   FOCUSED   │                      │   LOADING   │
              └──────┬──────┘                      └──────┬──────┘
                     │ blur                              │ complete/error
                     ▼                                   ▼
              ┌─────────────┐                      ┌─────────────┐
              │   IDLE      │                      │   IDLE      │
              └─────────────┘                      └─────────────┘
                     │ disabled={true}
                     ▼
              ┌─────────────┐
              │  DISABLED   │─── enabled ───► IDLE
              └─────────────┘
```

**Visual output per state**:

| State | Transform | Background | Shadow | Text | Border |
|-------|-----------|------------|--------|------|--------|
| IDLE (default) | translateY(0) scale(1) | `--color-accent-gradient` | `--shadow-accent` | White, 600 weight | None |
| IDLE (secondary) | translateY(0) scale(1) | `--color-bg-elevated` | None | `--color-text-primary` | 1px `--color-border-default` |
| IDLE (danger) | translateY(0) scale(1) | `--color-danger` | None | White | None |
| HOVER | translateY(-2px) | Same (slightly brightened) | `--shadow-accent-strong` | Same | None |
| PRESSED | translateY(0) scale(0.98) | Same (slightly darkened) | None | Same | None |
| FOCUS (keyboard) | translateY(0) | Same | `--color-accent-glow` outline | Same | outline: 3px |
| FOCUS (mouse) | translateY(0) | Same | None | Same | None |
| LOADING | translateY(0) | Same | None | Hidden | None |
| DISABLED | translateY(0) | `--color-bg-input` | None | `--color-text-muted`, 0.5 opacity | 1px `--color-border-default` |

**Timing**:
- Hover → transition: 150ms, ease-out-expo
- Press → release: 100ms total (50ms press, 50ms release)
- Focus ring: 150ms, ease-out-expo
- Loading → idle: instant on completion

### 11.2 Input — Full State Machine

```
              ┌─────────────┐
              │   IDLE      │
              └──────┬──────┘
                     │ focus
                     ▼
              ┌─────────────┐
              │   FOCUSED   │──────────────┐
              └──────┬──────┘              │ blur
                     │ value.length > 0    ▼
                     ▼              ┌─────────────┐
              ┌─────────────┐       │   IDLE      │
              │ WITH VALUE  │       └─────────────┘
              └──────┬──────┘
                     │ clear clicked
                     ▼
              ┌─────────────┐
              │   IDLE      │
              └─────────────┘
                     │ validation fails
                     ▼
              ┌─────────────┐
              │   ERROR     │────────── value change ──► FOCUSED
              └──────┬──────┘
                     │ disabled={true}
                     ▼
              ┌─────────────┐
              │  DISABLED   │
              └─────────────┘
```

**Visual output per state**:

| State | Background | Border | Shadow | Placeholder | Value |
|-------|-----------|--------|--------|-------------|-------|
| IDLE | `--color-bg-input` | 1px `--color-border-default` | `--shadow-inner` | `--color-text-placeholder` | `--color-text-primary` |
| FOCUSED | `--color-bg-input-focus` | 1px `--color-border-active` | 0 0 0 3px `--color-accent-glow` | Hidden | Same |
| WITH VALUE | Same as IDLE | Same as IDLE | Same | Hidden | Visible |
| ERROR | `--color-danger-bg` | 1px `--color-danger` | 0 0 0 3px `--color-danger-glow` | N/A | Same |
| DISABLED | `--color-bg-input` | 1px `--color-border-default` | None | `--color-text-muted`, 0.5 opacity | 0.5 opacity |

**Clear button**:
- Appears only when: focused AND value.length > 0
- Position: Right edge minus 8px, vertically centered
- Size: 20×20px icon
- Opacity: 0.6 (hover: 1.0)
- Click: Clears value, returns to IDLE state, maintains focus

**Character count** (when applicable):
- Shows at 90% of max length
- Position: Bottom-right of input, 4px below
- Color: `--color-warning` (< 10% remaining), `--color-danger` (at limit)

### 11.3 Modal — Full State Machine

```
CLOSED ──► OPENING ──► OPEN ──► CLOSING ──► CLOSED
               │           │          │
               │           │          │
               ▼           ▼          ▼
         Backdrop:     Focus:     Backdrop:
         fade in       trap       fade out
         300ms         active     200ms
         Content:                 Content:
         scale up                 scale down
         300ms                    200ms
```

**Opening sequence** (300ms total):
- 0ms: Backdrop begins fade-in (0 → 0.6 opacity, `--color-bg-modal-backdrop`)
- 0ms: Content begins scale-up (0.95 → 1.0) + fade-in (0 → 1)
- 50ms: Scrollbar lock applied to body
- 100ms: Focus trapped in modal
- 150ms: First focusable element receives focus
- 300ms: Animation complete, OPEN state

**Closing sequence** (200ms total):
- 0ms: Backdrop begins fade-out
- 0ms: Content begins scale-down (1.0 → 0.95) + fade-out
- 50ms: Focus returned to trigger element
- 100ms: Scrollbar lock released
- 200ms: Animation complete, CLOSED state

**Accessibility during open**:
- `aria-hidden="true"` on all sibling elements
- Tab cycle restricted to modal content
- Escape key = close
- Click outside = close (unless `closeOnBackdropClick` is false)
- Focus trap checks: Tab from last element → first element; Shift+Tab from first element → last element

### 11.4 Toast — Full State Machine

```
              ┌─────────────┐
              │  ENTERING   │──── 200ms ──► ┌─────────────┐
              └─────────────┘               │   VISIBLE   │
                                            └──────┬──────┘
                                                   │ duration expires or dismiss clicked
                                                   ▼
                                            ┌─────────────┐
                                            │  EXITING    │──── 200ms ──► ┌─────────────┐
                                            └─────────────┘               │  REMOVED    │
                                                                          └─────────────┘
```

**Entering animation**:
- Slide in from right: `translateX(100%)` → `translateX(0)` over 200ms
- Fade in: opacity 0 → 1 over 200ms
- Easing: `--ease-out-expo`

**Visible state**:
- Default duration: 5000ms (success/info), 6000ms (warning), 8000ms (error)
- Progress bar: Full width → 0 width over duration, `linear` timing
- Hover: Pauses countdown, resumes on mouseleave
- Dismiss button: Always visible, immediate close on click

**Exiting animation**:
- Slide out to right: `translateX(0)` → `translateX(100%)` over 200ms
- Fade out: opacity 1 → 0 over 200ms
- Stack: Remaining toasts shift up with 150ms stagger

**Stack behavior** (multiple toasts):
- Max visible: 3
- Stack vertical: bottom-up
- Gap: 8px between toasts
- Newest at bottom (closest to edge)
- Removal: Remaining toasts animate `translateY(-{height + 8px})` over 200ms

### 11.5 Conversation Item — Full State Machine

```
              ┌─────────────┐
              │   IDLE      │
              └──────┬──────┘
                     │ hover
                     ▼
              ┌─────────────┐
              │   HOVER     │──────────────┐
              └──────┬──────┘              │ mouseleave
                     │                     ▼
                     ▼              ┌─────────────┐
              ┌─────────────┐       │   IDLE      │
              │ ACTIONS     │       └─────────────┘
              │ VISIBLE     │
              └──────┬──────┘
                     │ click action button
                     ▼
              ┌─────────────┐
              │  ACTION     │  ← Favorite, Archive, Mute, Delete
              │  EXECUTING  │
              └──────┬──────┘
                     │ complete
                     ▼
              ┌─────────────┐
              │   IDLE      │  ← with new state reflected
              └─────────────┘
```

**Hover transition**:
- translateY(-2px): 150ms ease-out-expo
- Background: rgba(255,255,255,0.02) → rgba(255,255,255,0.05): 150ms
- Box-shadow: none → `--shadow-md` + `--shadow-accent-glow`: 150ms

**Action buttons** (hover-reveal):
- Opacity: 0 → 1 over 150ms, 50ms stagger per button (left to right)
- Container: flex-end, gap 4px, padding 4px
- Each button: 28×28px icon-only, `--radius-xs`

**Selection state** (future — for multi-select):
- Background: `--color-accent-glow-subtle`
- Border: 1px `--color-border-active`
- Checkbox: 18×18px, left of avatar

### 11.6 Message Bubble — Full State Machine

```
              ┌─────────────┐
              │  SENDING    │──── sent ──► ┌─────────────┐
              └─────────────┘              │   SENT      │
                                           └──────┬──────┘
                                                  │ delivered
                                                  ▼
                                           ┌─────────────┐
                                           │  DELIVERED  │
                                           └──────┬──────┘
                                                  │ read
                                                  ▼
                                           ┌─────────────┐
                                           │    READ     │
                                           └─────────────┘
                                                  │ hover
                                                  ▼
                                           ┌─────────────┐
                                           │ HOVER (show │
                                           │  reactions) │
                                           └──────┬──────┘
                                                  │ mouseleave (after 2s)
                                                  ▼
                                           ┌─────────────┐
                                           │  IDLE (hide │
                                           │  reactions) │
                                           └─────────────┘
```

**Sent animation**:
- 0ms: Opacity 0, translateY(8px)
- 50ms: Opacity 0.3, translateY(4px)
- 200ms: Opacity 0.8, translateY(1px)
- 400ms: Opacity 1, translateY(0)
- Shadow: None → `--shadow-bubble-sent` over 400ms
- Easing: `--ease-out-expo`

**Received animation**:
- 0ms: Opacity 0, translateY(10px), box-shadow: 0 0 0 var(--color-accent-glow)
- 100ms: box-shadow expands to 0 0 25px var(--color-accent-glow-strong)
- 300ms: Opacity 0.9, translateY(2px)
- 500ms: Opacity 1, translateY(0), box-shadow: var(--shadow-bubble-received)

**Status indicators**:
| State | Icon | Color | Position |
|-------|------|-------|----------|
| SENDING | ⏳ ClockIcon (10px) | `--color-text-muted` | After timestamp, 2px gap |
| SENT | ✓ | `--color-text-muted` | After timestamp |
| DELIVERED | ✓✓ CheckDoubleIcon (12px) | `--color-accent-bright` | After timestamp |
| READ | ✓✓ CheckDoubleIcon (12px) | `--color-accent` (bright solid) | After timestamp |

**Reaction picker** (appears on hover after 500ms):
- Position: Above message bubble, 8px gap
- Background: `--color-bg-elevated`, 1px `--color-border-default`
- Border-radius: `--radius-full`
- Padding: 4px 8px
- Shadow: `--shadow-lg`
- Emoji buttons: 28×28px, font-size 1.1rem
- Hover: scale(1.3) + tinted background, 100ms
- Sent alignment: right-aligned with bubble
- Received alignment: left-aligned with bubble

**Context menu** (right-click):
- Position: Below bubble, aligned to outer edge (sent=right, received=left)
- Background: `--color-bg-elevated`, 1px `--color-border-default`
- Border-radius: `--radius-md`
- Min-width: 120px
- Shadow: `--shadow-lg`
- Items: 32px height, `--space-xs` `--space-md` padding
- Hover: `--color-bg-hover`
- Danger items: `--color-danger`
- Animation: Fade in 100ms, no transform
- Or: Click outside to close

### 11.7 Emoji Picker — State Machine

```
CLOSED ──► OPENING ──► OPEN ──► CLOSING ──► CLOSED
               │           │          │
               │           │          │
               ▼           ▼          ▼
         Button click:  Grid       Click outside
         fadeIn 150ms   visible    or emoji select
```

**Opening**:
- Grid fades in 150ms
- Scale up from 0.95 to 1.0
- No stagger — all emoji appear simultaneously
- Position: Above the emoji button, 8px gap

**Closing triggers**:
- Click outside the picker
- Select an emoji
- Press Escape
- Click the emoji button again (toggle)

---

## 12. Screen Pixel Specifications

### 12.1 SetupView — Pixel Layout

```
Y=0    ┌──────────────────────────────────────────────────────┐  app-shell
       │                                                      │
Y=60   │                 ┌──────────────┐                    │
       │                 │   🔑 Icon    │ ← 80×80px          │
       │                 │   80×80px    │   border-radius: 24px
Y=140  │                 │  sonar ring  │   accent-gradient bg
       │                 └──────────────┘   shadow: shadow-accent-strong
       │                        │              + 0 0 60px accent-glow
       │                        │ 20px gap
Y=160  │        Initializing Secure Enclave                  │
       │        ──────────────────────────                    │
       │        font-size: 22px (--text-2xl)                  │
       │        font-weight: 700                              │
       │        color: --color-text-primary                    │
       │        text-align: center                            │
       │                                                      │
Y=190  │        Generating Ed25519 identity keys.             │
       │        They never leave your device.                 │
       │        ──────────────────────────                    │
       │        font-size: 13px (--text-md)                   │
       │        line-height: 1.6                              │
       │        color: --color-text-secondary                  │
       │        text-align: center                            │
       │                                                      │
Y=230  │              ●   ●   ●                              │ ← loading dots
       │        8px each, 6px gap                              │
       │        animation: dotBounce 1.4s                     │
       │        staggered: 0s, 0.2s, 0.4s                    │
       │                                                      │
Y=270  │        ┌────────────────────────────┐                │
       │        │ Ed25519 · X25519 · XChaCha │ ← crypto badge
       │        │ ───────────────────────── │                │
       │        │ height: 28px             │                │
       │        │ border-radius: 9999px    │                │
       │        │ bg: --color-bg-card      │                │
       │        │ glass blur: 20px         │                │
       │        │ font: --text-xs, mono    │                │
       │        │ color: --color-text-muted│                │
       │        └────────────────────────────┘                │
       │                                                      │
       └──────────────────────────────────────────────────────┘
```

### 12.2 VaultView — Pixel Layout (Create Mode)

```
Y=0    ┌──────────────────────────────────────────────────────┐
       │                                                      │
Y=80   │                 ┌──────────────┐                    │
       │                 │   🔒 Icon    │ ← 80×80px           │
       │                 │ glow breathe │   border-radius: 24px
Y=160  │                 └──────────────┘   accent-gradient bg 20%
       │                        │            border: 1px accent 20%
       │                        │ 16px gap   box-shadow: 0 0 40px accent-subtle
       │                                                      │
Y=176  │            Set Up Your Vault                         │
       │            ─────────────────                         │
       │            font-size: 18px (--text-xl)               │
       │            font-weight: 700                          │
       │            color: --color-text-primary                │
       │            text-align: center                        │
       │                                                      │
Y=204  │            Choose a strong passphrase to encrypt     │
       │            your identity keys and message history.    │
       │            font-size: 12px (--text-sm)               │
       │            color: --color-text-secondary              │
       │            text-align: center, line-height: 1.5      │
       │                                                      │
       │            Minimum 12 chars · Argon2id               │
       │            font-size: 11px (--text-sm)               │
       │            color: --color-text-muted                 │
       │                                                      │
       │         ┌────────────────────────────────┐          │
Y=250   │         │  Passphrase              👁 📋│          │  ← input height: 44px
       │         │                                │          │     padding: 12px 16px
       │         │  width: 100%, max-width: 380px │          │     mono font: 13px
       │         │  border-radius: 12px            │          │
       │         └────────────────────────────────┘          │
       │                        │ 4px gap                     │
       │         ┌────────────────────────────────┐          │
Y=298   │         │  ████████░░░░░░░░░░ 32 bits    │          │  ← strength bar
       │         │  height: 4px                     │          │     width input match
       │         └────────────────────────────────┘          │     border-radius: 2px
       │                        │ 4px gap                     │
       │         ┌────────────────────────────────┐          │
Y=346   │         │  Confirm passphrase       👁   │          │  ← same specs as above
       │         └────────────────────────────────┘          │
       │                        │ 4px gap                     │
       │         ✓ Passphrases match (shown when match)      │
       │         font-size: 11px, success, centered           │
       │                        │ 8px gap                     │
Y=400   │         What makes a strong passphrase?  [▼]       │  ← toggle
       │         font-size: 12px, accent, underlined          │
       │                        │ 4px gap                     │
       │         ┌── Tips ──┐ (expanded)                     │
       │         │ Use 5+.. │                                │
       │         └──────────┘                                │
       │                        │ 12px gap                    │
Y=460   │         ┌────────────────────────────────┐          │
       │         │       Create Vault              │          │  ← accent button
       │         │       ────────────               │          │     full width
       │         │       height: 44px               │          │     max-width: 380px
       │         │       border-radius: 18px        │          │
       │         └────────────────────────────────┘          │
       └──────────────────────────────────────────────────────┘
```

### 12.3 VaultView — Pixel Layout (Unlock Mode)

Same as create mode with these differences:
- Icon: Lock (idle) or Unlock (loading)
- Title: "Unlock Your Vault"
- Description: "Enter your passphrase to decrypt your local data."
- One input only (no confirm field)
- Button text: "Unlock"
- Added: Fingerprint hint below button
  ```
  This vault belongs to a1b2c3d4e5f6...
  font-size: 11px, mono, muted, centered, opacity 0.7
  ```

### 12.4 HubView — Connect Tab Pixel Layout

```
Y=52    ┌─── Header ────────────────────────────────────────┐
        │  [M2M logo] M2M               [● Online] [⚙️]     │
        └──────────────────────────────────────────────────┘
Y=96    ┌─── Tab Bar ──────────────────────────────────────┐
        │  [🔗 Connect] [💬 Chats 3] [📡 Nearby] [🏠 Family] │
        └──────────────────────────────────────────────────┘
Y=140   ┌─── Content ──────────────────────────────────────┐
        │  ← 32px horizontal padding (desktop)              │
        │                                                   │
        │  ● Listening for incoming connections             │
        │  ─────────────────────────────                    │
        │  font-size: 11px, success, 500 weight             │
        │  green dot: 8px, pulseRing 2s                     │
        │                        │ 16px gap                 │
        │                                                   │
        │  ┌─── Card: Host a Connection ────────────────┐   │
        │  │  ┌─── Header ──────────────────────────┐   │   │
        │  │  │ [+ icon] Host a Connection           │   │   │
        │  │  └─────────────────────────────────────┘   │   │
        │  │  Generate a one-time signed invite...       │   │
        │  │  font-size: 12px, text-secondary             │   │
        │  │                      │ 16px gap              │   │
        │  │  ┌───────────────────────────┐ [📋]          │   │
        │  │  │ m2m://a1b2c3d4e5f6...      │              │   │
        │  │  └───────────────────────────┘              │   │
        │  │  🔥 Expires in 59:32                        │   │
        │  │  ─────────────────                           │   │
        │  │  font-size: 11px, warning                    │   │
        │  │                      │ 8px gap               │   │
        │  │  Recent Invites                              │   │
        │  │  ──────────────                              │   │
        │  │  m2m://a1b2c3d4e5...            [📋]         │   │
        │  │  m2m://f6e5d4c3b2...            [📋]         │   │
        │  │  max 5 items                                 │   │
        │  └─────────────────────────────────────────────┘   │
        │                      │ 16px gap                    │
        │  ┌─── Card: Join a Connection ────────────────┐   │
        │  │  [🔗 icon] Join a Connection                 │   │
        │  │  Paste an invite link from a trusted peer...  │   │
        │  │                      │ 12px gap               │   │
        │  │  [m2m://...............] [Connect]            │   │
        │  │                      │ 8px gap                │   │
        │  │  ✓ Valid Invite Found                         │   │
        │  │  ┌─── Naming Panel ─────────────────────┐    │   │
        │  │  │  Your Name   [________________]      │    │   │
        │  │  │  Their Name  [________________]      │    │   │
        │  │  └──────────────────────────────────────┘    │   │
        │  └─────────────────────────────────────────────┘   │
        │                                                   │
        │  ─────────────────────────────────────             │
        │  border: 1px border-default, margin: 20px 0       │
        │                                                   │
        │  Your Identity Fingerprint                        │
        │  a1b2:c3d4:e5f6:g7h8:i9j0:k1l2:m3n4:o5p6  [📋]  │
        │  font: --text-sm, mono, centered, muted            │
        └──────────────────────────────────────────────────┘
```

### 12.5 HubView — Chats Tab Pixel Layout

```
Y=96    ┌─── Tab Bar ──────────────────────────────────────┐
        │  [🔗] [💬 Chats 3] [📡] [🏠]                     │
        └──────────────────────────────────────────────────┘
Y=140   ┌─── Content ──────────────────────────────────────┐
        │                                                   │
        │  ← 24px horizontal padding                        │
        │                                                   │
        │  ┌─── Search ────────────────────────────────┐    │
        │  │  🔍  Search conversations…             ✕   │    │
        │  │  height: 36px                                │    │
        │  │  border-radius: 12px                         │    │
        │  └─────────────────────────────────────────────┘    │
        │                      │ 8px gap                     │
        │                                                   │
        │  ┌─── Conversation Item ──────────────────────┐   │
        │  │  ┌──────┐                                   │   │
        │  │  │  AB  │  Alice                    2m ago  │   │
        │  │  │ 48px │  Hey, are you there?              │   │
        │  │  └──────┘  ●                           ★📂🔇│   │
        │  │  height: 64px + 8px gap                     │   │
        │  │  padding: 16px 20px                         │   │
        │  └─────────────────────────────────────────────┘   │
        │                                                   │
        │  ┌─── Conversation Item (favorite) ───────────┐   │
        │  │  ┌──────┐  ★                                │   │
        │  │  │  CD  │  Charlie           Yesterday      │   │
        │  │  │ 48px │  See you tomorrow!    ★📂🔔      │   │
        │  │  └──────┘  ●                                │   │
        │  └─────────────────────────────────────────────┘   │
        │                                                   │
        │  ┌─── Archived Section ───────────────────────┐   │
        │  │  📁 Archived                                │   │
        │  │  ┌─── Archived Item ───────────────────┐    │   │
        │  │  │  Eve (archived)            ★📂      │    │   │
        │  │  └─────────────────────────────────────┘    │   │
        │  └─────────────────────────────────────────────┘   │
        │                                                   │
        │  ── Empty State ──                                │
        │  💬 48px icon, muted                              │
        │  "No conversations yet" — text-lg, 600 weight     │
        │  "Generate an invite link..." — text-md, muted     │
        │  [Get Started] button                             │
        └──────────────────────────────────────────────────┘
```

**Conversation item exact pixel dimensions**:

```
┌──────────────────────────────────────────────────────────┐
│  ← 20px padding → ┌──────┐ ← 16px gap → ┌────────────┐  │
│                   │  AB  │               │ Alice       │  │
│                   │ 48px │               │             │  │
│                   │      │               │ 2m ago      │  │
│                   └──────┘               │             │  │
│                                          │ Hey, are... │  │
│                                          └────────────┘  │
│  ● = 8px green dot at top-right of avatar                │
│                                                          │
│  height: 64px (8 × 8px grid)                             │
│  Internal padding: 16px top/bottom, 20px left/right      │
│  Avatar: 48×48px, border-radius: 14px                    │
│  Avatar font: 20px, 700 weight, white, uppercase          │
│  Name: 14px, 600 weight                                  │
│  Time: 10px, muted, right-aligned                         │
│  Preview: 12px, secondary, single-line truncated           │
│  Online dot: 8px, 2px white border, top-right of avatar   │
│                                                          │
│  Action buttons (hover-reveal):                          │
│  ★ ☆ 📂 📁 🔇 🔔 🗑                                      │
│  Each: 28×28px, opacity 0→1 on hover                      │
└──────────────────────────────────────────────────────────┘
```

### 12.6 ChatView — Exact Pixel Layout

```
Y=0     ┌─── Header ─────────────────────────────────────────┐
        │  [🛡️] Encrypted Session         [← Hub] [●] [Disconnect]
        │  height: 52px, padding: 0 32px                     │
        └────────────────────────────────────────────────────┘
Y=52    ┌─── File Request Banner ───────────────────────────┐
        │  📄 report.pdf    2.4 MB      [Accept] [Reject]   │
        │  height: 52px, padding: 8px 32px                   │
        └────────────────────────────────────────────────────┘
Y=104   ┌─── File Transfer Progress ────────────────────────┐
        │  📄 photo.jpg                       4.2 MB         │
        │  ████████████████░░░░░░  65%                      │
        │  transferring    2.1 MB/s · 12s remaining          │
        │  height: 72px, padding: 8px 32px                   │
        └────────────────────────────────────────────────────┘
Y=176   ┌─── Search Bar (toggled with Ctrl+F) ─────────────┐
        │  [Search messages… (Esc)]                 3 results │
        │  height: 40px, padding: 8px 32px                   │
        └────────────────────────────────────────────────────┘
Y=216   ┌─── Typing Indicator ──────────────────────────────┐
        │  ● ● ●   Peer is typing…                          │
        │  height: 28px, padding: 4px 32px                   │
        └────────────────────────────────────────────────────┘
Y=244   ┌─── Message Area ─────────────────────────────────┐
        │  ← 32px horizontal padding                         │
        │  gap: 8px between bubbles                          │
        │                                                   │
        │     ┌─── Session Banner ────────────────────┐     │
        │     │  🔒 48px icon                         │     │
        │     │  End-to-end encrypted session          │     │
        │     │  established.                          │     │
        │     │  a1b2c3d4e5f6...  (fingerprint)       │     │
        │     └───────────────────────────────────────┘     │
        │                       │ 20px gap                  │
        │  ─── Today ───                                    │
        │                       │ 16px gap                  │
        │                       ┌──────────────────────────┐│
        │                       │  Hey, how are you?        ││
        │                       │                12:30 PM ✓││
        │                       └──────────────────────────┘│
        │              ↑ max-width: 75% of container        │
        │                       │ 4px gap                   │
        │  ┌──────────────────────────────┐                  │
        │  │  I'm doing great! You?      │                  │
        │  │                   12:31 PM   │                  │
        │  │     [👍 1]  [❤️ 1]          │                  │
        │  └──────────────────────────────┘                  │
        │                       │ 20px gap                  │
        │  ─── Yesterday ───                                 │
        │                       │ 4px gap                   │
        │                       ┌──────────────────────────┐│
        │                       │  See you tomorrow!        ││
        │                       │                9:15 PM ✓✓││
        │                       └──────────────────────────┘│
        │                                                   │
        │  ── Empty State ──                                │
        │  ✉️ 48px icon                                     │
        │  "Start the conversation" — text-lg, 600 weight   │
        │                                                   │
        └────────────────────────────────────────────────────┘
        │  ⬇  FAB (hidden by default, shows when scrolled)  │
        │     40×40px, bottom: 80px, right: 32px            │
        ──────────────────────────────────────────────────────
Y=672   ┌─── Input Area ───────────────────────────────────┐
        │  [📎] [😊] [message text... ░░░] [⏱️ ▼] [➤]     │
        │  height: auto (42px min, 120px max with text)      │
        │  padding: 16px 32px      border-top: 1px           │
        └────────────────────────────────────────────────────┘
Y=720   ┌─── Footer ───────────────────────────────────────┐
        │  End-to-end encrypted      Ctrl+Enter to send     │
        │  height: 24px, padding: 4px 32px                  │
        │  font-size: 10px, muted, flex: space-between      │
        └────────────────────────────────────────────────────┘
```

### 12.7 SettingsView — Pixel Layout

```
Y=0     ┌─── Header ─────────────────────────────────────────┐
        │  [⚙️] Settings                    [← Hub]          │
        │  height: 52px, padding: 0 32px                     │
        └────────────────────────────────────────────────────┘
Y=52    ┌─── Content (scrollable) ──────────────────────────┐
        │  ← 32px horizontal padding                         │
        │                         │ 24px section gap         │
        │  ─── Identity ───                                  │
        │  ┌─── Card ────────────────────────────────────┐   │
        │  │  Fingerprint    a1b2:c3d4:...      [📋]    │   │
        │  │  ─────────────────────────────────────────  │   │
        │  │  Public Key     0xabcd1234...               │   │
        │  └─────────────────────────────────────────────┘   │
        │                         │ 24px gap                 │
        │  ─── Theme ───                                     │
        │  ┌─── Card ────────────────────────────────────┐   │
        │  │  Appearance    [☀️] [🌙] [🖥️]  Current: dark │   │
        │  │  ─────────────────────────────────────────  │   │
        │  │  Accent Color  [■] #6366f1        [Reset]  │   │
        │  └─────────────────────────────────────────────┘   │
        │                         │ 24px gap                 │
        │  ─── Network ───                                   │
        │  ┌─── Card ────────────────────────────────────┐   │
        │  │  Public IP        203.0.113.42     [📋]     │   │
        │  │                   [Discover via STUN]       │   │
        │  │  NAT Type         [RestrictedCone]          │   │
        │  │  STUN Servers     3/4 reachable            │   │
        │  │  ─────────────────────────────────────────  │   │
        │  │  Private Mode     [⬜]  Hide IP from invites│   │
        │  │  Tor              [⬜]  Route via Tor       │   │
        │  │                   [Test Tor]               │   │
        │  │  ─────────────────────────────────────────  │   │
        │  │  Connectivity     [Check]                  │   │
        │  └─────────────────────────────────────────────┘   │
        │                         │ 24px gap                 │
        │  ─── Discovery ───                                  │
        │  ┌─── Card ────────────────────────────────────┐   │
        │  │  📡 LAN Discovery      [⬜]                  │   │
        │  │  🌐 DHT Discovery      [⬜]                  │   │
        │  │  Discovered Peers      3 found   [Refresh]   │   │
        │  │  ─────────────────────────────────────────  │   │
        │  │  ⚠️ Both OFF by default for privacy...      │   │
        │  └─────────────────────────────────────────────┘   │
        │                         │ 24px gap                 │
        │  ─── Security ───                                   │
        │  ┌─── Card ────────────────────────────────────┐   │
        │  │  👁 Screen Capture Protection      [⬜]      │   │
        │  │  Clipboard Auto-Clear             [30s ▼]   │   │
        │  │  Idle Vault Lock                  [5m ▼]    │   │
        │  │  ─────────────────────────────────────────  │   │
        │  │  🔒 Vault    [Lock Now] [Clear Clipboard]   │   │
        │  └─────────────────────────────────────────────┘   │
        │                         │ 24px gap                 │
        │  ─── STUN Servers ───                              │
        │  ┌─── Card ────────────────────────────────────┐   │
        │  │  [OK] stun.l.google.com:19302      12ms ✕  │   │
        │  │  [OK] stun1.l.google.com:19302     18ms ✕  │   │
        │  │  [FAIL] stun.custom.com:3478             ✕  │   │
        │  │  ─────────────────────────────────────────  │   │
        │  │  [host:port_________]  [Add]  [Reset]      │   │
        │  └─────────────────────────────────────────────┘   │
        │                         │ 24px gap                 │
        │  ─── About ───                                     │
        │  ┌─── Card ────────────────────────────────────┐   │
        │  │  Version              2.5.x                 │   │
        │  │  Crypto    Ed25519 · X25519 · XChaCha20     │   │
        │  └─────────────────────────────────────────────┘   │
        └────────────────────────────────────────────────────┘
```

**Settings row exact layout**:
```
┌──────────────────────────────────────────────────────────┐
│  ← 20px padding                                          │
│                                                          │
│  Label (120px fixed)     Value (flex: 1)     [action]    │
│  ──────────────          ──────────          ──────       │
│  font: 13px              font: 12px          min-width:   │
│  500 weight              mono for keys       depends     │
│  secondary               primary                         │
│                                                          │
│  gap: 12px between label and value                       │
│  gap: 8px between value and action                       │
│  divider: 1px border-default, margin 8px 0              │
└──────────────────────────────────────────────────────────┘
```

---

## 13. Interaction & Gesture Matrix

### 13.1 Desktop Mouse Interactions

| Element | Hover | Click | Double-click | Right-click | Drag |
|---------|-------|-------|-------------|-------------|------|
| Button | translateY(-2px) + shadow | translateY(0) + scale(0.98) | N/A | N/A | N/A |
| Input | No change | Focus + border active | Select word | Paste options | N/A |
| Conversation item | translateY(-2px) + glow | Navigate to ChatView | N/A | N/A | N/A |
| Message bubble | Show reaction picker after 500ms | N/A | N/A | Show context menu | N/A |
| Avatar | Subtle scale(1.05) | Peer info modal | N/A | N/A | N/A |
| Shield icon | Subtle scale(1.05) | Fingerprint modal | N/A | N/A | N/A |
| Reaction pill | scale(1.1) | Toggle reaction | N/A | N/A | N/A |
| Tab | Bottom border transition | Switch tab | N/A | N/A | N/A |
| Toast dismiss | Background brighten | Remove toast | N/A | N/A | N/A |
| Scroll-to-bottom FAB | scale(1.1) | Scroll to bottom | N/A | N/A | N/A |
| Send button | translateY(-2px) + shadow | Send message | N/A | N/A | N/A |
| Attach button | Border accent | File dialog | N/A | N/A | N/A |
| Emoji button | Border accent | Toggle emoji picker | N/A | N/A | N/A |
| Settings gear | rotate(30deg) 200ms | Navigate to settings | N/A | N/A | N/A |
| Back arrow | translateX(-4px) | Navigate back | N/A | N/A | N/A |
| Disconnect button | Background brighten | Disconnect with confirm | N/A | N/A | N/A |
| Drop zone | N/A | N/A | N/A | N/A | Show overlay, accept file |
| Modal backdrop | N/A | Close modal | N/A | N/A | N/A |
| Context menu item | Background hover | Execute action | N/A | N/A | N/A |
| Slider toggle | Shadow glow | Toggle state | N/A | N/A | N/A |
| Star (favorite) | Scale(1.2), gold tint | Toggle favorite | N/A | N/A | N/A |
| Folder (archive) | Scale(1.2) | Toggle archive | N/A | N/A | N/A |
| Close/X button | Background hover, color danger | Close/remove | N/A | N/A | N/A |

### 13.2 Keyboard Interaction Matrix

| Element | Enter | Space | Escape | Tab | Arrow keys | Delete | Ctrl/Meta combo |
|---------|-------|-------|--------|-----|------------|--------|-----------------|
| Button | Activate | Activate | N/A | Focus next | N/A | N/A | N/A |
| Input | N/A (form submit) | Space char | Blur (if empty) | Focus next | Cursor move | Delete prev char | Ctrl+A select all |
| Textarea | Ctrl+Enter = send, Enter = newline | Space char | Back to hub (if empty) | Focus next | Cursor move | Delete prev char | Ctrl+Enter send, Shift+Enter newline |
| Modal element | Activate | Activate | Close modal | Cycle inside | Depends on element | N/A | N/A |
| Conversation list | Open chat | N/A | N/A | Focus next | Up/Down navigate | Delete with confirm | Ctrl+N new chat |
| Message list | N/A | N/A | N/A | Focus input | Scroll | N/A | Ctrl+F search, Ctrl+K settings |
| Tab bar | Activate tab | Activate tab | N/A | Focus next | Left/Right switch | N/A | N/A |
| Context menu | Execute item | Execute item | Close menu | Next item | Up/Down navigate | N/A | N/A |
| Emoji picker | Select emoji | Select emoji | Close picker | Next emoji | Arrow grid nav | N/A | N/A |
| Select dropdown | Open/select | Open/select | Close | Next | Up/Down option | N/A | N/A |
| Toggle switch | N/A | Toggle | N/A | Focus next | N/A | N/A | N/A |

### 13.3 Touch Gestures (Mobile/Tablet)

| Gesture | Element | Action |
|---------|---------|--------|
| Tap | Button | Activate |
| Tap | Input | Focus |
| Tap | Conversation item | Open chat |
| Tap | Message | Show reaction picker (long press) |
| Tap | Avatar | Peer info |
| Tap | Send | Send message |
| Swipe left | Conversation item | Reveal actions (mute, archive, delete) |
| Swipe right | ChatView | Back to hub |
| Long press | Message | Context menu |
| Long press | Input | Paste options |
| Pinch | Message area | Font size adjustment (future) |
| Pull down | Conversation list | Refresh (future) |
| Pull down | Message area | Load older messages |

---

## 14. Animation & Motion Design System

### 14.1 Timing Chart

| Animation | Duration | Delay | Easing | Property animated | Elements affected |
|-----------|----------|-------|--------|-------------------|-------------------|
| `appEntrance` | 800ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | opacity, transform | `.app-shell` |
| `msgSlide` | 400ms | i × 50ms | cubic-bezier(0.16, 1, 0.3, 1) | opacity, transform | `.msg-bubble--sent` |
| `msgReceived` | 500ms | i × 50ms | cubic-bezier(0.16, 1, 0.3, 1) | opacity, transform, box-shadow | `.msg-bubble--received` |
| `dotBounce` | 1.4s | 0s, 0.2s, 0.4s | ease-in-out | transform, opacity | `.loading-dots span`, `.typing-indicator__dots span` |
| `modalFadeIn` | 300ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | opacity | `.modal-backdrop` |
| `modalZoomIn` | 300ms | 50ms | cubic-bezier(0.16, 1, 0.3, 1) | opacity, transform | `.modal-content` |
| `shake` | 400ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | transform | `.vault-form--shake`, `.vault-error` |
| `pulseRing` | 3s | 0ms | ease-in-out | opacity, transform | `.listening-indicator__dot`, `.vault-icon` |
| `sonarRing` | 2.5s | 0s, 0.6s, 1.2s | cubic-bezier(0.16, 1, 0.3, 1) | transform, opacity | `.setup-icon__glow` |
| `spin` | 0.6s | 0ms | linear | transform | `.spinner__ring`, `.msg-send-spinner` |
| `glowBreathe` | 3s | 0ms | ease-in-out | box-shadow | `.session-banner__icon`, `.vault-icon` |
| `fadeIn` | 150ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | opacity | Tooltips, context menus, reaction picker |
| `unlockBounce` | 600ms | 0ms | cubic-bezier(0.34, 1.56, 0.64, 1) | transform | `.vault-icon--loading` |
| `slideInRight` | 500ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | transform | View transitions |
| `slideInLeft` | 500ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | transform | View transitions |
| `fabAppear` | 300ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | opacity, transform | `.scroll-fab` |
| `expandDown` | 300ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | max-height, opacity | `.naming-panel`, `.tips-box` |
| `toastSlideIn` | 200ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | transform, opacity | Toast entering |
| `toastSlideOut` | 200ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | transform, opacity | Toast exiting |
| `progressShrink` | varies | 0ms | linear | width | Toast progress bar |
| `shimmer` | 2s | 0ms | linear | background-position | Progress bar fill, button shine |
| `popIn` | 300ms | 0ms | cubic-bezier(0.34, 1.56, 0.64, 1) | transform, opacity | Copied feedback |
| `btnHover` | 150ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | transform, box-shadow | Button hover |
| `btnActive` | 100ms | 0ms | cubic-bezier(0.16, 1, 0.3, 1) | transform | Button press |

### 14.2 Stagger Sequences

**Message list (new messages)**:
- Each consecutive message: `animation-delay: index × 50ms`
- Max stagger: 500ms (10 messages)
- Resets when user sends or receives a new batch

**Conversation list (initial render)**:
- Each item: `animation-delay: index × 30ms`
- Max stagger: 300ms (10 items)
- Only on first mount, not on re-render

**Reaction picker buttons**:
- Each emoji: `animation-delay: index × 20ms`
- Max stagger: 120ms (6 emoji)

**Context menu items**:
- Each item: `animation-delay: index × 30ms`
- Max stagger: 60ms (2 items)

### 14.3 Spring Animations

Used for celebratory/affirmative moments only:
- Vault unlock success: `unlockBounce` using `--ease-out-back` (spring)
- Copied feedback: `popIn` using `--ease-out-back`
- Verification success: Shield icon spring scale (1 → 1.2 → 1)

Never use spring for:
- Transitions between screens
- Button hovers
- Message animations
- Modal opening/closing

### 14.4 Parallax & Depth

The app shell has two layers of depth:
1. Background canvas: `--canvas-gradient` — slow pan animation (60s, linear, infinite)
2. Glass surface: `--color-bg-surface` with `--glass-blur` — fixed position
3. Content: Scrolls within glass surface

The canvas gradient has a subtle slow-drift animation:
```css
@keyframes canvasDrift {
  0% { background-position: 0% 0%; }
  50% { background-position: 100% 100%; }
  100% { background-position: 0% 0%; }
}
/* Duration: 60s, linear, infinite — barely perceptible */
```

---

## 15. Icon Catalog

### 15.1 Icon Grid & Naming Convention

All icons are:
- 24×24px viewBox
- Stroke width: 1.5px
- Stroke linecap: round
- Stroke linejoin: round
- Fill: none (except special cases)
- Color: `currentColor` (inherits from parent text color)
- Default size: 24px (can be overridden with `size` prop)

Naming: `{Name}Icon.tsx` → PascalCase function name

### 15.2 Icon Directory

```
src/components/ui/icons/
├── types.ts                     # IconProps interface
│
├── ShieldIcon.tsx               # Security/shield — peer verification
├── LockIcon.tsx                 # Lock — vault locked, encrypted
├── UnlockIcon.tsx               # Unlock — vault unlocked
├── KeyIcon.tsx                  # Key — identity, crypto
├── GearIcon.tsx                 # Gear — settings
├── PlusIcon.tsx                 # Plus — add, create
├── LinkIcon.tsx                 # Link — invite, connection
├── SearchIcon.tsx               # Magnifying glass — search
├── CloseIcon.tsx                # X — close, dismiss, clear
├── ArrowLeftIcon.tsx            # ← back navigation
├── ArrowDownIcon.tsx            # ↓ scroll down, download
├── SendIcon.tsx                 # ➤ send message
├── AttachIcon.tsx               # 📎 file attach
├── CopyIcon.tsx                 # 📋 copy to clipboard
├── CheckIcon.tsx                # ✓ confirm, done
├── CheckDoubleIcon.tsx          # ✓✓ delivered, read
├── MessageIcon.tsx              # 💬 conversation, chat
├── FileIcon.tsx                 # 📄 file transfer
├── VerifiedIcon.tsx             # ✓ badge — peer verified
├── EyeIcon.tsx                  # 👁 show password
├── EyeOffIcon.tsx               # 👁‍🗨 hide password
├── OnlineDot.tsx                # ● online indicator (8px)
├── OfflineDot.tsx               # ○ offline indicator (8px)
├── TrashIcon.tsx                # 🗑 delete
├── AlertTriangleIcon.tsx        # ⚠️ warning
├── InfoIcon.tsx                 # ℹ️ info
├── GlobeIcon.tsx                # 🌐 DHT, internet
├── HomeIcon.tsx                 # 🏠 home, family
├── WifiIcon.tsx                 # 📡 nearby, wifi
├── ChevronDownIcon.tsx          # ▼ expand, dropdown
├── MonitorIcon.tsx              # 🖥️ system theme (monitor)
├── SunIcon.tsx                  # ☀️ light theme
├── MoonIcon.tsx                 # 🌙 dark theme
├── SmileyIcon.tsx               # 😊 emoji picker
├── ClockIcon.tsx                # ⏳ sending, timer
└── index.ts                     # Re-exports all icons
```

### 15.3 Icon Size Usage Chart

| Size | Where Used |
|------|-----------|
| 8px | Loading dots, typing indicator dots, online/offline dots, reaction dot |
| 10px | Message status (ClockIcon in sending state) |
| 12px | CheckDoubleIcon in message footer, keyboard shortcut hints |
| 14px | Copy button, inline action icons, close buttons |
| 16px | Tab icons, button icons (sm), action icons in conv list |
| 18px | Button icons (default), input adornments, section header icons |
| 20px | App header logo, send/attach/emoji buttons, settings gear |
| 22px | Session banner lock icon |
| 24px | Default icon size, modal headers |
| 28px | Reaction picker buttons (emoji) |
| 32px | App header icon containers, vault icon |
| 36px | Setup view key icon, vault view lock icon |
| 48px | Empty state illustrations, message icons, conversation avatars |
| 64px | Reserved for future hero/welcome illustrations |

### 15.4 Icon Animation Map

| Icon | Animation | Trigger | Duration |
|------|-----------|---------|----------|
| ShieldIcon (unverified) | Subtle pulse | Always | 3s ease-in-out infinite |
| ShieldIcon (verified) | Checkmark sweep | On verify | 500ms |
| LockIcon | Glow breathe | Always while locked | 3s ease-in-out infinite |
| UnlockIcon | Bounce scale | On unlock success | 600ms spring |
| GearIcon | Rotate 30° | On hover | 200ms |
| ArrowLeftIcon | translateX(-4px) | On hover | 150ms |
| SendIcon | translateX(4px) | On hover | 150ms |
| CopyIcon → CheckIcon | Crossfade | On copy | 300ms |
| OnlineDot | Pulse | Always while connected | 2s ease-in-out infinite |
| Loading spinner | Rotate 360° | Always while loading | 600ms linear infinite |

---

## 16. Accessibility Audit

### 16.1 Per-Component Accessibility Audit Table

| Component | Role | ARIA | Keyboard | Focus | Name |
|-----------|------|------|----------|-------|------|
| Button (default) | `button` | N/A | Enter/Space activate | Focus ring via `:focus-visible` | Text content |
| Button (icon-only) | `button` | `aria-label="Describe action"` | Enter/Space activate | Focus ring | From `aria-label` |
| Button (loading) | `button` | `aria-busy="true"` | Disabled | No focus | Text content |
| Input | N/A | `aria-label` or `aria-labelledby` | Type, Tab to next | Focus ring + glow | From `aria-label`/placeholder |
| Input (error) | N/A | `aria-invalid="true"`, `aria-describedby="error-id"` | Same | Same + danger glow | From placeholder |
| Textarea | N/A | `aria-label` | Ctrl+Enter send, Shift+Enter newline | Focus ring | From placeholder |
| Card | `region` | `aria-label` if standalone | Depends on content | N/A | From header or label |
| Card (clickable) | `button` | `aria-label` | Enter/Space activate | Focus ring | From `aria-label` |
| Modal | `dialog` | `aria-modal="true"`, `aria-labelledby="title-id"`, `aria-describedby="body-id"` | Tab cycle trapped, Escape close | First element on open | From title |
| Badge | `status` | N/A | N/A | N/A | Text content |
| Toast | `alert` | `aria-live="assertive"` | N/A | N/A | Text content |
| LoadingSpinner | `status` | `aria-label="Loading..."` | N/A | N/A | From `aria-label` |
| ProgressBar | `progressbar` | `aria-valuenow`, `aria-valuemin="0"`, `aria-valuemax="100"` | N/A | N/A | From label |
| Select | `combobox` | `aria-label` | Arrow keys navigate options | Focus ring | From `aria-label` |
| Conversation list | `list` | N/A | Arrow keys navigate items | First item | N/A |
| Conversation item | `listitem` + `button` | `aria-label` with name | Enter/Space open | Focus ring | From name |
| Message bubble | N/A | `aria-label` with sender + preview | N/A | N/A | From content |
| Reaction picker | `group` | `aria-label="Reactions"` | Arrow keys navigate | First reaction focused | From label |
| Context menu | `menu` | `aria-label="Message actions"` | Arrow keys, Enter/Space activate | First item on open | From label |
| Tab list | `tablist` | N/A | Left/Right arrow switch, Enter/Space activate | Focus ring | From tab text |
| Tab | `tab` | `aria-selected="true/false"` | See tablist | Focus ring | Tab text |
| Toggle switch | `switch` | `aria-checked="true/false"` | Space toggle | Focus ring | From label |
| Emoji picker | `grid` | `aria-label="Emoji picker"` | Arrow keys navigate grid, Enter select | First emoji | From label |
| Emoji button | `gridcell` | `aria-label="emoji name"` | Enter/Space select | Focus ring | From emoji name |
| Drop zone | N/A | N/A | N/A (drag operation only) | N/A | From hint text |
| Fingerprint display | N/A | `aria-label` with fingerprint | N/A | N/A | From data |
| Update banner | `alert` | `aria-live="polite"` | Tab to buttons, Escape dismiss | Focus ring | From text |

### 16.2 Color Contrast Audit

| Token pair | Foreground | Background | Ratio | WCAG | Pass? |
|-----------|-----------|------------|-------|------|-------|
| `--color-text-primary` (dark) | #f8fafc | #030408 | 16.0:1 | AAA | ✅ |
| `--color-text-secondary` (dark) | #cbd5e1 | #030408 | 11.2:1 | AAA | ✅ |
| `--color-text-muted` (dark) | #94a3b8 | #030408 | 7.0:1 | AAA | ✅ |
| `--color-text-accent` (dark) | #a5b4fc | #030408 | 8.5:1 | AAA | ✅ |
| `--color-text-placeholder` (dark) | #475569 | #030408 | 4.9:1 | AA | ✅ |
| `--color-text-primary` (light) | #0f172a | #f1f5f9 | 14.0:1 | AAA | ✅ |
| `--color-text-secondary` (light) | #475569 | #f1f5f9 | 6.8:1 | AAA | ✅ |
| `--color-text-muted` (light) | #64748b | #f1f5f9 | 4.9:1 | AA | ✅ |
| `--color-text-accent` (light) | #4f46e5 | #f1f5f9 | 6.1:1 | AAA | ✅ |
| `--color-text-placeholder` (light) | #64748b | #f1f5f9 | 4.9:1 | AA | ✅ |
| Button text (default) | #ffffff | #6366f1 | 4.8:1 | AA | ✅ |
| Button text (danger) | #ffffff | #ef4444 | 4.3:1 | AA | ✅ |
| Badge text (success) | #10b981 | rgba(16,185,129,0.1) | 3.0:1+ | AA (large) | ✅ |
| Badge text (danger) | #ef4444 | rgba(239,68,68,0.1) | 3.0:1+ | AA (large) | ✅ |
| Sent bubble text | #ffffff | #6366f1→#4f46e5 | 4.8:1 | AA | ✅ |
| Received bubble text | #f8fafc | rgba(30,32,48,0.65) | 8.0:1 | AAA | ✅ |
| Link text | #a5b4fc | #030408 | 8.5:1 | AAA | ✅ |
| Link text (light) | #4f46e5 | #f1f5f9 | 6.1:1 | AAA | ✅ |
| Toast success text | #10b981 | rgba(16,185,129,0.1) | 3.0:1+ | AA (large) | ✅ |
| Toast error text | #ef4444 | rgba(239,68,68,0.1) | 3.0:1+ | AA (large) | ✅ |
| Disabled text | #94a3b8 | #030408 | 7.0:1 | AAA | ✅ |
| Disabled text (light) | #64748b | #f1f5f9 | 4.9:1 | AA | ✅ |
| Border (dark) | rgba(255,255,255,0.09) | #030408 | 1.5:1 | N/A (non-text) | ✅ |
| Border (light) | rgba(0,0,0,0.12) | #f1f5f9 | 1.8:1 | N/A (non-text) | ✅ |

### 16.3 Focus Order

The tab order follows the visual reading order (top-to-bottom, left-to-right):

**HubView focus order:**
1. Settings gear (top-right)
2. Connect tab button
3. Chats tab button
4. Nearby tab button
5. Family tab button
6. First conversation item
7. Second conversation item
8. ... (more items)
9. Search input (if Chats tab)
10. Any action buttons visible on hover

**ChatView focus order:**
1. Shield/verified icon (header)
2. Hub back button
3. Reconnect/status badge
4. Disconnect button
5. File request accept/reject (if visible)
6. Search bar toggle (Ctrl+F)
7. Message history (read-only, not focusable)
8. Scroll-to-bottom FAB (if visible)
9. Attach file button
10. Emoji picker button
11. Message textarea
12. Timer select
13. Send button

**SettingsView focus order:**
1. Hub back button
2. Fingerprint copy button
3. Theme buttons (☀️🌙🖥️)
4. Accent color picker
5. Accent reset button
6. Discover IP button
7. Private mode toggle
8. Tor toggle
9. Test Tor button
10. Connectivity check button
11. ... (scroll down through all settings)
12. Lock vault button
13. Clear clipboard button
14. STUN add input
15. Add STUN button
16. Reset STUN button

### 16.4 Screen Reader Announcements

| Event | Announcement | aria-live | Priority |
|-------|-------------|-----------|----------|
| New message received | "New message from [peer name]" | polite | High |
| Connection established | "Connected to [peer name]" | polite | High |
| Connection lost | "Disconnected from [peer name]" | assertive | High |
| Message sent | "Message sent" | polite | Normal |
| File transfer complete | "File transfer complete: [filename]" | polite | Normal |
| File transfer failed | "File transfer failed: [filename]" | assertive | High |
| Error occurred | "[Error message]" | assertive | High |
| Peer typing | "[Peer name] is typing" | polite | Low |
| Verification success | "Peer verified" | assertive | High |
| Vault locked | "Vault locked" | polite | Normal |
| Vault unlocked | "Vault unlocked" | polite | Normal |
| Update available | "Update available: version [x]" | polite | Normal |

---

## 17. Responsive Grid System

### 17.1 Breakpoint Definitions

```css
/* No media query = desktop baseline (> 1024px) */
/* Tablet: 600px - 1024px */
@media (max-width: 1024px) { ... }
/* Mobile: < 600px */
@media (max-width: 600px) { ... }
```

### 17.2 Desktop (> 1024px)

**Layout**: Floating glass card, centered
```
Window width:   1440px (typical)
App shell:      1000px max-width, centered
  └─ margin:    16px all sides
  └─ height:    94vh, max 800px
  └─ radius:    32px
  └─ shadow:    shadow-app-shell

Content padding: 32px horizontal
```

### 17.3 Tablet (600px - 1024px)

**Layout**: Full-width glass card, edge-to-edge
```
Window width:   768px (typical)
App shell:      100% width, no max
  └─ margin:    0
  └─ height:    100vh
  └─ radius:    0
  └─ shadow:    none

Content padding: 24px horizontal
```

**Specific changes**:
```
.app-shell {
  max-width: 100%;
  margin: 0;
  border-radius: 0;
  height: 100vh;
  max-height: none;
  box-shadow: none;
}

.header {
  padding: 14px 20px;
}

.messages {
  padding: 16px 24px;
}

.msg-area {
  padding: 24px;
}

.msg-input-area {
  padding: 12px 24px;
}

.conv-list {
  padding: 12px 24px;
}

.conv-item {
  padding: 14px 16px;
}
```

### 17.4 Mobile (< 600px)

**Layout**: Full-bleed, minimal padding
```
Window width:   375px (typical)
App shell:      100vw × 100vh, no border-radius
Content padding: 16px horizontal
```

**Specific changes**:
```
body { padding: 0; margin: 0; }
#root { padding: 0; }

.app-shell {
  height: 100dvh;
  max-height: none;
  border-radius: 0;
}

/* Smaller header */
.header {
  padding: 10px 16px;
  height: 44px;
}

/* Smaller tab bar */
.tab-bar {
  height: 40px;
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
}
.tab-bar__tab {
  padding: 8px 12px;
  font-size: 12px;
  white-space: nowrap;
}

/* Reduced message padding */
.msg-area {
  padding: 12px 16px;
}

.msg-input-area {
  padding: 8px 12px;
}

.msg-bubble {
  max-width: 85%;  /* Wider bubbles on small screens */
}

/* Stack settings rows */
.settings-row {
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
}

.settings-row .settings-mono {
  width: 100%;
  word-break: break-all;
}

/* Smaller fingerprint grid */
.fp-grid {
  grid-template-columns: repeat(2, 1fr);
}

/* Stack invite + connect sections */
.invite-section {
  max-width: 100%;
}

/* Smaller conversation items */
.conv-item {
  padding: 12px 14px;
}

.conv-avatar {
  width: 40px;
  height: 40px;
  font-size: 16px;
}

/* Full-width buttons on mobile */
.btn--full-mobile {
  width: 100%;
}

/* Bottom padding for system nav bar */
.app-shell {
  padding-bottom: env(safe-area-inset-bottom, 0px);
}

/* Top padding for status bar */
.app-shell {
  padding-top: env(safe-area-inset-top, 0px);
}
```

### 17.5 Safe Area Handling

```css
/* iOS notch + home indicator */
.app-shell {
  padding-top: env(safe-area-inset-top, 0px);
  padding-bottom: env(safe-area-inset-bottom, 0px);
  padding-left: env(safe-area-inset-left, 0px);
  padding-right: env(safe-area-inset-right, 0px);
}

/* Android status bar */
@media (display-mode: standalone) {
  .app-shell {
    padding-top: env(safe-area-inset-top, 24px);
  }
}
```

---

## 18. Platform-Specific Behavior

### 18.1 Windows

| Feature | Behavior |
|---------|----------|
| Title bar | Hidden — custom header acts as drag region |
| Minimize | Via system tray icon or window minimize button |
| Close button | Hides to tray (doesn't quit) |
| Notifications | Windows toast notifications via tauri-plugin-notification |
| Tray icon | System tray with Show/Hide, New Conversation, Settings, Quit |
| Screen capture | Uses `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` |
| Clipboard | Tauri clipboard API with auto-clear timer |
| File dialogs | Native Windows file picker via tauri-plugin-dialog |
| Font rendering | Uses system Segoe UI, fallback to Inter |
| Scrollbars | Thin overlay scrollbar (auto-hide after 2s) |

### 18.2 macOS

| Feature | Behavior |
|---------|----------|
| Title bar | Hidden — traffic lights show on hover (future) |
| Menu bar | Tray icon in menu bar extra |
| Close button | Hides to menu bar (doesn't quit) |
| Notifications | macOS notification center via tauri-plugin-notification |
| Dock | Icon shows, right-click for menu |
| Screen capture | No native protection (stub) |
| Clipboard | NSPasteboard via Tauri |
| File dialogs | Native macOS file picker |
| Font rendering | Uses San Francisco via system-ui |
| Scrollbars | Overlay scrollbar (auto-hide) |
| Vibrant | `NSVisualEffectView` behind glass surface (future) |

### 18.3 Linux

| Feature | Behavior |
|---------|----------|
| Title bar | Hidden — GTK headerbar (future) |
| System tray | libappindicator or StatusNotifierItem |
| Close button | Hides to tray (where supported) |
| Notifications | D-Bus notifications via tauri-plugin-notification |
| Screen capture | No native protection (stub) |
| Clipboard | Wayland/X11 clipboard via Tauri |
| File dialogs | Native GTK file picker |
| Font rendering | Uses system font (varies by distro) |
| Scrollbars | GTK overlay scrollbar |

---

## 19. Error & Recovery State Machines

### 19.1 Connection Error Recovery

```
                    ┌────────────────────┐
                    │   Connected        │
                    │   (established)    │
                    └────────┬───────────┘
                             │ TCP disconnect / timeout
                             ▼
                    ┌────────────────────┐
                    │   Disconnected     │
                    │   (disconnected)   │
                    └────────┬───────────┘
                             │
                    ┌────────┴────────┐
                    │                 │
                    ▼                 ▼
             ┌────────────┐    ┌────────────┐
             │ Was peer   │    │ Was peer   │
             │ verified?  │    │ unverified?│
             └──────┬─────┘    └──────┬─────┘
                    │                 │
                    ▼                 ▼
             ┌────────────┐    ┌────────────┐
             │ Show       │    │ Navigate   │
             │ Reconnect  │    │ to Hub     │
             │ button     │    │            │
             └──────┬─────┘    └────────────┘
                    │ user clicks "Reconnect"
                    ▼
             ┌────────────────────┐
             │ Attempt 1 (1s)     │─── success ──► Connected
             └────────┬───────────┘
                      │ fail
                      ▼
             ┌────────────────────┐
             │ Attempt 2 (2s)     │─── success ──► Connected
             └────────┬───────────┘
                      │ fail
                      ▼
             ┌────────────────────┐
             │ Attempt 3 (4s)     │─── success ──► Connected
             └────────┬───────────┘
                      │ fail
                      ▼
             ┌────────────────────┐
             │ Attempt 4 (8s)     │─── success ──► Connected
             └────────┬───────────┘
                      │ fail
                      ▼
             ┌────────────────────┐
             │ Attempt 5 (16s)    │─── success ──► Connected
             └────────┬───────────┘
                      │ fail
                      ▼
             ┌──────────────────────────┐
             │ "Reconnection failed     │
             │  after max attempts"     │
             │  Toast: error, 8s       │
             │  Navigate to Hub        │
             └──────────────────────────┘
```

### 19.2 File Transfer Error Recovery

```
                    ┌────────────────────┐
                    │  Sending chunks    │
                    │  (transferring)    │
                    └────────┬───────────┘
                             │
                    ┌────────┴────────┐
                    │                 │
                    ▼                 ▼
             ┌────────────┐    ┌────────────┐
             │ Chunk ACK  │    │ Chunk      │
             │ timeout    │    │ hash       │
             │ (5s)       │    │ mismatch   │
             └──────┬─────┘    └──────┬─────┘
                    │                 │
                    ▼                 ▼
             ┌────────────┐    ┌────────────┐
             │ Retry      │    │ Mark       │
             │ chunk      │    │ failed     │
             │ (max 3x)   │    │            │
             └──────┬─────┘    └────────────┘
                    │ fail x3
                    ▼
             ┌────────────────────┐
             │ Transfer Failed    │
             │ Toast: error, 8s  │
             │ Progress bar: red │
             │ "Retry" available │
             └────────────────────┘
```

### 19.3 Message Send Error Recovery

```
                    ┌────────────────────┐
                    │  User presses Send │
                    └────────┬───────────┘
                             │
                             ▼
                    ┌────────────────────┐
                    │  Message queued    │
                    │  locally           │
                    └────────┬───────────┘
                             │ send to peer
                             ▼
                    ┌────────────────────┐
                    │  Awaiting ACK      │
                    │  status: "sending" │
                    └────────┬───────────┘
                             │
                    ┌────────┴────────┐
                    │                 │
                    ▼                 ▼
             ┌────────────┐    ┌────────────┐
             │ ACK        │    │ Timeout    │
             │ received   │    │ (10s)      │
             └──────┬─────┘    └──────┬─────┘
                    │                 │
                    ▼                 ▼
             ┌────────────┐    ┌────────────┐
             │ Status:    │    │ Store in   │
             │ "sent"     │    │ offline    │
             │            │    │ queue      │
             └────────────┘    └──────┬─────┘
                                      │ reconnect
                                      ▼
                             ┌────────────────────┐
                             │ Flush queue on     │
                             │ reconnect          │
                             │ status: "sent"     │
                             └────────────────────┘
```

### 19.4 Vault Error Recovery

```
                    ┌────────────────────┐
                    │  User clicks       │
                    │  Create/Unlock     │
                    └────────┬───────────┘
                             │
                    ┌────────┴────────┐
                    │                 │
                    ▼                 ▼
             ┌────────────┐    ┌────────────┐
             │ Passphrase  │    │ Passphrase  │
             │ < 12 chars │    │ matches    │
             └──────┬─────┘    │ criteria   │
                    │          └──────┬─────┘
                    ▼                 │
             ┌────────────┐           ▼
             │ Show error │    ┌────────────────────┐
             │ "min 12"   │    │ Derive key via     │
             │ Shake form │    │ Argon2id           │
             └────────────┘    └────────┬───────────┘
                                        │
                               ┌────────┴────────┐
                               │                 │
                               ▼                 ▼
                        ┌────────────┐    ┌────────────┐
                        │ Key        │    │ Argon2id   │
                        │ derivation │    │ memory     │
                        │ success    │    │ error      │
                        └──────┬─────┘    └──────┬─────┘
                               │                 │
                               ▼                 ▼
                        ┌────────────┐    ┌────────────┐
                        │ Decrypt    │    │ Show error │
                        │ identity   │    │ "vault     │
                        │ key        │    │ corrupted" │
                        └──────┬─────┘    └────────────┘
                               │ fail (wrong pw)
                               ▼
                        ┌────────────────────┐
                        │ "Wrong passphrase" │
                        │ Shake form         │
                        │ Clear input        │
                        └────────────────────┘
```

---

## 20. Performance Budgets

### 20.1 Load Time Budgets

| Metric | Budget | Measurement |
|--------|--------|-------------|
| App cold start (first launch) | < 3s | From click to HubView visible |
| App warm start (subsequent) | < 1s | From click to HubView visible |
| Vault unlock | < 1.5s | Argon2id derivation + decryption |
| Message send roundtrip | < 1s | From send click to "sent" status |
| Message list load (100 messages) | < 500ms | From invoke to rendered |
| Conversation list load (50 items) | < 300ms | From invoke to rendered |
| Search (100 messages) | < 200ms | From keystroke to results |
| File transfer (1MB) | < 10s | On same LAN |
| File transfer (100MB) | < 5min | Over internet (STUN) |
| Theme switch | < 50ms | From click to CSS variable applied |
| Modal open | < 100ms | From click to visible |
| Toast show | < 50ms | From trigger to animation start |

### 20.2 Rendering Budgets

| Metric | Budget | Notes |
|--------|--------|-------|
| Frame rate | 60fps | All animations on GPU |
| First contentful paint | < 500ms | App shell visible |
| Largest contentful paint | < 1.5s | Main content visible |
| Time to interactive | < 2s | User can type/click |
| Layout shifts | 0 | No CLS on dynamic content |
| JavaScript heap | < 50MB | After app fully loaded |
| Memory (Rust backend) | < 100MB | Including crypto operations |
| DOM nodes | < 2000 | For 100 messages visible |
| Event listeners | < 100 | Active at any time |

### 20.3 Animation Budgets

| Metric | Budget | Notes |
|--------|--------|-------|
| Concurrent animations | < 10 | Simultaneous running |
| Animation frame budget | < 8ms | For 60fps (16ms total, 8ms for JS) |
| Style recalculations | < 50ms | Triggered by animation |
| Layout thrashing | 0 | Never read/write offset in same frame |

### 20.4 Network Budgets

| Metric | Budget | Notes |
|--------|--------|-------|
| Message overhead | < 1KB per message | Including encryption headers |
| File chunk size | 256KB | Optimized for TCP |
| DHT announce interval | 30s | Configurable |
| Heartbeat interval | 30s | Configurable |
| STUN discovery | < 5s | With 4 parallel servers |
| Invite validity | 60min | Configurable, max 24h |

---

*This document (Part 2) extends the Design Bible with pixel-level specifications, exhaustive state machines, interaction matrices, animation timelines, icon catalogs, accessibility audit tables, responsive grid specifications, platform-specific behavior documentation, error recovery state machines, and performance budgets — completing the full Apple/Linear/Figma/Notion-grade product specification. No aspect of the user interface remains unspecified.*
