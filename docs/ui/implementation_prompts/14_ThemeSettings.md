# ThemeSettings — Implementation Prompt

## Mission

Implement the Theme section of SettingsView, providing controls for visual appearance including light/dark/system mode toggles and accent color customization with instant CSS variable updates.

## Scope

Covers the Theme settings card including:
- Appearance mode toggle buttons (☀️ light, 🌙 dark, 🖥️ system)
- Current mode label
- Accent color picker with hex input and swatch
- Reset accent button
- Theme change flow (instant CSS variable update + persist)

Does NOT cover: Full SettingsView shell, theme application to other views (handled by CSS).

## Files Expected to Be Modified

- `src/components/ThemeSettings.tsx` — Component
- `src/styles/theme.css` — Theme variable overrides
- `src/hooks/useTheme.ts` — Theme toggle hook

## Components to Reuse

- **Card** (Section 2.3) — Settings container
- **Button** (Section 2.1) — Mode toggle buttons, reset button

## Components to Create

- **ThemeToggleButtons** — Group of 3 toggle buttons (light/dark/system)
- **AccentColorPicker** — Color swatch with hex input

## Layout Hierarchy

```
<Card title="Theme">
  <SettingsRow label="Appearance">
    <div class="theme-toggles">
      <Button variant={mode === 'light' ? 'default' : 'secondary'} aria-label="Light mode">
        <SunIcon />
      </Button>
      <Button variant={mode === 'dark' ? 'default' : 'secondary'} aria-label="Dark mode">
        <MoonIcon />
      </Button>
      <Button variant={mode === 'system' ? 'default' : 'secondary'} aria-label="System theme">
        <MonitorIcon />
      </Button>
      <span>Current: {mode}</span>
    </div>
  </SettingsRow>
  <SettingsRow label="Accent Color">
    <div class="accent-picker">
      <div class="accent-swatch" style={{ backgroundColor: accentColor }} />
      <Input value={accentColor} variant="mono" compact />
      <Button variant="ghost" onClick={resetAccent}>Reset</Button>
    </div>
  </SettingsRow>
</Card>
```

## Design Implementation Requirements

### Typography

- Current mode label: --text-sm, --color-text-muted
- Accent hex: --font-mono, --text-sm
- Reset button: --text-sm, --color-text-accent

### Colors

- Active button bg: --color-accent (filled)
- Inactive button bg: --color-bg-elevated
- Accent swatch: 32×32px square, --radius-sm, border 1px border-default
- Accent input: mono font, compact input

### Shadows

- Active button: --shadow-accent-glow

### Icons

- SunIcon — Light mode (16px)
- MoonIcon — Dark mode (16px)
- MonitorIcon — System theme (16px)

### Theme Change Flow

From Design Bible Section 4.9:
1. User clicks ☀️ (light), 🌙 (dark), or 🖥️ (system)
2. `data-theme` attribute updates immediately on documentElement
3. CSS variables cascade from theme.css
4. Preference persisted via `set_theme_preference` Tauri command
5. Accent color picker updates `--color-accent` CSS variable instantly
6. Preference persisted

### Error Messages

| ID | Trigger | Message | Type |
|----|---------|---------|------|
| S-011 | Theme save failed | "Failed to save theme preference: {error}" | warning toast, 4s |

## States

| State | Visual | Behavior |
|-------|--------|----------|
| Default (dark) | 🌙 active, others inactive | Standard look |
| Light selected | ☀️ active, accent bg | App switches to light mode |
| System selected | 🖥️ active | Follows OS preference |
| Accent changed | Swatch + input update | CSS variable updates instantly |
| Reset | Returns to #6366f1 | Toast confirmation |

## Acceptance Criteria

- [ ] Three mode toggle buttons (☀️ 🌙 🖥️) with active state highlighting
- [ ] Current mode label updates correctly
- [ ] Clicking a mode button updates data-theme immediately
- [ ] Preference persists across app restart
- [ ] Accent color picker shows current hex value
- [ ] Changing accent updates --color-accent CSS variable instantly
- [ ] Reset button returns accent to #6366f1
- [ ] System mode follows OS preference (matches prefers-color-scheme)
- [ ] Error toast on persistence failure
- [ ] Hover states on all buttons

## Self-Review Checklist

- [ ] Follows Design Bible Section 4.9 and theme settings in Section 3.5
- [ ] CSS variable approach from Section 44
- [ ] Instant visual feedback (no delay)
- [ ] Persistence works across sessions
