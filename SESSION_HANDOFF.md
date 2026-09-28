# SESSION HANDOFF — M2M architecture remediation

**Date:** 2026-09-28
**Repo:** `/mnt/hdd/projects/M2M` (branch `main`, HEAD `70cf7ed`)
**Baseline score given at session start:** 6.5 / 10
**Goal:** fix the defects found in a deep architecture scan, highest severity first.

---

## 1. Read this first — the critical constraint

### 1.1 The app's Rust tests have never been run in this environment

`cargo check` / `cargo test` / `cargo clippy` **cannot build the Tauri app here.**
The `glib-sys` / `gio-sys` / `gdk-sys` build scripts need GTK dev packages
(`.pc` files) that are absent. Only the GTK *runtime* is in the nix store, not
the dev output.

This is why a verification harness was built (see §3). **The app's own 378 tests
still need a run in a GTK-capable environment.** Anything claimed "verified"
below means "verified through the harness", not "verified by the app's test suite."

### 1.2 The toolchain locations

- `node` is not on `PATH`. It lives at
  `/nix/store/lfaydgacdyngci7p60s8wwvgdm74fjkx-nodejs-24.19.0/bin`.
  Export it before running `tsc` / `vitest` / `eslint`.
- `rustfmt` and `clippy` are **not installed**. Neither is `cargo fmt`.
- GTK dev libs are not obtainable (no network for nix, not in the store).

### 1.3 An external process auto-commits the working tree

Commits appear every few seconds (`v3.6.1138` and counting). Consequences:

- `git status` and `git diff` are **useless** for seeing my own work — the tree
  is always clean because everything is already committed.
- `git checkout <file>` restores from HEAD, which *does* contain my changes
  (the auto-committer swept them), but this is a trap worth avoiding.
- To inspect what I changed, use the marker greps in §2 and `git log -p`.

---

## 2. Work completed and verified

### 2.1 Crypto — real defects

| Fix | File(s) | Why it mattered |
|---|---|---|
| `used_opk` folded into the signed handshake transcript | `session.rs` | It selects X3DH DH4 participation, so an active MITM could rewrite it and force a 3-DH downgrade both sides accepted as valid |
| One-time prekey reserved under a **write** guard for the whole handshake | `commands/network.rs` | Read-then-retire-later was a TOCTOU: two concurrent handshakes both applied DH4 with the same secret |
| Skipped-key cache lookup no longer gated on `recv_message_number`; targeted prune instead of blanket `clear()` | `crypto.rs` | A ratchet resets the counter, so in-flight messages never consulted the cache — **every message crossing a ratchet was permanently lost**, ~1 in 100 sends |
| One-time invites actually enforced; `HandshakeInit::one_time` added and signed | `protocol.rs`, `session.rs`, `commands/network.rs` | The flag was generated, shown in the UI, and never checked — a "one-time" invite was replayable until expiry |
| Redacting `Debug` on `SenderKeyChain`, `CachedSenderKey`, `Group` | `crypto.rs`, `group.rs` | `#[derive(Debug)]` printed the group signing seed and every cached AEAD key+nonce |
| 6 un-zeroized HKDF buffers, 2 needless session-key clones | `crypto.rs` | Raw X25519 shared secret and live chain keys left in freed heap |
| Corrected the false "this is HKDF-Expand" doc on a raw `SHA256(K‖c)` | `crypto.rs` | Misleading on a crypto primitive |
| Pre-epoch clock no longer `panic!` | `identity.rs` | Was a second copy of a function already fixed in `session.rs` |
| `mlock` failure no longer `abort!`s | `secure_key.rs` | `StorageKey::new` calls it on **every vault unlock**; `RLIMIT_MEMLOCK` is commonly 64–8192 KB |

New regression tests: `test_in_flight_message_survives_dh_ratchet`,
`test_skipped_cache_stays_bounded_across_ratchets`.

### 2.2 Dead subsystems that were advertised as working

