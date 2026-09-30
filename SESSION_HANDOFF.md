# SESSION HANDOFF — M2M architecture remediation

**Date:** 2026-09-30 (session 4 — maintenance)
**Repo:** `/mnt/hdd/projects/M2M` (branch `main`)
**Goal this session:** close session 3's §5.1 (verify the frontend), §5.2 (the
background task) and §5.3 (tests for the new surface). All three are done and
every suite is green.

**Session 4 in one line:** the storage cap was enforced at one write path of
four and self-destruct expiry ran only while a chat screen was mounted, so the
two promises this app makes about *destroying its own data* were both
substantially unkept. Both now go through one function each, and both were
verified by mutating the live source and watching the tests fail.

---

## 0. Verification — all green, all re-run after the last edit

| Check | Result |
|---|---|
| `./tools/typecheck-harness/check.sh --run` (51 modules) | **0 errors, 0 warnings** |
| `./tools/crypto-probe/sync.sh` | **169 passed** (was 163; +6 new) |
| `tsc --noEmit` | **clean** |
| `pnpm test` | **358 passed** (was 329; +29 new) |
| `pnpm lint` | **0 errors, 10 warnings** — budget unchanged at 10 |
| `cargo test` (relay) | 15 passed |
| `cargo clippy` / `cargo fmt` | still not run — neither binary exists here |

The tree auto-commits on its own, so `git diff` shows nothing. Use
`git log -p` or diff against `89cc2dc` (the last session-3 commit).

---

## 0.1 What this session changed

### Backend

1. **`MessageStore::sweep(cap)`** (`storage.rs`) — one pass = elapsed
   self-destruct timers, then the cap. Deliberately in the *store*, not in the
   task, so the crypto-probe can execute it: `maintenance.rs` needs an
   `AppHandle` and is therefore unexecutable, and an unexecutable module is
   where a policy goes to be untested.
2. **`MessageStore::enforce_storage_cap(cap)`** (`storage.rs`) — the store-level
   half; returns `None` when nothing was destroyed, which is what the caller
   uses to decide whether to bother the user.
3. **`maintenance::enforce_cap(app, store, cap)`** — the single entry point
   every write path calls. It takes the cap as a *value* so callers read
   `security_config` before taking the store lock, per the existing lock-order
   rule.
4. **`maintenance::spawn` + `sweep_once`** — spawned from `lib.rs` `setup`,
   after the config restore, on a 15-minute `tokio::time::interval` with
   `MissedTickBehavior::Delay`. First tick fires immediately; a tick that finds
   no store is the normal pre-first-message state, not an error.
5. **Expiry is also enforced at `MessageStore::open`.** This is the load-bearing
   part for the app-closed case, and see §4.1 for why the first attempt at
   testing it proved nothing.
6. **All four write paths now enforce the cap** — inbound text, inbound group
   frames, outbound `send_message`, outbound `send_message_with_timer`,
   `send_group_message`. Only inbound text did before.

### Frontend

7. The two `setInterval`s in `ChatView` (10s and 60s, both calling
   `cleanup_expired_messages`) are replaced by one mount-time call.
8. `evictionNoticeText()` extracted to `src/utils.ts` so the notice text is
   testable without mounting the app.

### A second bug found and fixed while in there

**`send_group_message` never consulted `ephemeral_mode`.** Every outbound group
message was written to `group_messages` regardless, while `SecurityConfig`
documents itself as "RAM-only ephemeral conversations: NO message/reaction/
edit/delete content is ever written to SQLite. Nothing to seize on disk." A
user who enabled ephemeral mode and then discussed something sensitive *in a
group* — the exact case the mode exists for — left ciphertext and wrapped
content keys on disk. `groups.rs` contained zero occurrences of the string
"ephemeral" before this change. It is in scope here because it is the same
write path the cap work was touching, and because the cap's own accounting
would have been reasoning about a store the user believed was empty.

**This is the kind of gap worth hunting for deliberately next session**: any
security flag read in *some* `commands/*.rs` and not others.

---

---

## 1. What the user decided (do not relitigate)

1. **Cap is user-visible and configurable. Default 10 GiB.**
2. **When full, evict oldest-first. Permanent, not a soft-delete tombstone.**
3. **Eviction overrides a conversation's retention policy** — a full disk
   outranks a preference — **but the notification must name the conversation
   whose policy was overridden.**
4. **The cap is pausable / raisable before it takes effect**, so someone
   archiving evidence is not silently destroying it.
5. **There are no existing users**, so no migration path, no first-run
   special case, and no upgrade concern.
6. Deletion is "unrecoverable by all means" — with the honest caveat that it
   **cannot reach a backup taken before the eviction**, which is stated in the
   UI rather than glossed over.

---

## 2. What is built and verified

### 2.1 Byte accounting (`storage.rs`)

- `storage_stats` table (one row) + `idx_messages_oldest` index on
  `messages(timestamp)`.
- Per-row size is **exact**, not estimated:
  `LENGTH(content_encrypted) + LENGTH(content_nonce) + 72`
  (`WRAPPED_CEK_LEN = 24 + 32 + 16`).
- `recompute_stored_bytes()` — re-derives from SQL, covering **both** `messages`
  and `group_messages`. Called at `MessageStore::open()` so a store that already
  holds messages is accounted for from its first launch.
