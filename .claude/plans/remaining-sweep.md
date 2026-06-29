# Remaining Items — Sweep Plan

## Phase 1 — Ratchet Interval Configurability
**Files**: `crypto.rs`, `session.rs`
- Add `ratchet_interval: u64` field to `Session` (default 100)
- Replace hard-coded `should_ratchet(100)` with `should_ratchet(self.ratchet_interval)` at all 2 call sites (send_encrypted, send_encrypted_typed)
- Test: verify default is 100, verify custom values work

## Phase 2 — Split Icons.tsx
**Files**: `src/components/ui/Icons.tsx` → 20 individual files in `src/components/ui/icons/`
- Current 274-line monolith exports ~20 SVG icon components
- Each gets its own file (e.g., `ShieldIcon.tsx`, `LockIcon.tsx`, etc.)
- `src/components/ui/icons/index.ts` re-exports all for backward compat
- Update all import paths across the frontend
- Bundle analysis: before/after tree-shaking

## Phase 3 — Migrate HubView + ChatView to Focused Hooks
**Files**: `HubView.tsx`, `ChatView.tsx`, `SetupView.tsx`, `App.tsx`, `M2MContext.tsx`
- HubView: `useM2M()` → `useApp()` + `useChat()`
- ChatView: `useM2M()` → `useApp()` + `useChat()`
- SetupView: `useM2M()` → `useApp()` (only needs toasts)
- App.tsx: `useM2M()` → `useApp()` (only needs view + toasts)
- Remove `M2MContext.tsx` and the deprecated `useM2M()` shim
- Update all test mocks to use focused context mocks
- Files remaining on `useM2M()` after: 0 → remove

## Phase 4 — Live Connection Status in HubView
**Files**: `HubView.tsx`
- Read `connection` from ChatContext (available after Phase 3 migration)
- Replace `<OfflineDot /> Offline` with dynamic badge:
  - `connection === null` → "Offline" (gray)
  - `connection?.state === "established"` → "Connected" (green)
  - `isConnecting === true` → "Connecting…" (yellow)
- Show peer fingerprint when connected

## Phase 5 — Dead Code Audit
**Files**: All 42 `#[allow(dead_code)]` sites across 9 files
- For each: is it a reserved constant/enum variant? Remove the annotation and add a comment. Is it dead code? Delete.
- Priority files: `port_mapping.rs` (13), `protocol.rs` (7), `network.rs` (6), `session.rs` (3), `stun.rs` (3), `tor.rs` (3), `crypto.rs` (2), `state.rs` (2), `identity.rs` (2), `relay.rs` (1), `hole_punch.rs` (1)
- Each gets either: removed (if truly dead), or annotated with `// Reserved for future: ...` (if intentional)

## Phase 6 — Docs Refresh
**Files**: `docs/full_analysis.md`
- Update test count to 282
- Update scores to current (9.3/10 overall)
- Mark component tests and integration tests as ✅ Done
- Remove stale entries
- Add Tier B testing section