- **Relay keepalive** — client never sent `0x03`, server never refreshed on it, so registrations died at 5 min while `connected: true`. Fixed both sides; `test_keepalive_refreshes_idle_timer` added (relay 15/15).
- **Hole punching** — responder bound the already-occupied listener port → `EADDRINUSE`, `Role::Responder` unreachable. Removed as connect-only with an explanation; a coordinated simultaneous-open is a protocol change, not a patch.
- **DHT** — announced to an empty bootstrap list forever. Now seeds from LAN peers, drops `#![allow(dead_code)]`, and warns loudly when it has nothing to gossip to.
- **Relay rejected `X3DHHandshakeInit`** outright, so X3DH peers couldn't use the path that most needs it.

### 2.3 Concurrency

- **60 sites** held the global `connections` read guard across `.await`.
  Added `AppState::peer_connection()` and converted all of them. One of these
  (sync-resend) could pin the lock ~5.5 hours from a single 20-byte frame.
- **Two deadlock cycles** removed structurally:
  `identity`⇄`group_manager` (via `our_peer_key_hex()` / `our_identity_kp()`) and
  `storage_key`⇄`message_store` (order rule documented in `commands/util.rs`).
- **Remote vault-lock DoS** — `identity.read()` was held across an
  unauthenticated handshake; a trickling peer could stop the user locking their vault.
- `ensure_message_store` / `ensure_transfer_store` no longer take the global mutex on the fast path.
- `group_manager` write lock no longer held across a SQLite open.

### 2.4 Privacy / SSRF

- **UDP chokepoint** — `dial::bind_udp_for_external_query()`. STUN / PCP /
  NAT-PMP / SSDP had no guard, so Tor leaked the real IP from 5 production paths.
- **UPnP hop-2 SSRF** — the `controlURL` from the device description was POSTed
  unvalidated; the cloud-metadata deny-list was bypassable.
- **STUN server list** — `contains(':')` accepted `"a:b"`; now capped, parsed, bounded.

### 2.5 Structural

`relay.rs`'s 145-line fork of `handle_incoming_connection` is gone. Both
transports now call the single `commands::network::complete_inbound_connection`.
That fork had already drifted (live STUN on an unauthenticated socket, identity
held across the handshake, no X3DH dispatch).

### 2.6 File transfer (second batch)

- **Delivery-confirmation forgery.** `chunks_acked += ack.chunk_index -
  last_acked_index + 1` assumed no gaps, so one `ChunkAck` for the final index
  satisfied `wait_for_ack` for every remaining chunk. Now a contiguous-prefix
  rule, extracted as pure `advance_ack_watermark()` with 3 tests.
- **Progress was 2× wrong on every relay transfer** — used
  `protocol::MAX_FILE_CHUNK_SIZE` instead of `t.chunk_size` (128 KiB on relay).
- Sender's per-chunk `open`/`seek`/`read` moved to `spawn_blocking`.
- Pre-announced `chunk_hashes` are now actually verified. They were transmitted
  every transfer and dropped; the only check being applied compared each chunk
  against a hash the same peer supplied.

### 2.7 Frontend

- `onDeleteConversation` was still discarding the id (documented as fixed, was
  not). Handler now takes the id and owns the delete. **The signature change
  caught a test passing it straight to `onClick`.**
- `asCaptureWarning` existed, was tested, and was never called. Now used.
- `asArray` added to `events.ts`; the duplicate 7-line `asList` in `ChatContext`
  now delegates to it. Applied at 5 sites that would have thrown on `null`.
- `SettingsView` was `JSON.stringify`-ing the connectivity result into the DOM —
  including the user's own WAN/LAN addresses, which the type's own comment says
  must be treated as sensitive.
- `data-theme` was written by two providers; the OS listener clobbered the user's
  explicit choice. Removed from `AppContext`.
- `Esc`-in-a-modal also navigated to Hub (Modal listens on `document`,
  AppContext on `window`). Now guarded by a `[role="dialog"]` check.
- `mousemove` → rAF-throttled (2 custom-property writes per event, ~240
  forced style recalcs/sec at 120 Hz).
