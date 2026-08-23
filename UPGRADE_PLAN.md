# M2M — Upgrade Plan: Path to 10/10

> Full-repo audit (2026-08-22): backend security review + frontend/infra review.
> Self-reported score in `ROADMAP.md` is 9.3/10; realistic current score is **~7.5–8/10**.

## Progress log

- ✅ **Phase 1 (C1–C3)** done — transfer caps/validation, key zeroization, relay hardening
- ✅ **Phase 2 (H1–H4)** done — legacy-migration AAD fix, group E2EE trust model v2
  (self-generated member keys, identity-signed bundles, admin checks on receive,
  rotation+fan-out on membership change), conversation-scoped edits/deletes/reactions,
  chunk state/peer gating
- ✅ **Phase 5** done — `cargo audit`/`pnpm audit` now blocking, pnpm store cache on
  setup-node, ESLint (flat config, react-hooks rules; advisory rules as warnings with
  a `--max-warnings` budget), vitest coverage gates (45/55/30/45) wired into CI,
  +28 frontend tests (`utils.ts` entropy/formatting, `useIdleDetection`, `ErrorBoundary`).
  Remaining: component tests for GroupChatView/SetupView/FamilyTab, Playwright e2e,
  cargo llvm-cov gate.
- ✅ **Phase 3 (M1–M6)** done — handshake ±5min freshness window (+test), gap-derivation
  cap (MAX_GAP_DERIVATION=1000) + pre-check of skipped-key cache, HKDF RFC 5869 bound
  assert, `attempt_reconnect` now performs a real authenticated handshake before emitting
  "established", Argon2id + file hashing moved to `spawn_blocking`, IPC-reachable
  `.expect()` panics converted to errors.
  ⏳ **M7 (OPK support)** deferred — needs bundle generation/replenishment, SPK
  persistence/rotation; wire-visible feature work, tracked separately.
- ✅ **Phase 6** mostly done — updater footgun removed (plugin + empty-pubkey config +
  UpdateBanner), `release.yml` with tauri-action draft releases + tag/version check +
  signing-secret hooks. Remaining (needs owner action): obtain Apple/Windows signing
  certs, decide on a hosted updater endpoint before re-enabling it. Linux distro
  packaging (.deb/.rpm/AppImage/AUR) planned as follow-up.
- ⏳ **Phase 0** remaining: rotate `STITCH_API_KEY` + purge git history (destructive;
  requires force-push coordination)

---

## Phase 0 — Immediate (secrets & hygiene)

| # | Action | Reference |
|---|--------|-----------|
| 0.1 | 🔴 **Rotate `STITCH_API_KEY`** and purge `.claude/.env` from git history (git filter-repo / BFG). It was committed in `978fe4c`, deleted in `a495ab0` — still retrievable | git history |
| 0.2 | Delete tracked `VERIFICATION_PENDING.md`; decide CLAUDE.md policy (listed in .gitignore but tracked) | repo root |
| 0.3 | Clean root clutter: `tests_report*.json` (~550 KB), `stitch-*.html`, `prompt.txt`, `src-tauri/target2/`, `cargo_check_output.txt`, `check_err.txt` | repo root |
| 0.4 | Fix three-way version drift: `package.json` = 0.1.0, `tauri.conf.json` = 0.1.0, UI shows "2.5.x" (`SettingsView.tsx:339`), git tags say v3.6.x. Adopt conventional commits | versioning |
| 0.5 | Update CLAUDE.md — the "remaining pre-existing errors" (dht.rs unused import, chat.rs borrow conflicts) are **already fixed**; `cargo check` passes clean | CLAUDE.md |

---

## Phase 1 — Critical security fixes

### C1. Unbounded allocation from peer-controlled `total_chunks`
- `commands/network.rs:968`: `chunks_bitmask: vec![false; total_chunks as usize]` with no upper bound → ~4 GiB OOM DoS from a single 64 KiB frame.
- Also cap concurrent pending incoming transfers (`incoming_transfers`, `network.rs:944-976`).
- `util::create_temp_file` (`util.rs:370-380`) calls `set_len(peer_declared_size)` → disk exhaustion up to full u64 range.
- **Fix**: enforce max file size + max chunk count; reject over-limit transfers; bound pending transfer map.

