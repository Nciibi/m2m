# LoadingStates — Implementation Prompt

## Mission

Implement all loading skeleton components and spinner variants for indicating loading/processing states across the application. These provide visual feedback while data is being fetched or processed.

## Scope

Covers loading state components including:
- Conversation list skeleton (3 shimmer items)
- Message list skeleton (3 alternating bubbles + session banner)
- Settings skeleton (2 cards with shimmer rows)
- Button loading spinner (18px inline ring)
- Full-screen loading spinner (36px ring with optional label)
- Shimmer animation keyframes
- Avatar loading placeholder (pulsing circle)
- Skeleton fade-out transition (200ms)

Does NOT cover: View-specific loading states (SetupView has own sonar ring).

## Files Expected to Be Modified

- `src/components/LoadingSpinner.tsx` — Spinner component
- `src/components/ConvListSkeleton.tsx` — Conversation list skeleton
- `src/components/MessageListSkeleton.tsx` — Message list skeleton
- `src/components/SettingsSkeleton.tsx` — Settings skeleton
- `src/styles/animations.css` — Shimmer keyframes

## Components to Reuse

- **LoadingSpinner** (Section 2.7) — Inline and fullscreen variants

## Components to Create

- **ConvListSkeleton** — 3 shimmer items matching conv item dimensions
- **MessageListSkeleton** — Session banner + 3 alternating bubbles
- **SettingsSkeleton** — 2 cards with 3 shimmer rows each
- **AvatarSkeleton** — Pulsing circle with first letter placeholder

## Layout and Specs

### Conversation List Skeleton

From Design Bible Part 3 Section 23.1:

```
┌──────────────────────────────────────────┐
│  ┌──────┐                                │
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░   ░░░░░░░  │  ← shimmer animation
│  │ ░░░░ │  ░░░░░░░░░░░░░░░░░            │     height: 64px
│  │ 48px │                                │     gap: 8px
│  └──────┘                                │
└──────────────────────────────────────────┘
```

- 3 skeleton items, 64px height each
- Avatar skeleton: 48x48px rounded rectangle (--radius-lg)
- Name skeleton: full-width shimmer bar (--radius-sm)
- Preview skeleton: 60% width shimmer bar (--radius-sm)
- Time skeleton: 40px width shimmer bar, right-aligned

### Message List Skeleton

From Design Bible Part 3 Section 23.2:

```
┌──────────────────────────────────────────┐
│  ┌─── Session Banner Skeleton ───────┐    │
│  │  🔒 ░░░░░░░░░░░░░░░░░░░░░░░░░░░  │    │  ← 60px height
│  └────────────────────────────────────┘   │
│  ─── ░░░░ ───                            │
│                    ┌──────────────────┐   │  ← sent skeleton (right)
│                    │  ░░░░░░░░░░░░░░  │   │     max-width: 60%
│                    │  ░░░░░░░░░░░░░░  │   │     height: 52px
│                    │  ░░░ ░░░░░      │   │
│                    └──────────────────┘   │
│  ┌──────────────────┐                     │  ← received skeleton (left)
│  │  ░░░░░░░░░░░░░░  │                     │     max-width: 50%
│  │  ░░░░░░░░░░░░░░  │                     │     height: 52px
│  │  ░░░ ░░░░░      │                     │
│  └──────────────────┘                     │
└──────────────────────────────────────────┘
```

- Session banner: full-width shimmer bar, 60px height
- Date separator: short shimmer line, centered
- 2-3 alternating sent/received bubble skeletons (52px height)
- Same shimmer animation

### Settings Skeleton

From Design Bible Part 3 Section 23.3:

```
┌──────────────────────────────────────────┐
│  ─── ░░░░░░░ ───                         │
│  ┌────────────────────────────────────┐  │
│  │  ░░░░░░░░░░    ░░░░░░░░░░░░░░░░░  │  │  ← 2-3 cards
│  │  ░░░░░░░░░░    ░░░░░░░░░░░░░░░░░  │  │     each with 3 rows
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
```

- Section title: short shimmer bar
- Card: full width with --radius-lg
- 3 shimmer rows per card (label + value)

### Button Loading Spinner

From Design Bible Section 2.7:
- Size: 18px (inline), 36px (fullscreen)
- Ring: 2px stroke, currentColor with 0.3 opacity on trailing arc
- Animation: spin 0.6s linear infinite
- In buttons: replaces text, centered
- Fullscreen: centered in viewport, optional label below (--text-sm, --color-text-muted, --space-md gap)

### Shimmer Animation

```css
@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}

.shimmer {
  background: linear-gradient(
    90deg,
    rgba(255,255,255,0.03) 25%,
    rgba(255,255,255,0.08) 50%,
    rgba(255,255,255,0.03) 75%
  );
  background-size: 200% 100%;
  animation: shimmer 2s linear infinite;
}
```

Light mode shimmer colors:
```css
.shimmer--light {
  background: linear-gradient(
    90deg,
    rgba(0,0,0,0.03) 25%,
    rgba(0,0,0,0.08) 50%,
    rgba(0,0,0,0.03) 75%
  );
}
```

### Loading State Rules

From Design Bible Part 3 Section 23.4:

| State | Skeleton | Duration | Transition |
|-------|----------|----------|------------|
| Conv list loading | 3 items | Until loaded (max 3s) | Fade out 200ms |
| Messages loading | 3 bubbles + banner | Until loaded (max 3s) | Fade out 200ms |
| Settings loading | 2 cards | Until loaded (max 3s) | Fade out 200ms |
| Search loading | Spinner in bar | Until results | N/A |
| Avatar loading | Pulsing circle + letter | Until loaded | Fade in 300ms |

### Accessibility

- Loading skeletons: aria-label="Loading...", role="status"
- aria-busy="true" on parent container while loading
- Spinner: aria-label="Loading...", role="status"
- After transition: aria-busy="false"

### Performance

- Animate only background-position (GPU-composited)
- Use will-change: background-position on shimmer elements
- Skeleton fade-out uses opacity transition (200ms)

### prefers-reduced-motion

```css
@media (prefers-reduced-motion: reduce) {
  .shimmer {
    animation: none;
    background: rgba(255,255,255,0.05); /* static */
  }
  .spinner__ring {
    animation: none;
    opacity: 0.3;
  }
}
```

## Acceptance Criteria

- [ ] Conv list skeleton: 3 items, 64px each, shimmer animation
- [ ] Message list skeleton: session banner + 3 alternating bubbles
- [ ] Settings skeleton: 2 cards with shimmer rows
- [ ] Button spinner: 18px ring, replaces text
- [ ] Fullscreen spinner: 36px, optional label
- [ ] Shimmer animation: 2s linear infinite, correct colors
- [ ] Fade-out on content loaded: 200ms
- [ ] Avatar skeleton: pulsing circle with letter
- [ ] Loading max duration: 3s before fallback
- [ ] aria-busy toggles correctly
- [ ] prefers-reduced-motion respected
- [ ] GPU-composited animations only

## Self-Review Checklist

- [ ] Follows Design Bible Part 3 Section 23 exactly
- [ ] Shimmer specs match (colors, timing, dimensions)
- [ ] Skeleton dimensions match real component dimensions
- [ ] prefers-reduced-motion implemented
