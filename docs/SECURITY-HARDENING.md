# M2M — Security Hardening Roadmap

> Compiled from a full deep-scan audit (backend, crypto core, frontend, relay server).
> Ratings at time of scan: overall **7.5/10** — strong design, immature implementation.
> All items are actionable; none require redesigning the architecture.

---

## 1. Critical Fixes (fix first — highest ROI)

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 1 | ~~`import_identity` re-seals the private key under `derive_storage_key(&pub_bytes)`~~ **✅ FIXED** — import now seals under Argon2id(passphrase, salt = pubkey), registers a vault account, and marks vault initialized; regression-tested (`commands/vault.rs::tests`, H1) | `commands/vault.rs:758-767` | Imported identity is plaintext-equivalent at rest, permanently |
| 2 | ~~Byte-index string slicing panics on multi-byte UTF-8 boundaries (`&text[..77]`, `&content_str[..80]`)~~ **✅ FIXED** — shared char-boundary-safe `truncate_utf8()` helper (`commands/util.rs`); fixed audit sites plus two more found in `groups.rs:222` and `port_mapping.rs::truncate_safe`; regression-tested | `chat.rs:188`, `network.rs:2024` | Remote crash bug reachable from crafted network input |
| 3 | ~~Double Ratchet advances root/recv chain **before** AEAD verification; garbage frame with fake ratchet key permanently desyncs session~~ **✅ FIXED** — receive is now transactional: all derivations run on tentative locals (`receive_tentative` + `TentativeReceive`), commit only after AEAD success; skipped-key cache consumed only on successful decrypt; regression-tested (3 H3 tests in `crypto.rs`) | `crypto.rs:677-679, 585-599` | Irreversible session DoS by active attacker |
| 4 | ~~Legacy session path: random initial counters never exchanged → sessions fail (or path is dead code kept alive as weak fallback)~~ **✅ FIXED** — counters now start deterministically at zero on both peers; cross-session replay was already impossible (fresh per-handshake ephemeral keys → AEAD fails); in-session ordering via monotonic watermark; removed all test workarounds that forced counters; bidirectional H4 regression test added | `session.rs:79-95, 642-647` | Broken functionality + unnecessary attack surface |
| 5 | ~~Responder accepts any validly-signed stranger, auto-upserts into KeyStore — no contact allowlist gate~~ **✅ FIXED** — opt-in `require_known_contact` security setting (default off per project convention): when enabled, incoming connections from peers absent from the key store (`peers` table) and family list are rejected after signature authentication and BEFORE persistence/receive loop; unit-tested gate policy + storage check; Settings UI toggle added | `commands/network.rs:424-485` | Anyone who finds the port can open sessions/deliver messages |
| 6 | ~~One-time prekeys (OPKs) fully implemented but never used; SPK never rotates~~ **✅ FIXED** — `create_invite` now generates an OPK alongside the SPK (both rotate together per invite); OPK public key embedded in the signed invite payload; initiator includes DH4 and signals `used_opk`; responder applies its OPK secret deterministically (mismatch/stale invites fail cleanly); covered by 3 H6 integration tests | `commands/network.rs:238`, `session.rs:451-454` | Weakened forward secrecy for first messages |