### C2. Ratchet/X3DH key material never zeroized
- `DoubleRatchet` (`crypto.rs:456-471`): `root_key`, chain keys, `skipped_keys` are plain arrays, no `Drop`.
- `X3DHSessionKeys` (`crypto.rs:315-318`), intermediate DH outputs in `x3dh_initiate/respond` (`crypto.rs:339-425`).
- `SenderKeyChain.chain_key` + `cached_keys` (`crypto.rs:797-810`); `Group.our_signing_key` (`group.rs:73`) is Clone-able plain data.
- `Session::drop` (`session.rs:999-1007`) drops `session_keys` but never touches `self.ratchet`.
- **Fix**: apply `zeroize` derive/Drop to all of the above; stop deriving `Clone` on key structs where possible.

### C3. Relay server hardening
- `generate_relay_id()` (`relay-server/src/main.rs:126-134`): hex of u32 nanos — guessable/time-ordered → bridge hijack via CONNECT.
- `handle_connect` (`main.rs:266-306`) performs **no auth token check** (only REGISTER is protected, `main.rs:225-232`).
- No per-IP/global connection caps or registration limits; non-constant-time token compare (`main.rs:227`).
- **Fix**: CSPRNG 128-bit+ bridge IDs, require token on CONNECT, rate limiting, constant-time compare.

---

## Phase 2 — High-severity protocol/storage fixes

### H1. Legacy at-rest encryption key publicly derivable
- `derive_storage_key(public_key) = SHA256("m2m-storage-key-v1" || public_key)` (`util.rs:321-331`). Void until vault migration runs; profiles without passphrase stay unencrypted-at-rest.
- **Fix**: force migration prompt on first launch; refuse plaintext-key storage mode; delete legacy path after migration window.

### H2. Group E2EE trust model redesign (largest item)
Current model: sender keys distributed over pairwise sessions. Problems:
1. Admin generates & ships every member's private signing key (`group.rs:317-340`, same in `add_member` :395-421) → admin can impersonate anyone.
2. Incoming sender-keys never signature-verified (`group.rs:544-584`, receive loop `network.rs:1586-1610`) → any member can overwrite verification keys and forge signatures.
3. No admin/membership authorization on GroupInfo/Remove/Leave receive paths (`network.rs:1678-1790`).
4. New members can't decrypt existing members' traffic (TODO `group.rs:441-448`); no key rotation on join (only removal, `group.rs:483`).

**Fix plan**: members generate their own signing keys locally; distribute public keys signed by each member's long-term identity; verify signatures on receipt; enforce admin checks in receive handlers; rotate sender keys on membership change; ship initial chain keys to late joiners.

### H3. Unvalidated `chunk_index` on file-chunk writes
- `network.rs:992-1042`: seek/write past declared EOF; repeated index increments counter incorrectly.
- **Fix**: bounds-check against `total_chunks` + bitmask before writing.

### H4. Cross-conversation tampering
- Edit/Delete/Reaction handlers update rows by `message_id` alone with no conversation ownership check (`storage.rs:1061-1156`, `network.rs:1329-1426`).
- Receive side also lacks the 10-char reaction cap applied on send.
- **Fix**: verify message belongs to the sending peer's conversation before mutating; mirror send-side validation on receive.

---

## Phase 3 — Medium-severity hardening

| # | Issue | Location | Fix |
|---|-------|----------|-----|
| M1 | No handshake timestamp freshness check (replayable HandshakeInit) | `session.rs:170-437` | Reject timestamps outside ±window |
| M2 | MAX_SKIP gap derivation before AEAD verify → CPU amplification + skipped-key cache wedging | `crypto.rs:640-652` | Cap gap derivation per message; limit cache growth |
| M3 | `hkdf_expand` u8 overflow if length > 8160 B (latent panic) | `crypto.rs:220-235` | Assert/debug_assert + docs, or widen loop counter |
| M4 | `attempt_reconnect` emits "established" without any cryptographic handshake | `commands/mod.rs:261-305` | Run real handshake before emitting state |
| M5 | Argon2id + sync rusqlite inside async commands; full file loaded into RAM for hashing at completion | `vault.rs`, `chat.rs`, `files.rs`, `groups.rs`, `network.rs:1069` | `spawn_blocking` for Argon2; stream-hash file chunks |
| M6 | IPC-reachable `.expect()` panics | `vault.rs:298,302`, `session.rs:1039` | Return errors instead |
| M7 | X3DH runs without OPKs ever (`one_time_prekey: None` hardcoded); SPK memory-only, invalidated per invite, no rotation/persistence | `commands/network.rs:238`, `session.rs:440` | Implement OPK bundle generation/consumption/replenishment; persist + rotate SPK |

