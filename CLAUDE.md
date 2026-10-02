# M2M — Project Guide

> **Version 5.0.0. Wire protocol `0x03`.** A 5.0.0 client will not complete a
> handshake with a 4.x peer. There is no downgrade path, by design — see
> `RELEASE_NOTES_v5.0.0.md` for why the protocol version had to change.
>
> **That sentence was false until 2026-10-02 and is now true.** It is worth
> recording how, because the failure mode is the whole point of this guide:
>
> - `protocol::validate_version` *accepted* `0x01` after a `tracing::warn!`.
> - The initiator chose its handshake from the **invite**, which is plaintext on
>   the wire before any key exists, so a peer that omitted the prekey bundle
>   could force a 5.0.0 initiator onto the pre-X3DH handshake — no one-time
>   prekey, therefore no forward secrecy.
> - `attempt_reconnect` called the pre-X3DH `handshake_as_initiator` for *every*
>   reconnect, so **every** reconnect was a downgrade. `reconnect.rs`'s own
>   module doc claimed "a fresh X3DH handshake" while the code did the opposite.
> - `RawFrame::version` is parsed and never compared, and `sign_data` does not
>   cover the version byte, so an on-path attacker could rewrite it undetected.
>
> The first two are fixed. **Reconnecting is now refused** rather than performed
> downgraded: a fresh prekey bundle is unobtainable (no prekey-refresh packet
> exists, and the invite's one-time prekey is single-use), so `attempt_reconnect`
> returns an error telling the user to exchange a fresh invite. Restoring the
> feature properly needs a prekey-refresh packet type first.

## Architecture Overview

See `docs/architecture.md` for the full module map. Key modules:

| Module | Purpose |
|--------|---------|
| `src-tauri/src/crypto.rs` | Ed25519, X25519, X3DH, Double Ratchet, HKDF, AEAD |
| `src-tauri/src/session.rs` | Encrypted session lifecycle, send/receive with ratchet |
| `src-tauri/src/protocol.rs` | Wire format: packet types, framing, MessagePack serialization |
| `src-tauri/src/dial.rs` | **The only** outbound TCP path — enforces Tor/no-fallback rules |
| `src-tauri/src/commands/` | Tauri IPC bridge — 8 modules (chat, vault, network, files, discovery, security, relay, settings) |
| `src-tauri/src/storage.rs` | SQLite-backed MessageStore, KeyStore, TransferStore (app-level AEAD) |
| `src-tauri/src/state.rs` | Central AppState with all runtime config + connection state |
| `src-tauri/src/dht.rs` | Custom lightweight Kademlia DHT for peer discovery |
| `src-tauri/src/lan_discovery.rs` | UDP multicast LAN peer discovery |
| `src-tauri/src/stun.rs` | STUN with FINGERPRINT verification, NAT classification |
| `src-tauri/src/port_mapping.rs` | UPnP/NAT-PMP, with SSRF and response-size bounds |
| `src-tauri/src/maintenance.rs` | The one place self-destruct expiry and the storage cap are scheduled |
| `relay-server/src/main.rs` | Self-hosted TURN-style relay |
| `src/events.ts` | Runtime validators for every Tauri event payload |
| `src/i18n/` | Typed translation catalog and locale provider |
| `src/hooks/useNow.ts` | Ticking clock for relative-time rendering |
| `src/styles/tokens.css` | Design tokens — 147 primitives, scales and surfaces |

## What's Implemented

- ✅ X3DH + Double Ratchet (Signal-protocol E2EE)
- ✅ DHT peer discovery + LAN multicast discovery (OFF by default)
- ✅ TURN relay server (self-hosted)
- ✅ Identity export/import + family contacts
- ✅ File transfer with streaming, chunk hashing, ACK/cancel/resume
- ✅ Conversation retention policies (auto-delete/export)
- ✅ Private mode + Tor SOCKS5 support, with a single audited dial path
- ✅ STUN + Happy Eyeballs connection strategies
- ✅ Reactions (0x41), Message Edit (0x42), Message Delete (0x43)
- ✅ Self-destruct timer on messages, enforced on a background timer and at DB open
- ✅ User-visible storage cap (10 GiB default) with permanent oldest-first eviction
- ✅ Read receipts (local)
- ✅ Markdown rendering (bold, italic, code, links)
- ✅ Clipboard auto-clear + screen capture protection + idle vault lock
- ✅ Runtime-validated Tauri event boundary; no `any` in `src/`
- ✅ Typed translation catalog wired at the app root — **English only.**
  Infrastructure, not localisation: `LocaleCode` is `"en"`, there is no language
  picker, and roughly half the catalog is unreachable from any rendered view
  because most call sites are still literals. `HAS_MULTIPLE_LOCALES` is exposed
  from the provider so a picker is not added until a second locale exists. Do not
  describe the app as translated.

## Key Patterns

### Typed encrypted frames
Use `session.send_encrypted_typed(write_half, PacketType::Xxx, &serialized)` for any feature that needs to send structured data over the encrypted session. The handler in `network.rs` receives it and dispatches by `PacketType`.

### All outbound traffic goes through `dial.rs`
Never call `TcpStream::connect` directly, and never `UdpSocket::bind` for a query
that leaves the host. `src-tauri/src/dial.rs` decides between a Tor SOCKS5 proxy
and a direct socket, and refuses a direct connection when Tor is enabled. Routing
one subsystem around it means that subsystem silently leaks the user's real IP. An
architecture test in the Rust sources fails the build if a new raw
`TcpStream::connect` appears.

**UDP is guarded too, and this matters more than it looks.** A STUN Binding Request
carries the sender's source address to a third party *by construction*, and
PCP / NAT-PMP / SSDP talk to the user's router. All of them bypassed the TCP guard,
so enabling Tor did not stop M2M from disclosing the real address. Use
`dial::bind_udp_for_external_query()` for those sockets, not `UdpSocket::bind`.
A datagram to a peer address we have already chosen to talk to needs no guard.

**The guard has to come before DNS, not after it.** `stun.rs` resolved the STUN
server hostname with the system resolver 27 lines *before* the chokepoint, so
the DNS query left from the real address even though the datagram was refused.
The default servers are hostnames, so every call leaked. If you add a code path
that resolves a name and then talks to it, refuse under Tor *first* — resolving
`stun.l.google.com` is already a disclosure.

**A plaintext frame is read by everyone on the path.** `HandshakeInit` /
`HandshakeResponse` are written through `write_frame` before any key exists, so
they are unencrypted by construction — the peer, the Tor exit and every AS in
between can read them. That is where our LAN address, global IPv6 and
STUN-observed public IP were being published, on four call sites, with no Tor
filter, so the Tor circuit was decorative. Any candidate list bound for the wire
goes through `dial::filter_advertised_candidates()`, which under Tor keeps only
relay entries. `state.candidates` keeps the full set for diagnostics; it is not
what gets published.

**A remote peer must never cause the host to talk to a third party.**
`complete_inbound_connection` used to spawn a STUN refresh whenever the
candidate cache was empty, which is the normal state on a fresh install — so any
stranger with a self-signed Ed25519 key could force outbound queries from the
victim's real address. That is the shape of a deanonymisation primitive.

### Nothing the UI shows may be a claim it cannot back
The recurring failure in this codebase is a control that reports success when
the thing it describes did not happen. Every one of these shipped:

- `lock_vault` zeroized the Rust keys but emitted **no event**, so idle-lock and
  "Lock Now" left every decrypted message on screen while the button toasted
  "Vault locked" as a **success**. `m2m://vault-locked` now exists; the handler
  in `AppContext` was always correct and simply never fired.
- `handleOpenChat` hard-coded `peer_verified: true`, putting a green Verified
  badge on every conversation opened from the Hub and hiding the verify button.
  Verification is a user action, never a side effect of opening a view.
- Retention, mute, reactions and favourites used `catch {}` on an optimistic
  update, so a failed write left the UI asserting a policy that was not applied.
  Retention is the worst: "Auto-Delete After 24h" that silently did nothing
  means the messages are on disk forever.
- `m2m://security-error` was emitted when a protection failed to apply, and
  nothing listened.
- `navigator.clipboard.writeText` was never awaited, so the ✓ appeared even when
  the write was refused — and a one-time invite that never left the app looks
  identical to one that did.

When you add a control that changes security state, write the failure path at
the same time as the success path, and ask what the user will believe if the
`invoke` rejects.

### A promise about destroying data cannot depend on which view is mounted
`maintenance.rs` owns the two policies that destroy stored history on a timer,
and it is the **only** place that decides when they run:
`MessageStore::sweep(cap)` = elapsed self-destruct timers, then the storage cap.
`maintenance::enforce_cap(app, store, cap)` is the **only** definition of "the cap
is enforced", and every write path calls it — inbound text, inbound group,
outbound text, outbound group.

Both facts were learned the hard way and both were true in the shipped app:

- Expiry ran from a `setInterval` in `ChatView`. This app hides to the tray, so
  "auto-delete after 24h" and every self-destruct timer did **nothing at all**
  unless the user happened to be sitting in a conversation. `MessageStore::open`
  now destroys elapsed timers as well, because the app-closed case is the one
  that matters.
- The cap was enforced at **one** write site out of four. Outbound sends and both
  group paths wrote to the store with no ceiling, so a cap that most write paths
  bypass is not a cap. This is the same failure shape as the allowlist gate in
  `complete_inbound_connection` below.

The observable is the **disk**, not the read path. `load_messages` already
filters `expires_at <= now`, so a test that asserts an expired message is absent
from `load_messages` proves nothing — the row and its wrapped content key are
still on disk, which is what a seizure gets. Assert on the table.

Adding a write path? Call `maintenance::enforce_cap`. Adding a policy that
destroys data? Add it to `MessageStore::sweep`, so it runs on the timer *and* in
the crypto-probe, and not only when someone remembers to wire it into a view.

### One way to accept an inbound connection
`commands::network::complete_inbound_connection` is the **only** implementation of
"handshake → contact-allowlist gate → build `PeerConnection` → insert → emit → upsert
→ spawn receive loop". Direct TCP and relay both route through it, passing the
already-read handshake frame when they have one.

There were five hand-rolled copies, and they had already diverged: the allowlist
gate was added to the direct and relay paths *after the fact* because it was
bypassable by choosing a different transport, and the same gap reopened on the
discovery and family-contact paths. Forking this routine is what produces the next
such bug. If you need a new inbound transport, call the shared function.

### Lock scoping
Always scope `state.message_store.lock()` narrowly. The `rusqlite::Connection` uses
`RefCell` internally and makes the future `!Send` if held across `.await`.

**Never hold `state.connections` across an `.await`.** It is one global `RwLock`
for every peer, so a read guard held across a socket write (up to the 10s
`NETWORK_TIMEOUT`) blocks every `disconnect_peer`, every heartbeat teardown and
every new-connection insert in the process. Use `state.peer_connection(&key)` —
it clones the `Arc` and releases the guard before returning.

**Lock order:** when a path needs both the storage key and the message store, take
`storage_key` **first**, then `message_store`. `storage_key` is a write-preferring
`RwLock`, so the reverse order is a deadlock cycle. Likewise, snapshot identity
values out via `state.our_peer_key_hex()` / `state.our_identity_kp()` before
touching `group_manager` — never nest the two locks. Those snapshot helpers only
help if you call them *before* the window they exist to avoid:
`commands/groups.rs` called `our_peer_key_hex()` while already holding
`group_manager`, which reintroduced the exact `identity ⇄ group_manager` cycle
the helper was written to break. Same for `candidates ⇄ stun_config`, where
`refresh_stun` held the `stun_config` read guard across the discovery that then
wanted `candidates.write()`.

**A guard the compiler cannot see is still a guard.** `tokio::sync::RwLock` is
write-preferring, so *one* queued writer blocks every subsequent reader. Holding
the map guard across a per-peer `conn.lock()` — even without touching the socket
yourself — freezes the whole connection subsystem for as long as that peer is
slow. `state::connection_state()` did exactly this and the fix is to call
`peer_state_snapshot()`, which is the same function written correctly.

### Borrow checker workaround for send_encrypted_typed
```rust
let PeerConnection { session, write_half, .. } = &mut *conn;
session.send_encrypted_typed(write_half, packet_type, &data).await?;
```
This destructures `conn` first so both borrows are from the destructured fields, not from `conn`.

### Every Tauri event payload is validated
The `listen` boundary is untrusted input. A handler must narrow through a guard
from `src/events.ts` before touching the payload:
```ts
const unlisten = await listen("m2m://transfer-progress", (event) => {
  const p = asTransferProgressEvent(event.payload);
  if (!p) return;               // malformed → drop, never render
  setTransfers((prev) => ({ ...prev, [p.transfer_id]: p }));
});
```
Guards are the security boundary, not a type convenience: they bound string
lengths, reject control characters, and validate key and fingerprint shape.

### Do not type a mock Tauri handler as `any`
Test event handlers receive `unknown` and are typed that way in
`src/__tests__/tauriMock.ts`. Typing them `any` asserts the payload is already
the right shape, which is the opposite of what the validator checks. Use
`DeepPartial<T>` for deliberately partial context mocks.

### Long-lived listeners read fresh values through refs
The 13 chat listeners are registered once. A `useCallback`/effect that closes
over changing state must re-register to see it, and re-registration drops
inbound messages in the gap. Read the changing value through a ref instead
(`tRef`, `activeConversationIdRef`, …), as `ChatContext.tsx` does.

### Adding a new ChatMessage field
Update ALL construction sites:
1. `src-tauri/src/commands/mod.rs` — ChatMessage struct
2. `src-tauri/src/commands/chat.rs` — load_messages, send_message, send_message_with_timer
3. `src-tauri/src/commands/network.rs` — incoming message construction
4. `src-tauri/src/storage.rs` — StoredMessage struct + both query_map closures
5. `src/types.ts` — ChatMessage interface
6. `src/views/ChatView.tsx` — rendering (if visible)
7. `src/events.ts` — the guard must accept the new field

### Privacy-first defaults
All discovery and security features are OFF by default. Users must explicitly enable them. This follows the principle that convenience is the enemy of privacy.

## Verification

Every row below was **run**, not remembered. The date and the command matter
more than the number: a result without them is a claim, and this repo has
shipped enough false claims in its release notes to be worth distrusting by
default.

**Last full run: 2026-10-02, commit `f0f3014` + the working tree.**

The Rust rows were empty until now, and that emptiness was hiding a build
break. See "The Rust side had never been compiled" below.

| Command | Result | Verified how |
|---------|--------|---------------|
| `cargo fmt --all -- --check` | **clean** | run. Was 138 hunks dirty across 25 files before this pass. |
| `cargo check --all-targets` (`src-tauri`) | **clean, 0 warnings** | run. Was **22 errors**. |
| `cargo clippy --all-targets -- -D warnings` (`src-tauri`) | **clean** | run. Was **21 findings**. |
| `tools/crypto-probe` (5 modules, executed) | **194 passed, 0 failed** | run. Was 192/2 — and the 2 were real bugs, see below. |
| `cargo test` (`relay-server`) | **15 passed** | run |
| `cargo clippy --all-targets -- -D warnings` (`relay-server`) | **clean** | run. Was 7 findings. |
| `tsc --noEmit` | clean | run. Was **1 error** (`ChatContext.tsx` `.catch(() => {})` typing). |
| `pnpm test` | **386 passed**, 21 files | run |
| `pnpm lint` | 0 errors, **8** warnings (budget 8) | run. Was 11 — over a budget of 10, i.e. CI-red. |
| `pnpm test:coverage` | 65.14 stmts / 76.63 branch / 56.15 funcs / 65.14 lines | run. Gates raised to 60/70/48/60 to match. |
| `./tools/typecheck-harness/check.sh --run` | **still not run** | needs `bash`; this is a Windows shell. |
| `cargo test --lib` (`src-tauri`) | **cannot run here** | environment, not code — see below. |

### The Rust side had never been compiled

This is the most important finding of the whole pass, and it is only visible
because the linker finally worked.

Installing WinLibs (`BrechtSanders.WinLibs.POSIX.UCRT`) supplied the missing
`gcc`/`ld`, and `cargo +1.89.0-x86_64-pc-windows-gnu check --all-targets`
immediately reported **22 compile errors** in a tree that `CLAUDE.md`,
`README.md` and `RELEASE_NOTES_v5.0.0.md` had all described as building.
`cargo fmt` had been passing the whole time, because rustfmt only *parses*.

Several of these were not cosmetic, and two are worth naming because the
project's own comments asserted the opposite:

- **`crypto.rs` — the receive-chain key was not being scrubbed.** `scrub_and!`
  zeroizes `tent_chain`, but `tent_chain` was declared *after* the macro was
  defined. `macro_rules!` resolves body identifiers at the **definition site**,
  so the name did not resolve and the file did not compile. Deleting the line
  would have "fixed" the build while leaving the zeroization undone — the
  comment right above it says every error exit returned with the live receive
  chain key on the stack, and that was true. The declaration now precedes the
  macro.
- **`storage.rs` — a legacy database could not be opened.** The schema batch
  created `CREATE INDEX ... ON messages(expires_at)` on the grounds that
  `expires_at` is "declared above". But `CREATE TABLE IF NOT EXISTS` is a
  **no-op** when the table already exists, so for any database predating that
  column the index was created against a column that did not exist yet and
  `MessageStore::open` failed outright — *before* `migrate_messages_table` could
  `ALTER TABLE` and add it. Any user upgrading from the pre-crypto-shredding
  schema could not open their messages. The indexes now live only in the
  migration, which already created them idempotently.
- **`commands/network.rs` — the duplicate-transfer guard never compiled.** A
  binding declared without `mut` was assigned in both arms of a match, so the
  "do not re-prompt for a transfer id that already exists" logic had never run.
- **`port_mapping.rs`** — two call sites used a match arm whose guard bound a
  variable the other arm did not, so the UPnP `<service>` parser had never
  compiled; it is now `is_tag_name_end`, shared by both callers.
- **`lan_discovery.rs`** — `set_reuse_port` was called as if it existed. It is
  gated behind socket2's `all` feature **and** only exists in socket2's unix
  backend; Windows has no `SO_REUSEPORT`. LAN discovery is behind a default-off
  flag, which is the only reason this was invisible.
- **`stun.rs` / `port_mapping.rs`** — `is_global_unicast` was called as if it
  were a std method on `IpAddr`/`Ipv4Addr`. It is not; it is a project-local
  helper in `stun.rs`, now `pub(crate)` and used by both, so "may we publish
  this address" has one definition.
- **`session.rs`** — five `HandshakeInit` test literals were missing `one_time`.
  Adding the field alone would have compiled and then *failed*: the responder
  verifies `append_used_opk_to_sign_data(..., one_time)`, so three tests also
  needed the signed transcript updated to match.

The lesson generalises past this pass: **`cargo fmt` is not a build.** A parser
that accepts the file tells you nothing about whether it links.

### `cargo test --lib` still cannot run in this shell, and it is not the code

With the probe green and clippy clean, the full `src-tauri` test binary links
but exits `0xC0000135` (`STATUS_DLL_NOT_FOUND`) then `0xC0000139`
(`STATUS_ENTRYPOINT_NOT_FOUND`). Cause, established with `objdump -p`:

- The binary imports `TaskDialogIndirect` from **`comctl32.dll`**.
- This machine's `system32\comctl32.dll` is **version 5**, which does not export
  it; only the side-by-side v6 does.
- The test harness `.exe` has **no `.rsrc`/manifest**, so it cannot activate
  comctl32 v6 the way the real app binary does. `WebView2Loader.dll` also has to
  be copied into `target/debug/deps/` manually.

That is Tauri GUI plumbing, not application logic — which is precisely the gap
the crypto probe exists to cover. On a Linux CI runner the manifest and the v6
comctl32 are both present, so `cargo test --all-targets` runs there. **Do not
report the full Rust suite as passing on the strength of the probe alone; the
probe covers 5 of ~20 modules.**

### Why the crypto probe is the load-bearing harness

`crypto.rs`, `group.rs`, `protocol.rs`, `secure_key.rs` and `storage.rs` have no
Tauri dependency, so `tools/crypto-probe/sync.sh` copies those **five** files
into a standalone crate and *runs* their `#[cfg(test)]` modules. It re-copies on
every run, so it can never pass against a stale copy. Run it before claiming a
crypto or storage change works.

  **It also cannot see `maintenance.rs`**, which is why the scheduling lives in
  the store (`MessageStore::sweep`) and `maintenance.rs` only decides *when*. An
  unexecutable module is exactly where a policy goes to be untested.

Both harnesses are now wired into `.github/workflows/ci.yml`. They were absent,
so the only Rust verification this project actually trusts was the one thing
never run automatically, while three CI steps ran that cannot pass here.

### Lint budget

The 8 remaining warnings are `set-state-in-effect` plus one
`exhaustive-deps`, all in genuine async-load / reset-on-key-change positions.
The budget in `package.json` is pinned to the current count so it cannot
regress; **if you fix one, lower the number** (it was 11 → 8 during this pass).

The `preserve-manual-memoization` warnings are now gone. They were not merely
advisory: the compiler could not prove `connection?.peer_key_hex` was the same
value as the `connection.peer_key_hex` the handler bodies read, so it skipped
compiling `ChatContext` entirely, leaving all ~30 of its `useCallback`s
unoptimised. Hoisting the key to a `peerKeyHex` local fixed it.

## Design tokens
`src/styles/tokens.css` holds the dark palette; `src/styles/theme.css` overrides
it under `[data-theme="light"]`. **No colour literal may appear outside those two
files**, with the single exception of a `var(--token, <literal>)` fallback used
inline in `src/App.tsx`, which exists so the element still receives a colour if
the stylesheet fails to load — layout, animations and reset all reference `var(--token)`. That rule
exists because the shell chrome (sidebar, right panel, sticky-header scrim, the
shimmer and cursor sheens) previously carried hardcoded dark rgba() values in
theme-agnostic files, which meant a dark sidebar rendered on the light theme.

**This rule is now enforced, not just stated.**
`src/__tests__/designTokens.test.ts` fails the build on a colour literal in any
`.ts`/`.tsx`/`.css` outside the two token files, and on any `var(--token)` whose
definition is missing. Writing the test found **41 further violations** that the
prose rule had been letting through for months: seven `color="white"` in `.tsx`
(white-on-white on the light theme), a runtime `hsl()` in `utils.ts::hashToColor`
that put every avatar outside the palette, and ~30 inline `rgba()` in the
component sheets. Both were fixed rather than grandfathered.

Two named exceptions, both enforced:
- `App.tsx` — the sanctioned `var(--token, <literal>)` fallback.
- `context/ThemeContext.tsx` — `DEFAULT_ACCENT`, which seeds the `--color-accent`
  custom property at runtime and so cannot itself be a token. A separate check
  asserts it is the **only** place `#6366f1` appears, because it used to be
  duplicated in `SettingsView`'s reset handler.

`--focus-ring` must be used for `:focus-visible` outlines rather than a bare
1px-equivalent border, since a single flat ring disappears against both the dark
canvas and the light accent surfaces.

Several tokens are defined but unreferenced (the type, spacing, radius and z
scales deliberately define unused steps; `--color-bg-tooltip` and the
`--color-primary*` aliases are genuinely speculative). That is the intended
shape of a scale — do not prune scale steps to satisfy a usage count.

## Build status
`cargo check` passes clean in `src-tauri/` and `relay-server/`. No known
outstanding compile errors.

### Release signing
`bundle.createUpdaterArtifacts` is on, so `tauri build` finishes the bundles and
then exits non-zero with "A public key has been found, but no private key". That
is expected without `TAURI_SIGNING_PRIVATE_KEY` and is not a build defect — the
`.deb` and `.rpm` are already written at that point. Export the signing key
before the release build.

### Linux packaging
`tauri build` produces the release binary plus **deb** and **rpm** bundles,
both verified. The **AppImage** step does not complete when building from a
Nix store environment, for reasons that are environmental rather than defects
in this project. It requires a conventional (non-Nix) Linux builder.

Four distinct Nix problems had to be worked around to get as far as
linuxdeploy's ELF parser, in `/tmp/opencode/depshell.nix`:

1. `tauri build` aborts with "Can't detect any appindicator library" unless
   `libayatana-appindicator` and `librsvg` are in the build shell.
2. Tauri resolves the tray library with
   `pkg-config --libs-only-L ayatana-appindicator3-0.1` and joins every returned
   `-L` with a space before appending the soname. Nix's `.pc` declares
   `Requires:`, so that query returns the whole transitive chain (12 `-L`
   entries) and the "path" Tauri builds is a single non-existent filename.
   Fixed by shadowing the module with a `.pc` that has no `Requires:`.
3. `linuxdeploy-plugin-gtk.sh` copies `pkg-config --variable=schemasdir
   gio-2.0`, which is derived from a `prefix=` naming an unrealised store path.
   Fixed by rewriting only that one line of `gio-2.0.pc`, leaving `Libs` and
   `Cflags` untouched.
4. linuxdeploy is itself an AppImage and the plugin invokes it without
   Tauri's `--appimage-extract-and-run`, so it needs FUSE. Fixed with
   `APPIMAGE_EXTRACT_AND_RUN=1`.

With all four in place the plugin deploys 163 shared libraries into the AppDir
(287 MB) and then dies with
`linuxdeploy::core::elf_file::ElfFileParseError: Invalid magic bytes in file
header` — linuxdeploy treating a Nix store entry as an ELF file. That is
inside linuxdeploy itself and would require patching it or building on a
conventional distro.


## Versioning
The app version lives in four places that must stay in sync: `package.json`,
`src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and
`relay-server/Cargo.toml`. The wire protocol version is separate and lives in
`src-tauri/src/protocol.rs` as `PROTOCOL_VERSION`. The UI reads the live app
version at runtime via `getVersion()` (`SettingsView.tsx`). Current:
**5.0.0**, protocol **0x03**.