- `stored_bytes()` — O(1) counter read, for the hot path.
- `stored_bytes_verified()` — re-derives before enforcing. **The counter is a
  cache; enforcement must not trust it**, because a counter that drifted low
  overshoots the cap.
- `add_stored_bytes()` / `msg_row_bytes()` helpers.
- Incremented in `store_message_secure` and `store_group_message`; decremented
  in `delete_conversation` and `delete_expired_messages`, each measuring the
  rows about to be freed with a scoped `SUM()` so the arithmetic is obvious
  and the common case never scans the whole table.

### 2.2 Eviction (`storage.rs`)

`evict_to_cap(cap_bytes) -> EvictionReport`, at module scope for `EvictionReport`
(Rust does not allow structs in an `impl` block).

- Oldest-first, `timestamp ASC`, across **1:1 then group**. Group is included
  because it is a separate table — leaving it out makes the cap bypassable by an
  attacker who only sends group traffic.
- Low-water mark at 90% of cap, so it does not re-fire on every subsequent write.
- **Batch size is bounded by what the cap actually requires**, computed from the
  current average row size — not a fixed 200. See §4.2; a fixed batch was a real
  bug.
- `shred_message_keys()` then `wal_checkpoint(TRUNCATE)` then `DELETE` then
  `wal_checkpoint(TRUNCATE)` — the same four-step sequence as
  `delete_conversation`, which was already correct.
- `shredded_keys: AtomicU64` on `MessageStore`, with `shredded_key_count()`.
  This exists purely so the *wiring* is assertable — see §4.3.
- `overrode_retention: Vec<String>` — conversation ids whose `retention_policy`
  is not `none`, deduped, one policy lookup per *distinct* conversation per batch.

### 2.3 The setting (`state.rs`)

`#[serde(default)] storage_cap_bytes: u64` on `SecurityConfig`, plus
`DEFAULT_STORAGE_CAP_BYTES = 10 * 1024^3` and
`effective_storage_cap()` which maps `0` → the default.

**The `0` → default mapping is not cosmetic.** `AppState::new`
(`state.rs:454`) constructs `SecurityConfig::default()` on every launch with no
config file, and `Default` derives `0` for a `u64`. If `0` meant "no limit",
**the cap would never apply to a fresh install.** An explicit opt-out is a large
finite value from the UI, not an absence.

### 2.4 Enforcement on the inbound path (`commands/network.rs`)

In `handle_incoming_text`, before `store_message_secure`. `security_config` is
read **once, before the store lock is taken** — reading it inside the
`message_store` scope would nest `security_config` under `message_store`, a
pair with no documented order, which is how the two existing deadlocks were
shaped. `emit_storage_evicted()` fires the event.

### 2.5 Command + event + frontend

- `commands::chat::get_storage_usage` → `StorageUsage { used_bytes, cap_bytes }`,
  registered in `generate_handler!`. A closed store reports `0`, not an error —
  otherwise the settings row would be blank on first run.
- `m2m://storage-evicted` emitted with
  `{ messages_evicted, group_messages_evicted, bytes_freed, overrode_retention }`.
- `asStorageEvicted()` in `events.ts`.
- `SecurityConfig.storage_cap_bytes` + `StorageUsage` in `types.ts`.
- `DEFAULT_SECURITY_CONFIG` in `SettingsContext.tsx`, replacing **9**
  hand-written copies of the default object.
- `handleStorageCapChange` + `storageUsage` + `refreshStorageUsage` on the
  settings context.
- `SettingsView`: a usage row and a cap `<select>` (1/5/10/25/100 GB,
  Unlimited), with the "cannot be recovered, including from backups taken
  beforehand" caveat in the hint text.
- `App.tsx`: a listener that turns the event into a **dismissible standing
  banner**, not a toast — a toast disappears, and the user needs to understand
  that history is gone and that raising the cap is how to stop it recurring.

---

## 3. Tests — 163 passing, all mutations verified

10 new tests in `storage.rs`, **executed** via `./tools/crypto-probe/sync.sh`
(see §6 for how that harness was extended this session):

- `test_stored_bytes_counts_and_releases`
- `test_stored_bytes_ignores_duplicate_inserts`
- `test_stored_bytes_verified_recovers_from_drift`
- `test_evict_is_oldest_first`
- `test_evict_is_permanent_not_soft_delete`
- `test_evict_reports_overridden_retention`
- `test_evict_includes_group_messages`
- `test_evict_is_a_noop_under_the_cap`
- `test_shred_message_keys_destroys_the_cek`
- `test_evict_shreds_every_evicted_message`

Five mutations were applied to the live source and each was confirmed caught:

| Mutation | Caught by |
|---|---|
| fixed 200-row batch instead of need-based | `test_evict_is_oldest_first` |
| `ORDER BY timestamp DESC` (newest first) | `test_evict_is_oldest_first` |
| **skip `shred_message_keys` in `evict_to_cap`** | `test_evict_shreds_every_evicted_message` |
| shred zeroes `content_nonce` instead of `content_key_wrapped` | `test_shred_message_keys_destroys_the_cek` |
| `stored_bytes_verified` returns the cache | `test_stored_bytes_verified_recovers_from_drift` |