- `direction: string` → `"sent" | "received"`. **This immediately caught test
  fixtures using `"incoming"`/`"outgoing"` — values the app never emits.**
- Duplicate `TransferProgressEventPayload` → alias; removed 2 double-casts.
- `z-index: 9998` → `--z-banner` token.

### 2.8 Styling

Six tokens used in CSS were never overridden for light theme and are
white-on-white in it: `--color-bg-tertiary`, `--color-bg-chip` (mute/read chips
invisible), `--edge-light`, `--shadow-bubble-received` (received bubbles had no
edge), plus `--shadow-inner` and `--color-bg-modal-backdrop`.
`--color-danger` and `--color-warning` were **both `#d97706`** — an error toast
and a warning toast were the same colour.

### 2.9 Cleanup

- Deleted 0-byte `src-tauri/src/files.rs` (orphan, not in `lib.rs`).
- Deleted `hub.css` (109 lines) and `chat.css` (69 lines) — 100% dead, and
  `hub.css`'s `body` block referenced an undefined `--indigo-glow` which made
  `background-image` compute to `none` and, via `!important`, killed
  `--canvas-gradient` entirely.
- Centralised the 5-copy passphrase policy into `util::validate_passphrase()`
  (2 copies had drifted in wording).
- Removed 6 genuinely dead constants and corrected 2 comments citing a deleted
  rate limit as if live.
- Removed 4 stale `#[allow(dead_code)]` annotations that no longer described reality.
- `AAD_MSG_STORE` de-duplicated: `commands/util.rs` re-exports the single
  definition in `storage.rs` instead of keeping a hand-synchronised copy.
- `apply_connection_pragmas()` at all 3 store opens: `secure_delete` was only
  set on delete paths (so routine churn left free-page remnants), and
  `foreign_keys` was never enabled (making every FK clause decorative).

### 2.10 Verification results

| Check | Result |
|---|---|
| `cargo check` (harness, all 50 modules) | see §4 — **4 errors outstanding** |
| `cargo test` (relay-server) | **15 passed** |
| `tsc --noEmit` | **clean** |
| `pnpm test` / vitest | **313 passed** |
| `pnpm lint` / eslint | **0 errors, 10 warnings** (at the pinned budget) |

---

## 3. The verification harness (important)

Location: `/tmp/opencode/tch`

A cargo crate that compiles **all 50 source modules**, including the Tauri
command layer, with consistent dependency versions.

- `src/tauri_stub/` — a stub crate named `tauri` providing `AppHandle`, `State`,
  `Emitter`, `Manager`, `WindowEvent`, `async_runtime::spawn_blocking`,
  `menu::*`, `tray::*`, `image::Image`. It is a harness artifact and never ships.
- `check.sh` — copies the live `src-tauri/src` into the harness, strips
  `#[tauri::command]` / `#[tauri::main]` (attribute macros need a proc-macro
  crate) and rewrites `tauri::` to the stub. Then run `cargo check --offline --lib`.
- `Cargo.lock` is **copied from the workspace** so cargo resolves the same
  versions that are already in the local registry cache. Without it, offline
  resolution fails on uncached crates.

### 3.1 ⚠️ DANGER: the earlier harness destroyed 20 source files

A previous harness (`/tmp/opencode/tc`, rlib-based) created **symlinks** from
`src/*.rs` to the live files. A `sed ... >` redirect then wrote *through* the
symlinks and truncated every `src-tauri/src/*.rs` to 0 bytes. All 20 were
restored from git (newest non-empty blob per file), and all changes verified
intact, but this is a real hazard.

`check.sh` now has an explicit symlink guard and refuses to run if any target is
a symlink. **Do not reintroduce symlinks into any harness.**

### 3.2 What the harness earned its keep on

Building it found **6 real bugs in my own refactors** that would otherwise have
shipped broken:

1. `protocol::RawFrame` — the type is in `network`, not `protocol`.
2. `IdentityKeypair` undeclared in `commands/network.rs`.
3. `state.lan_state.clone()` — `RwLock` is not `Clone`.
4. `file_path.to_path_buf()` where `file_path: &str`.
5. `?` used in `complete_inbound_connection`, which returns `()`.
6. `StrategyResult` destructure missing the `role` field.

---

## 4. IN PROGRESS — the error taxonomy (unfinished)

### 4.1 What was built

**`src-tauri/src/error.rs`** (373 lines, new, registered in `lib.rs` as
`pub mod error;`)

`AppError { code: &'static str, message: String }`, serialising to
`{ "code": "crypto.replay_detected", "message": "..." }`.

- Constructors: `new`, `invalid`, `blocked`, `not_connected`, `storage`,
  `vault_locked`, `weak_passphrase`.
- `From` impls for 12 enums (`CryptoError`, `SessionError`, `NetworkError`,
  `ProtocolError`, `StorageError`, `StunError`, `PortMapError`,
  `ConnectionError`, `DhtError`, `RelayError`, `TorError`, `IdentityError`),
  generated from the real variant lists — **91 variants mapped**, each an
  exhaustive `match &e`, so adding a variant anywhere is a compile error here.
- `From<String>` and `From<&str>` default to `code: "invalid_input"`.
  **This is a placeholder, not a decision** — see §4.4.
- The 14 original `thiserror` enums are untouched and keep their own variants;
  `message` carries the full rendered chain, so nothing is lost.

### 4.2 Conversion progress

- 108 command signatures converted `Result<T, String>` → `Result<T, AppError>`.
- 31 `Err("literal".to_string())` → `Err(AppError::invalid("literal"))`.
- 22 `Err(format!(…))` → `Err(AppError::invalid(format!(…)))`.
- 144 `.map_err(|e| format!(…))` → `.map_err(|e| AppError::invalid(format!(…)))`.
- `use crate::error::AppError;` added to all 12 command modules, placed **after**
  the `//!` module doc block (an earlier attempt put it on line 1 and produced 87
  `E0753: expected outer doc comment` errors).
- A separate regex pass also converted helper functions (`decode_peer_key`,
  `finish_and_chain`, `crypto_decrypt_storage`, …).

### 4.3 ⚠️ A transformer bug to be aware of

The bulk `.map_err` rewrite initially emitted
`.map_err(||e| AppError::invalid(|e| format!(…))` — a duplicated closure
parameter and one missing `)`, from an off-by-one slice in the rewriter. 142
sites were repaired. **Grep for `map_err(||e|` to confirm none remain** — it
should return 0.

### 4.4 The 4 remaining compile errors

```
E0308  src/commands/files.rs:562:26
E0277  src/commands/files.rs:899:107   `?` couldn't convert the error to String
E0277  src/commands/files.rs:908:96    `?` couldn't convert the error to String
E0308  src/commands/util.rs:524:5
```

Likely all one root cause: a helper that still returns `Result<_, String>` (its
signature contains `[u8; 32]`, which defeated the conversion regex that excluded
`;`). `files.rs:899/908` are inside `compute_file_hashes`; `util.rs:524` is
`crypto_decrypt_storage`'s AEAD decrypt tail. **Fix by hand — the mechanical
passes are done.**

After those, the remaining work is:
1. Replace the `"CANNOT_REACH"` string sentinel in `connect_family_member`
   (`commands/vault.rs`) with a real `AppError` code.
2. Give the most common error sites meaningful codes instead of the
   `invalid_input` default — the `From<String>` fallback means codes are
   currently untyped prose for most of the 109 commands.
3. Update the frontend (see §4.5) — **not started.**

### 4.5 Frontend work not yet started

`AppError` serialises to an object, so the frontend currently receives
`{code, message}` where it expected a string. Required:

- `src/utils.ts` `errorMessage()` — handle the object shape. It already has a
  `{"message" in e}` branch, so extend it to prefer `e.message` and expose `e.code`.
- Add an `asAppError()` guard in `src/events.ts`, following the existing
  pattern, and re-export the code union so callers can switch on it.