### Secondary fixes
- ~~Pre-auth STUN/candidate gathering in `handle_incoming_connection` = self-inflicted DoS amplification~~ **✅ FIXED** — responder now answers the handshake from the CACHED candidate set; a full STUN refresh only runs post-authentication and only when the cache is empty (`commands/network.rs::handle_incoming_connection`)
- ~~Release per-peer connection mutex before SQLite writes in receive loop~~ **✅ FIXED** — `EncryptedMessage`, `ConversationMeta`, `MessageReaction`, `MessageEdit`, `MessageDelete` arms now decrypt under the per-peer lock only and drop both guards before persistence; slow disk I/O can no longer head-of-line block outbound sends to that peer
- ~~Encrypt transfers.db / family contacts / reactions metadata~~ **✅ FIXED** — sensitive text is now stored as authenticated envelopes (`enc1:nonce_hex:ct_hex`, XChaCha20-Poly1305 under the vault storage key, per-domain AAD): reaction text (`m2m-reaction-v1`), family nicknames + last-known addresses (`m2m-family-v1`), transfer filenames/paths/errors (`m2m-transfer-v1`). Legacy plaintext rows still read (no destructive migration); reaction dedupe/remove match by DECRYPTED text (random nonces defeat SQL equality); undecryptable rows degrade gracefully (skipped / `[encrypted]`) instead of failing queries; export/import pass the key so backups are plaintext while disk stays sealed; tested (`test_*_encrypted_at_rest`)
- ~~Heartbeat timeout documented but never implemented~~ **✅ FIXED** — heartbeat worker polls at half the ack timeout, tracks `last_hb_sent`/`last_hb_ack` per connection (`state.rs::PeerConnection`); an unanswered probe tears the connection down within one timeout window (reconnect info saved + disconnect event emitted); acks recorded in the receive loop
- ~~Handshake signatures don't cover `candidates`~~ **✅ FIXED** — all four handshake paths (legacy + X3DH × initiator + responder) fold a canonical length-prefixed encoding of the candidate list into the signed data via `append_candidates_to_sign_data`; candidate tampering (reconnect-target poisoning) now invalidates the signature; regression-tested
- ~~DHT `parse_node_response` hardcodes IPv4 while announce supports IPv6~~ **✅ FIXED** — NODE_RESPONSE entries now carry an af_tag (4/6) mirroring the announce body format, parsed sequentially so mixed v4/v6 bodies work; legacy fixed-38-byte all-IPv4 responses fall back cleanly; tested (tagged, IPv6, mixed-AF, legacy fallback)
- ~~Split ~1,400-line `spawn_receive_loop`~~ **✅ DONE** — loop is now a ~235-line dispatcher; packet domains extracted verbatim into dedicated handlers (`handle_incoming_text`, `handle_file_transfer_packet`, `handle_heartbeat_frame`, `handle_conversation_meta`, `handle_message_update_frame`, `handle_sync_frame`, `handle_group_frame`). ~~De-duplicate ChatMessage construction sites~~ **✅ DONE** — all 6 construction sites use `ChatMessage::new(...)` + builder setters, so adding a field touches only the constructor

---

## 2. Anti-Surveillance Features (screen recording / keyloggers)

**Honest threat model:** an app can defeat casual/opportunistic spying; nothing user-space defeats kernel-level malware on the same machine.

| Technique | Status | Stops | Can't stop |
|-----------|--------|-------|------------|
| `WDA_EXCLUDEFROMCAPTURE` (Windows) | ✅ implemented (`window_security.rs:57`) | OBS, Zoom-share, most recorders | Kernel capture, HDMI capture cards, phone camera |
| macOS `NSWindow.sharingType = .none` | ✅ implemented — libobjc FFI (`objc_msgSend` typed casts), `setSharingType:NSWindowSharingNone`, restore to readWrite on disable (`window_security.rs::apply_to_window`) | ScreenCaptureKit streams, screenshots, window sharing | Kernel capture, HDMI capture cards, phone camera |
| Linux X11 privacy hint / Wayland isolation | ✅ honest capability reporting — Wayland: best-effort with explicit caveat; X11: enabling returns an ERROR instead of pretending (any X client can read other windows) | (Wayland) unmapped-window isolation by default | X11 clients can always read other windows |
| Re-apply affinity after window recreation | ✅ implemented — config persisted to `<data_dir>/security.json` (corrupt files quarantined); re-applied at startup, on every window focus (idempotent FFI), and on frontend mount via `reapply_security_config` command | Silent protection drop on webview recreate / restart | — |
| On-screen keyboard for passphrase entry | ✅ implemented (`OnScreenKeyboard.tsx`) — Fisher–Yates reshuffle per open AND per keystroke (click positions leak no frequency data); `preventDefault` keeps input focus; integrated into both vault passphrase fields | Hook-based keyloggers (for the highest-value secret) | Screen recorders watching clicks, kernel loggers |
| Blur content when window loses focus | ✅ implemented (`useFocusBlur` + `.security-blur`) — watches `blur`/`focus`/`visibilitychange`; gated by `blur_on_focus_loss` (OFF by default) | Background capture tools, shoulder surfing | Focused capture |
| Detect known capture processes (OBS etc.) + warn | ✅ implemented (`capture_monitor.rs`, sysinfo minimal-features) — 10s scan, emits `m2m://capture-warning` only when the detected set changes; exact-vs-substring matching discipline unit-tested; banner in UI. Detection is warning UX only | Nothing — but honest detection/warning UX | Determined malware |
| Clipboard auto-clear | ✅ implemented | Clipboard sniffers | Reads before clear fires |
| Idle vault auto-lock | ✅ implemented | Unattended access | Active-session compromise |

**New in this pass:** `SecurityConfig.capture_process_detection` + `SecurityConfig.blur_on_focus_loss` toggles (both OFF by default); `get_capture_capability` command surfaces an honest per-platform capability report ("full" Windows / "partial" macOS+Wayland / "unsupported" X11) directly in Settings; `m2m://security-error` event is emitted loudly if protection fails to apply so the user is never silently unprotected.

