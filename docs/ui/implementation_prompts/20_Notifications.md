# Notifications — Implementation Prompt

## Mission

Implement the toast notification system and update banner for providing transient feedback on user actions and system events. The toast system supports multiple types, auto-dismiss, hover-pause, stacking, and accessibility requirements.

## Scope

Covers notification components including:
- Toast types: success, error, info, warning (with 3px colored border-left)
- Toast stack (max 3 visible, newest at bottom)
- Auto-dismiss timers (4s/5s/6s/8s)
- Progress bar countdown
- Hover-to-pause, resume on mouseleave
- Slide animations (enter/exit)
- Update banner for new version notification
- z-index management (toast: 1000, update banner: 1000)

Does NOT cover: Local OS notifications (handled by tauri-plugin-notification), push notification server.

## Files Expected to Be Modified

- `src/components/Toast.tsx` — Toast component
- `src/components/ToastContainer.tsx` — Toast stack manager
- `src/components/UpdateBanner.tsx` — Update notification banner
- `src/styles/components/toast.css` — Toast styles
- `src/styles/components/utilities.css` — Update banner styles

## Components to Reuse

- **Button** (Section 2.1) — Dismiss icon button, Update Now action
- **ProgressBar** (Section 2.8) — Countdown progress (small 4px variant)

## Components to Create

- **ToastContainer** — Fixed-position container managing toast stack
- **ToastItem** — Individual toast with type styling
- **UpdateBanner** — Version update notification

## Layout Hierarchy

**Toast Container:**
```
<div class="toast-container">
  <ToastItem type="success">
    <span class="toast__icon">✅</span>
    <span class="toast__body">✓ Vault unlocked</span>
    <ProgressBar variant="success" size="small" value={progress} />
    <Button icon aria-label="Dismiss"><CloseIcon /></Button>
  </ToastItem>
  <ToastItem type="error" visible={true}>
    <!-- ... -->
  </ToastItem>
</div>
```

**Update Banner:**
```
<div class="update-banner">
  <span>📦</span>
  <span>Update available: v1.2.3</span>
  <Button variant="default" sm>Update Now</Button>
  <Button icon aria-label="Dismiss"><CloseIcon /></Button>
</div>
```

## Design Implementation Requirements

### Toast Specs

From Design Bible Section 2.6 and 11.4:

- Position: bottom-right, 16px from edges
- Width: 360px max
- Height: auto (min 44px)
- Border-radius: --radius-md
- Background: --color-bg-elevated
- Border-left: 3px solid semantic color
- Shadow: --shadow-toast
- z-index: 1000 (toast container)

**Type-specific styling:**

| Type | Border Color | Icon | Duration | Progress Color |
|------|-------------|------|----------|----------------|
| success | --color-success (#10b981) | ✅ | 4s | --color-success |
| error | --color-danger (#ef4444) | ❌ | 8s | --color-danger |
| info | --color-accent (#6366f1) | ℹ️ | 5s | --color-accent |
| warning | --color-warning (#f59e0b) | ⚠️ | 6s | --color-warning |

### Animations

From Design Bible Section 14:

| Animation | Duration | Easing | Trigger |
|-----------|----------|--------|---------|
| toastSlideIn | 200ms | ease-out-expo | Toast enters (translateX(100%) → 0) |
| toastSlideOut | 200ms | ease-out-expo | Toast exits (0 → translateX(100%)) |
| progressShrink | varies | linear | Progress bar shrinks over duration |
| stagger | 150ms | — | Remaining toasts shift after removal |

### Stack Behavior

- Max visible: 3 toasts
- Stack vertical: bottom-up (newest at bottom, closest to edge)
- Gap: 8px between toasts
- Removal: remaining toasts animate translateY(-{height + 8px}) over 200ms

### Update Banner Specs

From Design Bible Section 2.15:

- Position: fixed bottom-right, 16px from edges
- Background: --color-bg-elevated
- Border: 1px --color-border-accent
- Border-radius: --radius-lg
- Shadow: --shadow-lg
- Animation: slide up 200ms
- Dismiss: X button or update installed
- z-index: 1000

### Accessibility

- Each toast: role="alert", aria-live="assertive"
- Toast container: aria-live="polite", aria-relevant="additions removals"
- Update banner: role="alert", aria-live="polite"
- Dismiss button: aria-label="Dismiss notification"
- Update button: aria-label="Download and install update"
- Focus: updates toasts do not steal focus

### Edge Cases

- **Toast during modal open**: Toast visible above content (higher z-index than modal content? No — toast at 1000, modal at 9999. Toast is below modal backdrop.)
  - Correction from spec: Toast z-index is 1000, modal is 9999. So toasts appear BELOW modals.
  - Wait, spec says --z-toast: 1000, --z-modal: 9999. Toast should be above modal. Let me recheck... spec says toast at 1000, modal at 9999. So modal is higher. Toasts can be hidden behind modal. That's intentional — critical dialogs should not be obscured.
- **Toast during drag**: Toast unaffected (different stacking context)
- **Rapid successive toasts**: Queue them; show latest 3; older ones auto-dismiss
- **Hover pause**: setInterval paused on mouseenter, resumed on mouseleave

## Acceptance Criteria

- [ ] 4 toast types with correct border colors, icons, and durations
- [ ] Toast slides in from right (translateX(100%) → 0) over 200ms
- [ ] Toast slides out to right over 200ms
- [ ] Progress bar shrinks from full to 0 over the duration
- [ ] Hover pauses countdown, resume on mouseleave
- [ ] Max 3 toasts visible in stack
- [ ] Newest toast at bottom (closest to edge)
- [ ] Remaining toasts shift up on removal (150ms stagger)
- [ ] Dismiss button on each toast
- [ ] Update banner slides up with version info and actions
- [ ] Update banner dismissible with X button
- [ ] z-index: toast container at 1000
- [ ] role="alert", aria-live="assertive" on toasts
- [ ] Hover pause/resume works correctly
- [ ] No focus stealing on toast appearance

## Self-Review Checklist

- [ ] Follows Design Bible Sections 2.6, 2.15, 11.4
- [ ] Animations from Section 14
- [ ] z-index from Section 25 (1000)
- [ ] Accessibility from Section 16
