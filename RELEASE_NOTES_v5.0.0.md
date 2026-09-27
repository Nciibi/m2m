# M2M 5.0.0 — Hardening Release

> **This is a wire-breaking release. Both sides must upgrade.**
> The wire protocol moved from `0x02` to `0x03`. A 5.0.0 client will refuse to
> complete a handshake with a 4.x peer and vice versa. There is no
> compatibility shim and no downgrade path — this is deliberate. The previous
> protocol version left authenticated headers unauthenticated, and a
> cross-version fallback would have kept that hole open for anyone still
> running 4.x.

---

## Why 5.0.0

4.x was feature-complete but not yet defensible. A review of the transport,
crypto, and storage layers turned up several issues that mattered specifically
for the threat model this app targets — journalists and people at risk of
targeted surveillance — rather than for a general-purpose messenger. Most were
invisible from the UI.

## Security

### Wire protocol v0x02 → v0x03 (breaking)

The Double Ratchet derived an AEAD key from a chain that did **not** cover the
ratchet key or the message counter. An attacker with a position on the network
could truncate or reorder a session — replaying older ciphertext, dropping
messages, or substituting the sender's ratchet key — and the receiving side
would decrypt it without complaint.

Both fields are now bound into the AEAD associated data, so tampering fails
authentication. `0x02` is rejected at handshake time, and because the protocol
version is itself authenticated, a downgrade attempt is detected too.

### Peer key cannot silently change mid-session

Session rekeying used a non-constant-time comparison to check whether a peer key
had "changed", and would re-key on mismatch instead of failing. A peer that
presented a different key got a fresh session with no error surfaced. Key
comparison is now constant-time and a mismatch aborts the session.

### One-time prekeys are actually single-use

Both X3DH responder paths could complete a handshake without consuming the
prekey. The same prekey could be replayed to derive the same session twice.
Prekeys are now burned after a successful handshake on both paths.

### Group messaging: SenderKey forgery and replay

Receiving a group message derived the SenderKey, compared it, and only then
committed state — so a forged group message could desynchronise a sender's
commitment chain. Derivation now happens fully before any state is committed,
and replays and oversized chain gaps are rejected. The sender's key change is
authorised rather than accepted from any group member.

### Replay and duplication resistance

- Relay and direct inbound connections are gated on the contact allowlist.
  Unauthenticated peers can no longer displace an existing connection to
  establish a second one.
- Sync invite tokens are authorised, single-use, and scoped to the paired
  device.
- Disconnect and typing frames are authenticated. A new, unestablished
  connection can no longer forcibly terminate an established one.

### Resource limits on untrusted input

- Frame size is bounded to 1 MiB globally, with a tighter per-packet-type cap,
  and the header is validated before any allocation.
- Per-connection rate limiting caps frames per second and bytes per second, and
  disconnects after five consecutive breaches.
- DHT packet declared lengths no longer underflow.
- STUN responses are verified against RFC 5389 `FINGERPRINT`.
- UPnP responses are bound in size and time and no longer permit cloud-metadata
  redirection; LAN port mapping is refused outright while Tor is enabled.

### Crypto hygiene

- Nonce length and counter arithmetic are validated; a malformed peer-supplied
  nonce aborts the operation instead of the process.
- X25519 public keys are validated on the curve, with strict Ed25519
  small-order point rejection.
- HKDF is covered by RFC 5869 test vectors.
- Memory locking is ordered before the key material it protects.
- The RNG and key derivation paths retry rather than fall back to weaker input.
- The duress passphrase is compared in constant time.

## Connectivity

All outbound TCP now goes through a single audited dial path. Previously each
subsystem connected independently, and it was possible for one of them to bypass
the privacy rules the rest obeyed. In particular:

- **Tor routing can no longer fall back to a direct connection.** Every path
  that can open a socket goes through the chokepoint, and an architecture test
  fails the build if a new raw `TcpStream::connect` is introduced.
- LAN port mapping is disabled while Tor is active.

## Reliability

- Group, sync, and reaction payloads are bounds-checked before use.
- Clipboard auto-clear now reports failure loudly instead of silently leaving a
  copied passphrase or fingerprint in the clipboard.
- Peer verification no longer reports success for an unverified peer.

## Interface

- Native panic and duress confirmations replaced with in-app dialogs.
- Event payloads arriving over the Tauri bridge are validated at runtime
  against typed guards; malformed or hostile payloads are dropped rather than
  rendered.
- The render path contains no `any`.
- ARIA tab semantics, a live transcript region, progress announcements, and
  severity-aware toasts.
- Focus management: modals no longer steal focus on mount, and the blur-on-focus-loss
  wrapper is correctly inert.
- Contexts are memoized; event listeners no longer re-register on every render,
  which was both a performance problem and a window in which inbound messages
  were dropped.

## Upgrading

1. Upgrade **both** peers to 5.0.0 before exchanging new messages. A 4.x peer
   will fail to connect rather than silently misbehave.
2. Existing sessions do not resume. A fresh handshake is performed, which
   consumes a new prekey bundle from each peer.
3. No message history is lost — stored ciphertext and plaintext records are
   untouched. Only the live ratchet state is re-established.
4. Review Security settings after upgrading. The `tor_reachable` indicator was
   reading a field that the connectivity check never returned, so it always
   reported failure; it is now correct.

## Verification

| Suite | Result |
| --- | --- |
| `cargo test --all-targets` (app) | 379 passed |
| `cargo test` (relay server) | 14 passed |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `tsc --noEmit` | clean |
| `vitest` | 210 passed |
| `eslint` | 0 errors, 22 warnings (budget: 23) |

The 22 remaining warnings are React Compiler advisories
(`set-state-in-effect`, `preserve-manual-memoization`) about render scheduling.
None represent incorrect behaviour, and the lint budget is pinned to the current
count so the number cannot regress.