**Rule:** a VM window on an infected host is NOT isolated — host keyloggers see keystrokes passing through the host input stack; host recorders capture the VM window. True isolation requires replacing the host environment (Qubes/Tails) or raw hardware input (IOMMU/VT-d USB passthrough).

---

## 3. Kernel Driver Option (Windows hardened mode)

The only in-app path that meaningfully beats kernel-level keyloggers. This is how commercial anti-keyloggers (KeyScrambler) work.

| Driver type | Capability |
|---|-----------|
| Keyboard filter driver (below `kbdclass`) | Sits lower than most keyloggers — encrypt/scramble keystrokes so anything hooked above reads garbage |
| Process-protect callbacks (`ObRegisterCallbacks`) | Block memory readers, injection, debuggers from opening the process |
| Display/capture minifilter | Deny screen-capture APIs for the app window even to admin processes |
| Registry/callback monitoring | Detect persistence attempts by surveillance tools |

**Costs / constraints:**
- Windows loads only signed drivers: EV code-signing cert + Microsoft attestation/WHQL signing (~hundreds $/yr, review cycles per update)
- Driver bugs = BSOD, possibly on boot
- Each Windows major update can break filter internals; PatchGuard blocks some techniques
- Still not absolute: rootkit drivers or hypervisor implants outrank/coexist with yours
- **Windows-only** — macOS third-party kexts are effectively dead (DriverKit too limited)
- Trust paradox: a privacy app requesting ring-0 looks suspicious itself

**Recommended sequencing:** finish app-layer wins → external crypto audit → only if project gains revenue base, ship as paid "Hardened Mode".

---

## 4. Military-Grade Checklist

### Deniability & coercion resistance
- [ ] **Hidden volume** — TrueCrypt-style indistinguishable ciphertext. PARTIALLY covered: multi-account vaults already let a plausible decoy account coexist with the real one (each passphrase selects its account), but ciphertext is not yet indistinguishable between accounts
- [x] **Duress passphrase** — registered hash (Argon2id + fresh salt, only the hash stored); entering it at unlock zeroizes all in-memory secrets, deletes every local DB (+WAL/SHM) and security.json, then returns the IDENTICAL generic wrong-passphrase error (`duress.rs`, `vault.rs::execute_duress_wipe`). Constant-time compare; no confirmation prompt at unlock by design; strength-gated at registration so it can actually trigger. Tested.
- [x] No usernames/identifiers anywhere — keep absolute

### Data at rest
- [x] **Crypto-shredding for deletion** — per-message content keys wrapped under the vault key; deletion shreds the wrapped keys and truncates WAL, making ciphertext unrecoverable even from remnants (H7, tested)
- [x] Disable crash dumps — `SetErrorMode` hardening on Windows; `RLIMIT_CORE=0` via `setrlimit` on Unix; runs FIRST at startup before any key material exists (`lib.rs::disable_crash_dumps`). Note: WER *local-dumps* registry policy is machine-level and outside app control — high-risk users should also verify it's disabled system-wide.
- [x] `VirtualLock`/mlock coverage on long-term secrets — `StorageKey` (panic-on-fail) AND identity Ed25519 seed + X25519 secret now locked at their stable heap addresses on placement into state, unlocked before removal (`secure_key::lock_range/unlock_range`, `*.lock_memory()/unlock_memory()`). Ephemeral/session keys rely on zeroize-on-drop only (short-lived; acceptable). lock_vault additionally clears identity/prekeys now.

### Traffic analysis resistance
- [x] **Fixed-size / bucketed frame padding** — exponential-tier padding (64B→1KB … >4KB→16KB) with tamper-checked padding suffix (`crypto.rs::pad_message_variable`, tested)
- [x] Optional batching delay for sends — random 0..N ms pre-send jitter on every message frame (never handshake/keepalive), configured via `send_batching_ms` (`session.rs::apply_send_jitter`), OFF by default
- [x] **Encrypted+authenticated heartbeats** — heartbeats now travel through the session AEAD path (`Session::send_heartbeat`/`send_heartbeat_ack`, Double Ratchet or legacy SessionKeys); plaintext keepalives were a liveness oracle and forgeable. Only a DECRYPTABLE ack counts as liveness, so injected frames can't defeat the heartbeat timeout. Fixing this exposed and repaired two latent Double Ratchet bugs: the responder could never send first ("no send chain key" — encrypt now forces a DH ratchet when no send chain exists) and the initiator could never decrypt that reply (receive path now accepts a ratcheted frame with no prior receive chain; initiator's initial DR ratchet keypair is the X3DH ephemeral, matching the responder's assumption).
- [x] Cover traffic for typing indicators — randomized 0–600 ms indicator delay when `cover_typing_traffic` enabled (read receipts are local-only in M2M, no wire timing to cover)