**The third one initially was not caught**, which is the most important thing to
know from this session: eviction shreds and then deletes, so after a pass there
is nothing left to inspect. A test that only called `shred_message_keys`
directly passed unchanged with the call removed from `evict_to_cap`. The
`AtomicU64` counter exists specifically to close that hole. If you touch the
eviction path, re-run the mutations — the tests are only as good as that check.

---

## 4. Three real bugs the tests caught in my own new code

Worth recording, because the pattern is what made the tests worth writing.

**4.1 `conn.changes()` read after the wrong statement.** I read it after the
`UPDATE conversations`, so it reported *that* update's row count (always 1)
instead of the insert's. Every ignored duplicate inflated the byte counter — a
peer looping one frame id would have tripped the cap without adding anything.
Fixed by capturing the count immediately after the `INSERT`.

**4.2 Evicting far more than the cap required.** The batch was a fixed 200 rows
even when only 4 needed to go, so a 10-message store had all 10 destroyed to
satisfy a cap that required ~6. Fixed by computing the batch size from the
needed byte delta over the current average row size, clamped to 200.

**4.3 `group_messages_evicted` was never incremented** — the first draft of
`evict_to_cap` only looped over `messages` while declaring a group counter. The
field was dead, which would have made the group path silently untested. Rewrote
the loop to drain 1:1 first, then group, with a break when both are empty.

---

## 5. What remains

### 5.1 Verify the frontend — do this first

```bash
export PATH="/nix/store/lfaydgacdyngci7p60s8wwvgdm74fjkx-nodejs-24.19.0/bin:$PATH"
cd /mnt/hdd/projects/M2M
./node_modules/.bin/tsc --noEmit
./node_modules/.bin/vitest run
./node_modules/.bin/eslint src --max-warnings 10
```

Three known-risky places, all guessable without running anything:

1. **`SettingsView.test.tsx`** — the `useSettings` mock was extended with
   `handleStorageCapChange`, `refreshStorageUsage` and `storageUsage`. All 22
   tests in that file were failing with `refreshStorageUsage is not a function`
   before the mock was fixed. If the mock is incomplete the *whole view* dies,
   because the call is in a mount effect.
2. **`App.tsx`** — `refreshStorageUsage` is invoked from a `useEffect` in
   `SettingsView`; any other test that mounts it needs the same mock field.
   `SecurityBanner` also gained an optional `onDismiss`, so any test asserting
   its exact markup may need updating.
3. **eslint budget** — `SettingsView` gained a `useEffect` that calls
   `void refreshStorageUsage()`. That is an async load, not derivable state, so
   it *should* land in the existing 10 `set-state-in-effect` budget — but if it
   pushes the count to 11, restructure (move the reset into the handler) rather
   than raising the pinned number. See `CLAUDE.md`.

### 5.2 Step 3 — the background task (NOT STARTED)

This is the substantive remaining work. Two triggers in one task:

- **Periodic**, which also relocates `delete_expired_messages` off the
  `ChatView` `setInterval`. **Right now expiry only runs while the chat screen
  is mounted**, so "auto-delete after 24h" silently does nothing if the app is
  closed. That is a broken promise independent of any attacker and is the single
  highest-value thing left in this feature.
- **Immediate on crossing the cap**, so a wait for the timer cannot be used to
  fill the disk.

Implementation notes:
- Spawn from `lib.rs` `setup`, which already does `blocking_write` on the config.
- **Must tolerate the store not being open.** `ensure_message_store` is lazy —
  called from ~5 command sites, never at startup. So the task has to handle
  `None` gracefully on every tick.
- Read the cap from `security_config` **outside** any store lock, per §2.4.
- Do not hold the store lock across the whole sweep; `evict_to_cap` is already
  batched and checkpointed, so a tick should call it once and let it finish.

### 5.3 Frontend test for the new surface

None written yet. Minimum valuable set:
- `asStorageEvicted` guard: accepts the real shape; rejects a non-array
  `overrode_retention` (note `asArray` coerces to `[]`, which is why the guard
  uses `Array.isArray` directly — a regression here would silently drop the
  conversation list rather than rejecting).
- The banner renders the eviction message and the dismiss button clears it.
- The cap `<select>` persists through `set_security_config`.

---

## 6. Tooling added this session

**`tools/crypto-probe/`** now also runs `storage.rs`, so the storage-cap tests
actually execute. It previously covered only crypto/group/protocol/secure_key
(113 tests → 163).

To do that it needed two stubs, both in `tools/crypto-probe/src/`:
- `commands.rs` — `ChatMessage` plus mirrors of the two AEAD helpers in
  `commands/util.rs`. `storage.rs` reaches for them only on the *identity* path,
  never the message path, so the cap tests do not depend on them.
- `error.rs` — a two-constructor `AppError`.

**Known limit, already documented in `commands.rs`:** the two AEAD helpers are a
faithful *reimplementation*, not the original, because the real ones return
`AppError` which maps all twelve backend error enums. A change to the real
helper would not be caught by this probe. Same trade as
`tools/typecheck-harness/tauri_stub`: a stub that lets real code be *executed*
beats no execution, provided the limit is written down.

Also new deps in `tools/crypto-probe/Cargo.toml`: `rusqlite`, `chrono`, `uuid`.

---

## 7. Everything from session 2 is still true

