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
- Encrypt transfers.db / family contacts / reactions metadata, or document as intentionally plaintext
- ~~Heartbeat timeout documented but never implemented~~ **✅ FIXED** — heartbeat worker polls at half the ack timeout, tracks `last_hb_sent`/`last_hb_ack` per connection (`state.rs::PeerConnection`); an unanswered probe tears the connection down within one timeout window (reconnect info saved + disconnect event emitted); acks recorded in the receive loop
- ~~Handshake signatures don't cover `candidates`~~ **✅ FIXED** — all four handshake paths (legacy + X3DH × initiator + responder) fold a canonical length-prefixed encoding of the candidate list into the signed data via `append_candidates_to_sign_data`; candidate tampering (reconnect-target poisoning) now invalidates the signature; regression-tested
- ~~DHT `parse_node_response` hardcodes IPv4 while announce supports IPv6~~ **✅ FIXED** — NODE_RESPONSE entries now carry an af_tag (4/6) mirroring the announce body format, parsed sequentially so mixed v4/v6 bodies work; legacy fixed-38-byte all-IPv4 responses fall back cleanly; tested (tagged, IPv6, mixed-AF, legacy fallback)
- Split ~1,400-line `spawn_receive_loop`; de-duplicate ChatMessage construction sites

---

## 2. Anti-Surveillance Features (screen recording / keyloggers)

**Honest threat model:** an app can defeat casual/opportunistic spying; nothing user-space defeats kernel-level malware on the same machine.

| Technique | Status | Stops | Can't stop |
|-----------|--------|-------|------------|
| `WDA_EXCLUDEFROMCAPTURE` (Windows) | ✅ implemented (`window_security.rs:57`) | OBS, Zoom-share, most recorders | Kernel capture, HDMI capture cards, phone camera |
| macOS `NSWindow.sharingType = .none` | ❌ stub (`window_security.rs:70`) | — | Implement via objc FFI |
| Linux X11 privacy hint / Wayland isolation | ❌ stub | — | X11 clients can always read other windows |
| Re-apply affinity after window recreation | ❌ missing | Silent protection drop on webview recreate | — |
| On-screen keyboard for passphrase entry | ❌ TODO | Hook-based keyloggers (for the highest-value secret) | Screen recorders watching clicks |
| Blur content when window loses focus | ❌ TODO | Background capture tools | Focused capture |
| Detect known capture processes (OBS etc.) + warn | ❌ TODO | Nothing — but honest detection/warning UX | Determined malware |
| Clipboard auto-clear | ✅ implemented | Clipboard sniffers | Reads before clear fires |
| Idle vault auto-lock | ✅ implemented | Unattended access | Active-session compromise |

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
- [ ] **Hidden volume** — decoy vault under fake passphrase opens boring data; indistinguishable ciphertext (TrueCrypt-style)
- [ ] **Duress passphrase** — special password silently unlocks decoy and/or triggers wipe instead of revealing real vault
- [ ] No usernames/identifiers anywhere — keep absolute

### Data at rest
- [ ] **Crypto-shredding for deletion** — destroy per-message keys instead of deleting rows; ciphertext becomes unrecoverable regardless of WAL/disk remnants (current `secure_delete` + skip-VACUUM leaves remnants, `storage.rs:1039-1054`)
- [ ] Disable crash dumps (minidumps contain decrypted message buffers)
- [ ] Full `VirtualLock`/mlock coverage on all plaintext secrets

### Traffic analysis resistance
- [ ] **Fixed-size / bucketed frame padding** — message sizes currently reveal content type (64 KiB text vs chunks vs reactions)
- [ ] Optional batching delay for sends — send-time reveals nothing
- [x] **Encrypted+authenticated heartbeats** — heartbeats now travel through the session AEAD path (`Session::send_heartbeat`/`send_heartbeat_ack`, Double Ratchet or legacy SessionKeys); plaintext keepalives were a liveness oracle and forgeable. Only a DECRYPTABLE ack counts as liveness, so injected frames can't defeat the heartbeat timeout. Fixing this exposed and repaired two latent Double Ratchet bugs: the responder could never send first ("no send chain key" — encrypt now forces a DH ratchet when no send chain exists) and the initiator could never decrypt that reply (receive path now accepts a ratcheted frame with no prior receive chain; initiator's initial DR ratchet keypair is the X3DH ephemeral, matching the responder's assumption).
- [ ] Consider cover traffic for typing indicators / read receipts timing metadata

### Supply chain & build integrity
- [ ] **Reproducible builds** — released binaries verifiably match source; without this every other claim is unverifiable
- [ ] Migrate archived sodiumoxide → RustCrypto (`ed25519-dalek`, `x25519-dalek`, `chacha20poly1305`)
- [ ] **Signed updates** via Tauri updater signatures — unsigned update channel is the kill-chain
- [ ] Dependency pinning + periodic `cargo audit` (CI already runs this)

### Assurance (what makes "military-grade" a fact, not marketing)
- [ ] **External crypto audit** — non-negotiable; the entire difference between claims and reality
- [ ] Fuzzing with cargo-fuzz on `protocol.rs` parsing (internet-facing packet parser)
- [ ] Formal verification of ratchet/HKDF protocol logic (ProVerif/hax)
- [ ] Bug bounty program

### Operational features for high-risk users
- [ ] **Emergency panic wipe** — hotkey zeroizes vault keys + deletes storage mid-session
- [ ] **Air-gap mode** — LAN-only operation, no internet-facing listener
- [ ] **Ephemeral RAM-only sessions** — conversation never touches SQLite
- [ ] Hardware-backed key sealing (TPM 2.0 / Secure Enclave) — disk theft alone yields nothing

### Documentation for high-risk users
- [ ] Official guides: run M2M in **Tails**, **Whonix**, or **Qubes OS**
- [ ] Ship hardened VM image (Debian + M2M + Tor preconfigured)

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