---

## Phase 4 — Frontend quality

### State & correctness
- Fix misleading trust indicator: `handleOpenChat` fabricates `peer_verified: true` (`ChatContext.tsx:252-257`).
- `msgStatus` never advances past "sent" though UI renders delivered/read branches (`ChatView.tsx:48,191,482-483`) — wire backend ACKs (`network.rs:914`) to frontend state.
- Remove dead duplicate transfer-progress subscriptions; dedupe cleanup intervals (`ChatView.tsx:60-65` vs `:68-73`).
- Notification "click" handler misuses `visibilitychange` (`ChatContext.tsx:455`).
- Split the 13-listener mega-effect (`ChatContext.tsx:425-602`) so conversation switches don't tear down/re-subscribe all listeners (missed-event race).
- Consistent send keys (1:1 uses Ctrl+Enter `ChatView.tsx:617`, group uses Enter `GroupChatView.tsx:216`).
- Replace silent `catch {}` blocks with surfaced errors (`ChatContext.tsx`, `GroupChatView.tsx:24,31,89`); add retry UI when app init fails (`AppContext.tsx:50-52`).

### Structure
- Split god components: `ChatView.tsx` (705 lines → markdown renderer, date grouping, self-destruct timer modules), `ChatContext.tsx` (~35 values), `HubView.tsx` tabs typed as `any` (`HubView.tsx:169,308,425`).
- Type fixes: `direction` should be a union type (`types.ts`); import `TransferProgress` instead of re-declaring inline (`ChatView.tsx:50`); fix partial `ConversationEntry` construction (`HubView.tsx:433-440`).

### Accessibility & i18n
- Keyboard operability: fingerprint toggle `<span onClick>` (`ChatView.tsx:268`), reaction picker + context menu hover/right-click only (`ChatView.tsx:436-437,523-555`), tablist arrow-key nav (`HubView.tsx:117-133`, `SetupView.tsx:82-88`), Space activation (`HubView.tsx:375`), emoji dropdown focus management + Escape.
- Introduce i18n framework (all strings currently hardcoded English).

---

## Phase 5 — Testing & CI

### Tests (currently ~94 frontend tests, strong Rust suite)
Add tests for: `GroupChatView.tsx`, `SetupView.tsx`, `FamilyTab.tsx`, `ThemeContext`, `VaultContext`, `utils.ts` (esp. `estimateEntropy`), `useIdleDetection`, `ErrorBoundary`, UI library (Modal focus trap, Input, Select).
Flows: search, pagination, edit/delete/reactions, self-destruct timer, reconnect, markdown renderer, invite expiry.
Add Playwright e2e covering setup → connect → chat → file transfer.

### CI gaps (`.github/workflows/ci.yml`)
- Make `cargo audit` + `pnpm audit` blocking (both currently `continue-on-error: true`).
- Add ESLint (+ config/deps — none exist today).
- Add coverage gates (frontend vitest thresholds + cargo llvm-cov).
- Enable pnpm store cache on `setup-node`.

---

## Phase 6 — Release pipeline

- Wire `tauri-action` to create real GitHub releases (`tagName`/`releaseId`).
- Code signing: macOS signing identity, Windows cert thumbprint, Linux packages — bundles currently unsigned.
- Updater: either configure real `pubkey` + endpoints in `tauri.conf.json:39-43` or remove the updater scaffold + `UpdateBanner.tsx` `check()` call entirely (empty pubkey is a footgun).
- Version bump strategy aligned across manifests + git tags.

---

## Priority order

1. Phase 0 (secret rotation — immediate)
2. Phase 1 (C1–C3)
3. Phase 2 (H1–H4, group E2EE redesign is the largest work item)
4. Phase 6 (release/signing/updater)
5. Phase 3 (M1–M7)
6. Phase 5 (tests/CI)
7. Phase 4 (frontend quality)

## What's already strong (keep)

- Crypto primitives: HKDF-SHA256, X25519, Ed25519, XChaCha20-Poly1305, Double Ratchet with skipped-key replay protection, random initial counters
- Frame validation: 16 MiB cap enforced pre-allocation, Slowloris timeouts, per-IP rate limiting, auth-before-processing
- Storage: fully parameterized SQL, idempotent migrations
- Privacy defaults: LAN/DHT off by default, ephemeral rotating IDs, no IP logging, Tor guard
- Nonce management and constant-time comparisons: clean