### Supply chain & build integrity
- [x] **Reproducible builds scaffolding** — pinned toolchain (`src-tauri/rust-toolchain.toml`), locked-dependency release script with fixed SOURCE_DATE_EPOCH + published hashes (`scripts/build-release.sh`). Full reproducibility claim still needs two independent CI machines producing byte-identical artifacts.
- [x] **sodiumoxide → RustCrypto migration COMPLETE** — Ed25519 (ed25519-dalek 2), X25519 (x25519-dalek 2), XChaCha20-Poly1305-IETF (chacha20poly1305 0.10), SHA-256 (sha2), CSPRNG (getrandom). Byte-compatibility PROVEN by golden vectors captured from libsodium pre-swap (keys, signatures, DH shares, AEAD ciphertexts all identical) plus RFC 8032 §7.1 / RFC 7748 §5.2 known-answer tests. Intentional break: legacy `crypto_kx` session-key derivation replaced with documented HKDF construction (`m2m-kx-v1`) — libsodium's BLAKE2b-based kx is not reproducible outside libsodium; no installed base exists to interop with.
- [x] **Signed updates wiring** — `tauri-plugin-updater` registered, `createUpdaterArtifacts: true`; channel INERT until maintainer sets pubkey+endpoint per `docs/SIGNED-UPDATES.md`
- [x] Dependency pinning + periodic `cargo audit` (CI already runs this); sysinfo pinned to exact version

### Assurance (what makes "military-grade" a fact, not marketing)
- [ ] **External crypto audit** — non-negotiable; the entire difference between claims and reality
- [x] **Fuzzing on `protocol.rs` + internet-facing parsers** — cargo-fuzz crate (`src-tauri/fuzz/`) with 4 targets: length-prefixed frame parser, all attacker-reachable MessagePack wire structs, message padding (roundtrip property), DHT bootstrap parsers. Runs under Linux/WSL nightly (`cargo +nightly fuzz run <target>`); deterministic regression tests (`protocol_fuzz_regression`, 12 tests) encode hostile inputs into normal CI so parser regressions fail on every platform. **First run already paid off**: found `read_frame_impl` panicking on frames declaring `< 2` payload bytes (remote DoS via index OOB) — FIXED with a FrameTooSmall rejection.
- [ ] Formal verification of ratchet/HKDF protocol logic (ProVerif/hax)
- [ ] Bug bounty program

### Operational features for high-risk users
- [x] **Emergency panic wipe** — ARMABLE hotkey Ctrl+Alt+Shift+W (`panic_hotkey_enabled`, OFF by default, double-confirmed at arming): runs the duress wipe (zeroize secrets + delete every DB/config) then exits instantly. Backend refuses when not armed.
- [x] **Air-gap mode** — `SecurityConfig.air_gap_mode`: blocks STUN discovery/connectivity checks, invite creation (STUN/UPnP/relay), peer-discovery enablement, and Tor enable; LAN-only listening/connecting unaffected (`AppState::ensure_not_air_gapped`)
- [x] **Ephemeral RAM-only sessions** — `SecurityConfig.ephemeral_mode`: gates EVERY conversation write (messages sent/received, reactions, edits, deletes, group messages, read receipts, display-name metadata); UI still works live, nothing touches SQLite
- [ ] Hardware-backed key sealing (TPM 2.0 / Secure Enclave) — requires Windows NCrypt/CNG (or Apple CryptoKit) FFI with hardware-in-the-loop testing on real TPMs; a blind implementation without device testing would be security theater. Concrete plan: seal the wrapped storage key via `NCryptImport`/platform TPM key; fall back to current passphrase-only path when no TPM present. Dedicated task.

### Documentation for high-risk users
- [x] Official guides: run M2M in **Tails**, **Whonix**, or **Qubes OS** (`docs/HIGH-RISK-ENVIRONMENTS.md`, including honest limits of each)
- [x] Hardened VM image — `vm/` (Debian-slim Dockerfile + client-only Tor + entrypoint; binary verified against reproducible-build hashes before inclusion; threat-model table in `vm/README.md`)

---

## 5. Recommended Priority Order

1. Critical fixes table §1 (≈1 week focused work)
2. Crypto-shredding deletion
3. Frame padding
4. Reproducible builds
5. Signed updates
6. macOS capture protection + on-screen passphrase keyboard + focus blur
7. Fuzzing
8. External audit ← *this flips "promising" into "verified"*
9. Deniability layer (hidden vault, duress passphrase)
10. Kernel driver as optional Windows "Hardened Mode" (only after audit + sustainable funding)
