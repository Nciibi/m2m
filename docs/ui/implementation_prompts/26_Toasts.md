# Toasts — Implementation Prompt

## Mission

Implement the toast notification system for providing transient, non-blocking feedback on user actions. The system supports multiple types, auto-dismiss with progress bar, hover-pause, stacking, and animations.

## Scope

Covers the toast system including:
- 4 toast types: success, error, info, warning
- Toast stack management (max 3 visible)
- Auto-dismiss with countdown progress bar
- Hover-to-pause behavior
- Slide-in/slide-out animations
- Dismiss button on each toast
- Accessibility (role="alert", aria-live)

Does NOT cover: Update banner (prompt 20), OS-level notifications.

## Files Expected to Be Modified

- `src/components/ToastContainer.tsx` — Stack manager
- `src/components/ToastItem.tsx` — Individual toast
- `src/styles/components/toast.css` — Toast styles
- `src/hooks/useToast.ts` — Toast state management hook

## Components to Reuse

- **Button** (Section 2.1) — Dismiss button (icon variant)
- **ProgressBar** (Section 2.8) — Countdown progress (small 4px variant)

## Layout Hierarchy

```
<ToastContainer>
  <div class="toast-container" aria-live="assertive">
    <div class="toast toast--success">
      <div class="toast__border" />          <!-- 3px colored left border -->
      <span class="toast__icon">✅</span>
      <span class="toast__body">✓ Vault unlocked</span>
      <ProgressBar variant="success" size="small" />
      <button class="toast__dismiss" aria-label="Dismiss">
        <CloseIcon size={14} />
      </button>
    </div>
  </div>
</ToastContainer>
```

## Design Implementation Requirements

### Specs

From Design Bible Sections 2.6 & 11.4:

- Position: bottom-right, 16px from window edges
- Width: 360px max
- Min-height: 44px
- Border-radius: --radius-md (12px)
- Background: --color-bg-elevated
- Border-left: 3px solid semantic color
- Box-shadow: --shadow-toast (0 8px 32px rgba(0,0,0,0.5))
- Padding: 12px 16px
- z-index: 1000 (toast container)

### Type-Specific Styles

| Type | Border Color | Icon | Duration | Progress Color |
|------|-------------|------|----------|----------------|
| success | --color-success (#10b981) | ✅ checkmark | 4s | --color-success |
| error | --color-danger (#ef4444) | ❌ cross | 8s | --color-danger |
| info | --color-accent (#6366f1) | ℹ️ info | 5s | --color-accent |
| warning | --color-warning (#f59e0b) | ⚠️ warning | 6s | --color-warning |

### Animations

From Design Bible Section 14.1:

| Animation | Duration | Easing | Property | Trigger |
|-----------|----------|--------|----------|---------|
| toastSlideIn | 200ms | ease-out-expo | transform + opacity | Toast enters |
| toastSlideOut | 200ms | ease-out-expo | transform + opacity | Toast exits |
| progressShrink | varies | linear | width | Countdown (full → 0) |

**Entering:**
- translateX(100%) → translateX(0) over 200ms
- opacity 0 → 1 over 200ms

**Exiting:**
- translateX(0) → translateX(100%) over 200ms
- opacity 1 → 0 over 200ms

### Stack Behavior

- Max visible: 3 toasts
- Stack direction: vertical, bottom-up
- Newest at bottom (closest to screen edge)
- Gap: 8px between toasts
- On removal: remaining toasts animate translateY(-{height + 8px}) over 200ms with 150ms stagger

### Hover Behavior

- Mouse enter: pause countdown (pause ProgressBar animation)
- Mouse leave: resume countdown from current position
- Only applies to auto-dismissing toasts (not manual)

### Accessibility

- Each toast: role="alert"
- Container: aria-live="assertive", aria-relevant="additions removals"
- Dismiss button: aria-label="Dismiss notification"
- Toast content should be concise and descriptive
- Toasts do not steal focus

## Acceptance Criteria

- [ ] 4 toast types with correct border color, icon, and duration
- [ ] Slide-in animation from right (200ms)
- [ ] Slide-out animation to right (200ms)
- [ ] Progress bar shrinks over duration (linear)
- [ ] Hover pauses countdown, resume on mouseleave
- [ ] Max 3 toasts visible in stack
- [ ] Newest toast at bottom of stack
- [ ] Remaining toasts shift up on removal (150ms stagger)
- [ ] Dismiss button on each toast
- [ ] role="alert" on each toast
- [ ] aria-live="assertive" on container
- [ ] z-index: 1000
- [ ] Toasts do not steal focus

## Self-Review Checklist

- [ ] Follows Design Bible Sections 2.6 and 11.4 exactly
- [ ] Animations from Section 14.1
- [ ] z-index from Section 25 (1000)
- [ ] Accessibility from Section 16.3