The four IP-leakage fixes, the ratchet in-flight fix, the group (key,nonce)
reuse fix, the roster cap, the false-safety UI class, and the four lock fixes
are all in. Two stale `CLAUDE.md` claims were corrected. Session 2's notes are
below this section and remain accurate.

**Do not raise the two Rust suites to 0 warnings with `cargo fmt`/`cargo clippy`
believing they pass — neither binary is installed in this environment.**

---

---

# ═══ Session 2 archive ═══
# Everything below is the previous session's record, kept because it documents
# the four-agent audit and the Tier 1 fixes. §0.1 is the item-by-item list of
# what session 2 fixed and verified; §5 lists what it deliberately left open.

## 0. Verification (all green as of this session)

> ⚠️ **Superseded.** These numbers were true at the end of session 2. The
> storage-cap work in session 3 (above) changed `types.ts`,
> `SettingsContext.tsx`, `SettingsView.tsx`, `App.tsx` and
> `SettingsView.test.tsx` **without a completed test run** — see §0 at the top
> of this file. The backend numbers below still hold; the `pnpm test` count of
> 329 is now unverified.

| Check | Result |
|---|---|
| `./tools/typecheck-harness/check.sh --run` (all 50 modules) | **0 errors, 0 warnings** |
| `cargo test` (crypto + group + protocol + secure_key) | **113 passed** |
| `cargo test` (relay-server) | **15 passed** |
| `tsc --noEmit` | **clean** |
| `pnpm test` / vitest | **329 passed** |
| `pnpm lint` / eslint | **0 errors, 10 warnings** (at the pinned budget) |

**The Rust suites are now actually *executed*, not just compiled.** `crypto.rs`,
`group.rs`, `protocol.rs` and `secure_key.rs` have no GTK dependency, so
`tools/crypto-probe/sync.sh` copies those four files into a standalone crate and
**runs** their `#[cfg(test)]` modules — the only way to execute a Rust test in
this environment.

```bash
./tools/crypto-probe/sync.sh        # 113 passed
```

It re-copies from `src-tauri/src/` on every run, so it cannot pass against a
stale copy — verified by mutating the live `crypto.rs` and watching
`test_in_flight_message_survives_dh_ratchet` fail. **Run it before claiming any
crypto change works.** Mutation-verified in this session for: the ratchet
staging, the group one-shot guard, and the group membership check.

**Two documented claims were false. Both are now corrected in `CLAUDE.md`:**

- `cargo test --all-targets` → "378 passed" was **never observed and was
  wrong**. Two `HandshakeInit` literals in `protocol.rs`'s own test module were
  missing the `one_time` field added the previous session, so the test binary
  **did not compile at all**. Fixed — the suite now builds and runs.
- `test_in_flight_message_survives_dh_ratchet` was recorded as fixed and
  **could not have passed**: `receive_tentative` reset the counter at the
  ratchet *before* staging the superseded chain's pending key, so the key was
  never in the cache. The `retain(|num| num > superseded_upto)` "fix" was a
  no-op by construction, since every cached key is `<=` that value. Fixed with
  an epoch-keyed cache, and **mutation-verified**.

**Also corrected:** my previous session claimed "all 45 `[object Object]` sites
fixed". One was missed — `src/views/ChatView.tsx:574` uses `+ err`, and my grep
used a word boundary after `e` that cannot match `err`. The lesson: a residue
grep is only as good as the pattern, so `grep -rnE '\+ *(e|err|error|ex)\b'`
is the one to use.

---

## 0.1 What this session fixed

**Privacy — the product's premise (4 critical, all verified before and after):**

| Fix | Where |
|---|---|
| `HandshakeInit`/`HandshakeResponse` published LAN IP, global IPv6 and public IP **in a plaintext frame**, unfiltered by Tor, on 4 call sites | `dial::filter_advertised_candidates()`; applied at `network.rs` ×2, `discovery.rs`, `vault.rs` |
| STUN resolved the server hostname with the system resolver **27 lines before** the Tor guard | `stun.rs::query_single_server` |
| An inbound handshake from any stranger spawned a STUN refresh when the candidate cache was empty — remotely forcing DNS + UDP from the real address | `network.rs::complete_inbound_connection` |
| LAN discovery broadcast the listening port and a rotating token to every local host with no Tor guard | `commands/discovery.rs::set_discovery_config` |

**Crypto:**

- **Ratchet lost every message in flight across a DH ratchet** (~1 in 100
  sends). `skipped_keys` is now keyed by `(ratchet_epoch, message_number)`, and
  the superseded chain's next `RATCHET_INFLIGHT_WINDOW` (64) keys are staged at
  the ratchet. Epoch keying is required, not cosmetic: the counter resets to
  zero, so an old-chain and a new-chain message share a number and a bare `u64`
  key made them collide.
- **Group sender-key chain could be rewound ⇒ (key, nonce) reuse.** A bundle is
  a repeatable statement; re-sending the original rebuilt the victim's chain at
  position 0, and anyone holding the key could then encrypt under a consumed
  (key, nonce) pair — which leaks the plaintext XOR and the Poly1305 one-time
  key, and permits forgery. `handle_sender_key` now enforces **membership** and
  **one-shot acceptance** (cleared on removal, so a genuine re-join still works).
