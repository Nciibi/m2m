# Verification Pending

This file tracks build/test steps that could **not** be executed because the
sandbox command classifier was temporarily unavailable (all execution tools —
Bash, PowerShell, Monitor — refused to run). Read-only tools continued to work.

Run these commands manually to close the loop. In the Claude Code prompt you can
prefix with `!` to run in-session (e.g. `!npm run build`).

## Pending commands

| Command | Purpose | Status |
|---------|---------|--------|
| `npm run build` (`tsc && vite build`) | Full type-check + production bundle | NOT RUN — vite bundling step unverified |
| `npm test` (`vitest run`) | Unit/integration test suite | NOT RUN |

## What WAS verified (read-only, passing)

- `tsc --noEmit` ran clean across all 5 views (ChatView, SettingsView, HubView,
  VaultView, GroupChatView). This is the type-checking half of `npm run build`.
- Whole-tree sweep: zero `material-symbols`, zero Tailwind/glass utility classes,
  no `@tailwind` directives, no `tailwind.config.*`, no dead CDN/font link in
  `index.html`.
- Every CSS class referenced by the 5 views resolves to a real rule in
  `src/styles/` (spot-checked conv-*, msg-*, vault-*, setup-*, settings-*,
  app-header__icon-bg--accent, naming-panel).
- Every component prop / type field used in GroupChatView.tsx matches its source
  (ViewName union includes "groups"; GroupInfo/GroupDetail/ChatMessage fields;
  Badge variant "default"; Input compact/mono props).

## Migration summary

- ChatView, SettingsView, HubView, VaultView: reverted from pre-Tailwind git
  ancestors (confirmed feature supersets of the Tailwind HEAD versions).
- GroupChatView: hand-migrated off Tailwind/material-symbols to the project's
  custom-CSS convention (app-shell / Sidebar / app-header / conv-* / msg-* /
  naming-panel + SVG icons from components/ui/Icons).

## Log

- Created after migration phase; build blocked by unavailable command classifier.