- Find and fix every site doing `"..." + e` or `String(e)` on an invoke
  rejection — these now produce `[object Object]`. `errorMessage(e)` is the
  replacement. `grep -rn '" + e\|String(e)' src/` to enumerate.

**This is the half of the change most likely to break the UI, and it is
untested until it is done.** `tsc` will pass with an object because `invoke<T>`
is unchecked; the visible failure is a toast reading `[object Object]`.

---

## 5. Deliberately NOT done (with reasons)

| Item | Why |
|---|---|
| **Receiver-side per-chunk `seek`/`write_all`** still blocks the async runtime | Correct fix is `temp_file` → `tokio::fs::File`, touching 8 sites in the file-receive path. Unverifiable-ish and the most safety-sensitive path; an unverified rewrite is worse than a documented perf issue. Self-contained follow-up. |
| **Legacy pre-X3DH handshake** still on reconnect / discovery / family paths | No break-in recovery. Real fix needs a stored prekey bundle per peer — a design change, not a patch. |
| **Zero traits in the crate** | Real architectural gap (nothing mockable). Needs a `trait Transport` seam in `dial.rs` and a `trait` over the three stores. Large, and no longer blocked by the verification problem. |
| **No task cancellation / shutdown path** | 38 `tokio::spawn`, 0 retained `JoinHandle`, 0 `CancellationToken`, tray-only process with no `RunEvent::ExitRequested` handler. |
| **ICE priority computed then discarded** | Faithful RFC 8445 §5.1.2.1 priority that never reaches the wire and never orders the dial; 3 of 5 strategies are the same function; 5 divergent `IpAddr` classifiers. |
| **`AppError` wire-contract change** | `used_opk` and `one_time` in the signed transcript and the new error shape should ship with a `PROTOCOL_VERSION` bump. `protocol.rs` currently reads `0x03`. |

---

## 6. How to resume

```bash
export PATH="/nix/store/lfaydgacdyngci7p60s8wwvgdm74fjkx-nodejs-24.19.0/bin:$PATH"

# Rust: harness (all 50 modules) — expect the 4 errors from §4.4
cd /tmp/opencode/tch && ./check.sh && cargo check --offline --lib

# Rust: relay (works standalone)
cd /mnt/hdd/projects/M2M/relay-server && cargo test

# Frontend
cd /mnt/hdd/projects/M2M && ./node_modules/.bin/tsc --noEmit
./node_modules/.bin/vitest run
./node_modules/.bin/eslint src --max-warnings 10

# Confirm no transformer residue
cd /mnt/hdd/projects/M2M && grep -rn 'map_err(||e|' src-tauri/src/ ; echo "expect 0"
cd /mnt/hdd/projects/M2M && grep -rc "Result<.*, String>" src-tauri/src/commands/*.rs
```

### Sanity greps for the completed work

```bash
cd /mnt/hdd/projects/M2M
for m in append_used_opk_to_sign_data ratchet_reset peer_connection \
         our_peer_key_hex our_identity_kp advance_ack_watermark \
         bind_udp_for_external_query TorUdpUnsupported punch_connect_only \
         lan_dht_seeds apply_connection_pragmas validate_passphrase; do
  printf "%-32s %s file(s)\n" "$m" "$(grep -rl "$m" src-tauri/src/ | wc -l)"
done
```

---

## 7. Live failure modes to remember

1. **`cargo check` on the app fails** on GTK — not a code problem.
2. **The tree auto-commits** — `git diff` shows nothing.
3. **Never symlink into the live source from a harness** — 20 files were lost
   this way.
4. **`rustfmt` and `clippy` do not exist here** — so formatting is unverified.
   Run `cargo fmt` before committing anywhere real.
5. **Rewriting Rust with regex is a trap.** Two separate passes produced
   paren-corrupted output that only the harness caught. Prefer the compiler:
   change signatures, then let `cargo check` find the bodies.
6. `#[tauri::command]` cannot be preserved in the harness — it is an attribute
   macro. `check.sh` strips those lines.