- **Group roster cap was bypassable**: `if roster.len() > 31` sat *below* the
  `if let Some(existing)` early return, so an invite for a group we were already
  in took the uncapped path — ~260k members, ~260k SQLite inserts, from one
  512 KiB frame, at 30 frames/s. Cap moved above every branch, and
  `GroupCreate`'s O(n²) `Vec::contains` dedup is now a `HashSet`.

**The false-safety class (this app's most distinctive failure mode):**

- `m2m://vault-locked` was **never emitted**, so idle-lock and "Lock Now"
  zeroized the Rust keys while the webview still showed every decrypted
  message — and the button toasted "Vault locked" as a success. The `AppContext`
  handler was always correct; nothing ever fired it.
- `m2m://security-error` was emitted and had no listener. A protection that
  failed to apply was silent.
- `handleOpenChat` hard-coded `peer_verified: true` → a green Verified badge on
  every conversation opened from the Hub, and the verify button hidden. Now
  asks `get_connection_state` for the real value.
- **Cross-conversation contamination**: `m2m://message` appended to whatever
  conversation was open, so an informant's message appeared in a source's
  transcript and replies went to the source. Same bug in `GroupChatView` (the
  `group_id` was destructured and discarded). Both fixed + regression test.
- `ChatView.submit` had an empty `catch` — a failed send was indistinguishable
  from a delivered one.
- Retention / mute / reactions used `catch {}` on optimistic updates. Retention
  was the worst: "Auto-Delete After 24h" that silently did nothing means the
  messages are on disk forever. All now roll back and say so.
- Clipboard writes were unawaited, so a one-time invite that never left the app
  showed ✓.
- `SetupView` `catch {}` → an unrecoverable startup hang with no error or retry.
- `panic_wipe` failure was `console.error` only — the user who just pressed the
  emergency hotkey mid-incident saw nothing and would assume they were safe.
- `ChatView.tsx:574` — the one site I missed last session.

**Concurrency:**

- `state::connection_state` held the global `connections` **read** guard across
  `conn.lock().await` — write-preferring, so one slow peer froze every
  `disconnect_peer`, heartbeat teardown and new-connection insert. Now delegates
  to `peer_state_snapshot`.
- `disconnect_peer` held the **write** guard across a socket send (up to 10 s).
  Now removes from the map first, then sends.
- `refresh_stun` held `stun_config` across discovery then wanted
  `candidates.write()`; `collect_network_diagnostics` did the inverse. A
  confirmed 2-cycle, both sides now snapshot and release.
- `load_group_messages` called `our_peer_key_hex()` (which reads `identity`)
  while holding `storage_key` — the `storage_key ⇄ identity` cycle that the
  snapshot helper was written to prevent. Hoisted above the locks.

**Four more `#[expect(dead_code)]` attributes removed as stale**, and
`protocol.rs`'s test module fixed so the suite compiles.

### 0.2 Still open (ranked)

See §5. The next ones are the LAN-discovery blocking `recv_from` inside
`tokio::spawn` (pins a worker thread for the life of the feature), unbounded
disk growth (rate limits cap rate, not total), zero file-permission hardening,
temp files orphaned forever, no shutdown path, and the
`connect_family_member` / `connect_discovered_peer` `[0u8; 32]` sentinel that
makes both features non-functional while still leaking the plaintext
`HandshakeInit` first.

---

## 1. Status: the error taxonomy is DONE

The previous session's §4 ("in progress") is closed. All of it is implemented,
typechecked and tested.

| Check | Result |
|---|---|
| `tools/typecheck-harness/check.sh --run` (all 50 modules) | **0 errors, 0 warnings** |
| `cargo test` (relay-server) | **15 passed** |
| `tsc --noEmit` | **clean** |
| `pnpm test` / vitest | **328 passed** (was 313; +15 new) |
| `pnpm lint` / eslint | **0 errors, 10 warnings** (at the pinned budget) |

### 1.1 What was left broken, and is now fixed

The 4 compile errors the previous session documented, plus the whole frontend
half it never started:

- **`files.rs`** — `compute_file_hashes` still returned `Result<_, String>` after
  its body had been converted to `AppError`; its callers then could not `?` it.
- **`util.rs`** — `crypto_encrypt_storage` / `crypto_decrypt_storage` had
  `map_err(|_| "…".to_string())` tails that flattened an already-typed error.
  One of them only typechecked *because* of the `From<String>` placeholder, so
  it was a latent duplicate of the reported bug.
- **`files.rs:556`** — an `AppError` was being passed to `update_state`, which
  wants `Option<&str>`.
- **`"CANNOT_REACH"`** — the last string sentinel is gone. It is now the
  `family.unreachable` code, and the frontend branches on the code instead of
  substring-matching `String(e)`.
- **The frontend** — 45 sites did `"…" + e` or `String(e)` on an `invoke`
  rejection. Every command failure would have rendered as `[object Object]`.
  All 45 now go through `errorMessage()`.

### 1.2 The frontend work, in detail

This was the half the previous session flagged as "most likely to break the UI,
and untested until it is done". It is now tested.

- **`src/events.ts`** — new `asAppError()` guard, plus an `AppErrorCode` type.
  The code is deliberately *not* a closed union: the backend defines ~91 codes
  and adds more without a version bump, so a closed union would reject new
  codes at runtime. Known codes are named for autocomplete and the
  `(string & {})` arm keeps everything else assignable.
- **`src/utils.ts`** — `errorMessage()` already had a `"message" in e` branch,
  so it needed no behaviour change; its doc comment was rewritten to describe
  the new rejection shape.
- **`asTransferErrorEvent`** — `error` is *kept as a string*. Emitting the
  serialised `AppError` would fail `isDisplayText`, the guard would return
  `null`, and the whole event would be dropped: the user would watch a transfer
  fail with no toast at all. The code rides alongside as `error_code`.
- **`FamilyTab.tsx`** — the `CANNOT_REACH` check had a second bug: it
  *swallowed every other failure silently*. Connecting to an offline family
  member showed the user nothing. There is now an `else` branch with a toast.

### 1.3 Tests added (15, and one of them is mutation-verified)

- `asAppError` — 6 cases: accepts unknown codes, rejects strings, bounds and
  control-character-checks both fields.
- `errorMessage` — 5 cases, including an explicit
  `expect(errorMessage(e)).not.toBe("[object Object]")`.
- `asTransferErrorEvent` — 3 cases, including one that pins the
  "rejected payload drops the toast" regression.
- `GroupChatView` — rejects `create_group` with a real `{code, message}` and
  asserts the toast shows the message.

**Mutation-verified:** reverting `GroupChatView` to `"…" + e` makes that last
test fail. Without that check it is easy to write a test that passes for the
wrong reason.

---

## 2. Read this first

### 2.1 The harness is now IN THE REPO

The previous harness lived at `/tmp/opencode/tch` and was **wiped**, which cost
a large part of this session rebuilding it. It now lives in the repo:

```bash
cd /mnt/hdd/projects/M2M
./tools/typecheck-harness/check.sh --run
```

`build/Cargo.toml` is *generated* from `src-tauri/Cargo.toml` on every run
(minus the four GTK-coupled crates), so there is no second dependency list to
drift. `build/` is gitignored; `tauri_stub/` and `check.sh` are the only source
of truth. **`tools/typecheck-harness/README.md` documents its three known
limits — read it before trusting a green result.**

### 2.2 The app's Rust tests still have never run

`cargo check` / `cargo test` / `cargo clippy` cannot build the Tauri app here —
the GTK *development* packages are absent. The 378 backend tests still need a
GTK-capable machine. Anything claimed "verified" below means "verified through
the harness", not "verified by the app's own test suite."

`rustfmt` and `clippy` are **not installed**, so formatting is unverified. Run
`cargo fmt` before committing anywhere real.

### 2.3 The tree auto-commits

An external process commits every few seconds (`git status` and `git diff` are
useless for seeing your own work). Use `git log -p` or take a backup copy
before a bulk edit.

### 2.4 An external process auto-commits the working tree

Commits appear every few seconds (`v3.6.1138` and counting). Consequences:

- `git status` and `git diff` are **useless** for seeing my own work — the tree
  is always clean because everything is already committed.
- `git checkout <file>` restores from HEAD, which *does* contain my changes
  (the auto-committer swept them), but this is a trap worth avoiding.
- To inspect what I changed, use the marker greps in §2 and `git log -p`.

---

## 3. Work completed and verified

### 3.1 Crypto — real defects

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

### 3.2 Dead subsystems that were advertised as working

- **Relay keepalive** — client never sent `0x03`, server never refreshed on it, so
  registrations died at 5 min while `connected: true`. Fixed both sides;
  `test_keepalive_refreshes_idle_timer` added (relay 15/15).
- **Hole punching** — responder bound the already-occupied listener port →
  `EADDRINUSE`, `Role::Responder` unreachable. Removed as connect-only with an
  explanation; a coordinated simultaneous-open is a protocol change, not a patch.
- **DHT** — announced to an empty bootstrap list forever. Now seeds from LAN
  peers, drops `#![allow(dead_code)]`, and warns loudly when it has nothing to
  gossip to.
- **Relay rejected `X3DHHandshakeInit`** outright, so X3DH peers couldn't use the
  path that most needs it.

### 3.3 Concurrency

- **60 sites** held the global `connections` read guard across `.await`. Added
  `AppState::peer_connection()` and converted all of them. One of these
  (sync-resend) could pin the lock ~5.5 hours from a single 20-byte frame.
- **Two deadlock cycles** removed structurally:
  `identity`⇄`group_manager` (via `our_peer_key_hex()` / `our_identity_kp()`) and
  `storage_key`⇄`message_store` (order rule documented in `commands/util.rs`).
- **Remote vault-lock DoS** — `identity.read()` was held across an
  unauthenticated handshake; a trickling peer could stop the user locking their vault.
- `ensure_message_store` / `ensure_transfer_store` no longer take the global mutex
  on the fast path.
- `group_manager` write lock no longer held across a SQLite open.

### 3.4 Privacy / SSRF

- **UDP chokepoint** — `dial::bind_udp_for_external_query()`. STUN / PCP /
  NAT-PMP / SSDP had no guard, so Tor leaked the real IP from 5 production paths.
- **UPnP hop-2 SSRF** — the `controlURL` from the device description was POSTed
  unvalidated; the cloud-metadata deny-list was bypassable.
- **STUN server list** — `contains(':')` accepted `"a:b"`; now capped, parsed, bounded.

### 3.5 Structural

`relay.rs`'s 145-line fork of `handle_incoming_connection` is gone. Both
transports now call the single `commands::network::complete_inbound_connection`.
That fork had already drifted (live STUN on an unauthenticated socket, identity
held across the handshake, no X3DH dispatch).

### 3.6 File transfer (second batch)

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

### 3.7 Frontend (beyond the error work in §1.2)

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

### 3.8 Styling

Six tokens used in CSS were never overridden for light theme and are
white-on-white in it: `--color-bg-tertiary`, `--color-bg-chip` (mute/read chips
invisible), `--edge-light`, `--shadow-bubble-received` (received bubbles had no
edge), plus `--shadow-inner` and `--color-bg-modal-backdrop`.
`--color-danger` and `--color-warning` were **both `#d97706`** — an error toast
and a warning toast were the same colour.

### 3.9 Cleanup

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

### 3.10 NEW this session — error codes beyond the mechanical conversion

The mechanical pass left every code as `invalid_input`, because the regex-based
rewrite could not know what any given site meant. The highest-frequency sites
are now coded:

| Change | Sites | Why |
|---|---|---|
| `message store init: {e}` → `storage` | 11 | SQLite open failed. **The single most common error string in the command layer** and it claimed the caller sent bad input |
| `serialization failed/error: {e}`, `serialize reaction/sender key` → `serialization` | 16 | Almost always a bug, not a bad request — these types are `Serialize` by construction |
| `db error`, `key store error`, `data dir error`, `transfer store *`, `list family`, `failed to store/mark/reconstruct identity`, `delete failed`, `failed to encrypt identity` → `storage` | 24 | All persistence-layer failures |
| `vault must be unlocked …` → `vault_locked` | 2 | Actionable: the UI can offer to unlock, which is the whole reason that constructor exists |
| air-gap / admin-only / Tor-blocks-STUN → `blocked` | 3 | Policy refusal, not bad input — the remedy is "change a setting" |
| `not listening`, reconnection exhausted → `not_connected` | 2 | Not a failure to retry, and not a bad request |

**The `map_err` calls that were destroying typed errors.** This was the most
interesting find. The mechanical pass wrapped *everything*, including calls whose
inner function already returned a properly-coded `AppError` or a typed enum:

- `util::crypto_encrypt_storage(...)` returns `AppError` with
  `storage.encryption_failed`. The wrapper overwrote the code with
  `invalid_input` *and* produced the user-facing message
  `"encryption failed: encryption failed"`. Four sites. Now just `?`.
- `handshake_as_initiator_x3dh` / `handshake_as_initiator` return `SessionError`,
  whose `From` maps every variant to a precise code (`session.replay_detected`,
  `session.protocol`, …). The wrapper flattened all of that. Four sites. Now `?`.
- `IdentityKeypair::generate()` returns `CryptoError`. Two sites. Now `?`.
- `group::encrypt_message` genuinely returns `String`, so it keeps a `map_err` —
  now with `crypto.encryption_failed`.

Rule worth remembering: **only `map_err` when the inner error is actually
untyped.** Wrapping a typed error discards the variant.

### 3.11 NEW this session — dead code the compiler found

With `generate_handler!` faithfully referencing every command (see §4.2), the
harness reports exactly 3 warnings, and the app compiles with **zero** warnings:

- `relay.rs` — 7 dead imports, left behind when the 145-line fork was deleted.
- `port_mapping.rs`, `stun.rs` — `use tokio::net::UdpSocket`, dead since all 8
  call sites moved to `dial::bind_udp_for_external_query()`.
- 5 stale `#[expect(dead_code, …)]` whose items are now genuinely live — the
  `#[expect]` form of the stale `#[allow]`s the previous session already removed
  once. `crypto.rs:35` was worse than stale: it was an **orphaned doc comment**
  for a const that no longer exists.
- `files.rs` — one redundant `mut`.
- `hole_punch.rs` — `Role::Responder` and `StrategyResult::role` are unconstructible
  / write-only, a real consequence of the connect-only change. Annotated with
  accurate `#[expect]` reasons rather than deleted, because removing a variant
  from a protocol-shaped enum is a design call, not a cleanup.

---

## 4. The verification harness

Location: **`tools/typecheck-harness/`** (in the repo, see §2.1). Full
documentation of its limits is in `tools/typecheck-harness/README.md`; the two
that matter most:

### 4.1 ⚠️ Never symlink the live sources into a harness

A previous harness (`/tmp/opencode/tc`, rlib-based) created **symlinks** from
`src/*.rs` to the live files. A `sed ... >` redirect then wrote *through* the
symlinks and truncated every `src-tauri/src/*.rs` to 0 bytes. All 20 were
restored from git (newest non-empty blob per file), and all changes verified
intact, but this is a real hazard. `check.sh` copies real files and refuses to
run if it finds a symlink under `src-tauri/src`.

### 4.2 The stub had to be made faithful — twice

Both times, a wrong stub produced a *wrong answer*, which is worse than no
harness:

1. **Dead-code analysis.** `generate_handler!` originally expanded to `()`. That
   made every `#[tauri::command]` nothing calls from Rust look dead — and
   `sync.rs`'s five functions were about to be deleted on that basis, when they
   are registered and reachable. The stub now expands to `let _ = <path>;` per
   command, which reproduces the use the real macro creates.
2. **`Manager` vs inherent methods.** The stub originally gave `AppHandle`
   inherent `state` / `try_state` / `get_webview_window`. That made
   `use tauri::Manager` look unnecessary in `window_security.rs` and `lib.rs`
   — code that would not compile against the real crate. Verified against
   tauri **2.11.4** source: `state`/`try_state`/`get_webview_window` are
   `Manager` methods (`src/lib.rs:729, 744, 576`); `exit` is **inherent**
   (`src/app.rs:574`). The stub now matches.

**When upgrading `tauri`, re-verify these against the real source rather than
trusting the stub.**

### 4.3 What the harness earned its keep on

Building it (and tightening it) found **6 real bugs in the previous session's own
refactors** plus the 4 outstanding compile errors — all of which would
otherwise have shipped broken.

---

## 5. Deliberately NOT done (with reasons)

| Item | Why |
|---|---|
| **Receiver-side per-chunk `seek`/`write_all`** still blocks the async runtime | Correct fix is `temp_file` → `tokio::fs::File`, touching 8 sites in the file-receive path. Unverifiable-ish and the most safety-sensitive path; an unverified rewrite is worse than a documented perf issue. Self-contained follow-up. |
| **Legacy pre-X3DH handshake** still on reconnect / discovery / family paths | No break-in recovery. Real fix needs a stored prekey bundle per peer — a design change, not a patch. |
| **Zero traits in the crate** | Real architectural gap (nothing mockable). Needs a `trait Transport` seam in `dial.rs` and a `trait` over the three stores. Large, and no longer blocked by the verification problem. |
| **No task cancellation / shutdown path** | 38 `tokio::spawn`, 0 retained `JoinHandle`, 0 `CancellationToken`, tray-only process with no `RunEvent::ExitRequested` handler. |
| **ICE priority computed then discarded** | Faithful RFC 8445 §5.1.2.1 priority that never reaches the wire and never orders the dial; 3 of 5 strategies are the same function; 5 divergent `IpAddr` classifiers. |
| **132 `AppError::invalid` sites left** | These are genuine input validation (bad key length, empty nickname, oversized reaction), where `invalid_input` is the correct code. The high-frequency *mis*codings are fixed; the remainder are low-value churn. |
| **`AppError` wire-contract change** | `used_opk` and `one_time` in the signed transcript and the new error shape should ship with a `PROTOCOL_VERSION` bump. `protocol.rs` currently reads `0x03`. |

---

## 6. How to resume

```bash
export PATH="/nix/store/lfaydgacdyngci7p60s8wwvgdm74fjkx-nodejs-24.19.0/bin:$PATH"

# Rust: harness (all 50 modules) — expect 0 errors, 0 warnings
cd /mnt/hdd/projects/M2M && ./tools/typecheck-harness/check.sh --run

# Rust: relay (works standalone)
cd /mnt/hdd/projects/M2M/relay-server && cargo test

# Frontend
cd /mnt/hdd/projects/M2M && ./node_modules/.bin/tsc --noEmit
./node_modules/.bin/vitest run
./node_modules/.bin/eslint src --max-warnings 10

# Confirm no regression in the error taxonomy
cd /mnt/hdd/projects/M2M
grep -rn 'String(e)\|" *+ *e\b' src/ --include=*.tsx --include=*.ts | grep -v __tests__
grep -rc "Result<.*, String>" src-tauri/src/commands/*.rs | grep -v ':0'   # expect none
grep -rn 'CANNOT_REACH' src/ src-tauri/src/                                # expect none
```

### Sanity greps for the completed work

```bash
cd /mnt/hdd/projects/M2M
for m in append_used_opk_to_sign_data ratchet_reset peer_connection \
         our_peer_key_hex our_identity_kp advance_ack_watermark \
         bind_udp_for_external_query TorUdpUnsupported punch_connect_only \
         lan_dht_seeds apply_connection_pragmas validate_passphrase \
         asAppError errorMessage family.unreachable; do
  printf "%-32s %s file(s)\n" "$m" "$(grep -rl "$m" src-tauri/src/ src/ 2>/dev/null | wc -l)"
done
```

---

## 7. Live failure modes to remember

1. **`cargo check` on the app fails** on GTK — not a code problem. Use
   `./tools/typecheck-harness/check.sh --run`.
2. **The tree auto-commits** — `git diff` shows nothing. Take a backup copy
   before any bulk edit.
3. **Never symlink into the live source from a harness** — 20 files were lost
   this way. `check.sh` guards against it; keep it that way.
4. **`rustfmt` and `clippy` do not exist here** — so formatting is unverified.
   Run `cargo fmt` before committing anywhere real.
5. **Rewriting Rust with regex is a trap.** Two separate passes produced
   paren-corrupted output that only the harness caught. Prefer the compiler:
   change signatures, then let `cargo check` find the bodies.
6. **Do not `map_err` over a typed error.** It discards the variant and, where
   the inner type was already an `AppError`, overwrites a correct code with
   `invalid_input`. See §3.10.
7. `#[tauri::command]` cannot be preserved in the harness — it is an attribute
   macro. `check.sh` strips those lines.
