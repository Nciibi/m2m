# ErrorStates — Implementation Prompt

## Mission

Implement all error state components and error boundaries for M2M. This includes the per-view error boundary wrappers, the error display UI with expandable details, fatal error screen, and inline form errors with shake animation.

## Scope

Covers error state components including:
- ErrorBoundary wrapper component (per-view)
- Standard error display: ⚠️ AlertTriangle, "Something went wrong", expandable details
- Fatal error display: ❌ "Critical Error", Copy Error Log, Restart App
- Inline form error with shake animation (400ms)
- Error message display methods (toast, inline, modal, badge)

Does NOT cover: Specific error message content (see Part 3 Section 21), toast system (prompt 20, 26).

## Files Expected to Be Modified

- `src/components/ErrorBoundary.tsx` — Error boundary wrapper
- `src/components/InlineError.tsx` — Inline form error with shake
- `src/components/FatalError.tsx` — Fatal full-screen error
- `src/styles/components/utilities.css` — Error styles
- `src/styles/animations.css` — Shake keyframes

## Components to Reuse

- **Button** (Section 2.1) — Dismiss, Reload View, Restart App, Retry, Copy Error Log
- **Card** (Section 2.3) — Error container

## Components to Create

- **ErrorBoundary** — React error boundary with fallback UI
- **InlineError** — Form error with shake animation, --color-danger
- **FatalError** — Full-screen critical error
- **ErrorDetails** — Expandable <details> with monospace error text

## Layout and Specs

### ErrorBoundary — Standard Error

From Design Bible Part 3 Section 27:

```
┌──────────────────────────────────────────┐
│  ⚠️                                        │  ← AlertTriangleIcon, 48px, warning color
│                                         │
│  Something went wrong                     │  ← --text-xl, 700 weight
│                                         │
│  [view name] encountered an unexpected   │  ← --text-sm, secondary
│  error. The application can continue.    │
│                                         │
│  [error message]                         │  ← collapsed by default
│  [▼ Error details]                       │  ← expandable <details>
│                                         │
│  [Dismiss]  [Reload View]  [Restart App] │
└──────────────────────────────────────────┘
```

Specs:
- Background: --color-bg-elevated, centered in view
- Icon: AlertTriangle 48px, --color-warning
- Title: --text-xl, 700 weight, color-text-primary
- Description: --text-sm, color-text-secondary
- Error details: Expandable `<details>` with monospace text
- Buttons: Dismiss (closes boundary, returns to hub), Reload View (remounts), Restart App

### Fatal Error

```
┌──────────────────────────────────────────┐
│  ❌                                        │  ← 48px, danger color
│                                         │
│  Critical Error                           │  ← --text-xl, 700 weight
│                                         │
│  The application encountered a critical  │
│  error and cannot continue safely.       │
│                                         │
│  [error details]                          │
│                                         │
│  [Copy Error Log]  [Restart App]         │
└──────────────────────────────────────────┘
```

- Icon: ❌ (or CloseIcon), 48px, --color-danger
- "Copy Error Log" copies error details to clipboard
- "Restart App" invokes Tauri restart

### Inline Error (Form)

From Design Bible Section 2.2 Input error state:

```
[─── bg-danger-bg ─── border-danger ───]
         ↓ 4px gap
    error message (--text-xs, --color-danger)
```

- Field bg: --color-danger-bg
- Field border: 1px --color-danger
- Focus ring: 0 0 0 3px --color-danger-glow
- Error text: --text-xs, --color-danger, 11px below input
- Animation: shake 400ms on error appearance

**Shake animation:**
```css
@keyframes shake {
  0%, 100% { transform: translateX(0); }
  10%, 30%, 50%, 70%, 90% { transform: translateX(-4px); }
  20%, 40%, 60%, 80% { transform: translateX(4px); }
}
```

### Error Boundary Coverage

From Design Bible Part 3 Section 27.2:

| View | Error Boundary Level |
|------|---------------------|
| SetupView | Per-view |
| VaultView | Per-view |
| HubView | Per-view (with child boundaries for tabs) |
| ChatView | Per-view |
| SettingsView | Per-view |
| Toast system | Global (toast failures don't crash app) |

### Error Message Template

All errors follow this pattern from Part 3 Section 21:
- Exact text (no placeholder)
- Type: error, warning, info
- Display method: toast, inline, modal, badge
- Duration (for toasts): 3s-8s
- Action: what to do (Retry, Dismiss, etc.)

### Accessibility

- Error boundary: role="alert"
- AlertTriangle icon: aria-hidden="true" (decorative)
- Error details <details>: aria-expanded
- Dismiss/Reload/Restart buttons: descriptive aria-label
- Inline error: aria-describedby on the input, aria-invalid="true"

## Acceptance Criteria

- [ ] ErrorBoundary wrapping each view catches errors
- [ ] Standard error shows AlertTriangle 48px + "Something went wrong"
- [ ] Error details expandable with <details>
- [ ] Dismiss returns to hub, Reload remounts, Restart calls Tauri restart
- [ ] Fatal error shows ❌ + "Critical Error" with Copy + Restart
- [ ] Inline form error shows with danger background + border
- [ ] Shake animation (400ms) on form error appearance
- [ ] Error auto-clear on next keystroke (form errors)
- [ ] Error messages use exact text from Section 21
- [ ] aria-invalid and aria-describedby on errored inputs
- [ ] Error boundary: role="alert"
- [ ] All states handled (standard, fatal, inline, toast)

## Self-Review Checklist

- [ ] Follows Design Bible Section 27 and 9.2 exactly
- [ ] Error message catalog from Section 21 referenced
- [ ] Shake animation from Section 14.1
- [ ] Accessibility from Section 16.3
