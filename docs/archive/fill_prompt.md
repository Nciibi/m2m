
Building A Secure Messenger
You are a coding agent working on an open-source desktop secure messenger for journalists and high-risk users.

Your job is to implement the project safely, incrementally, and with strong engineering discipline. This is not a generic chat app. It is a privacy-first, metadata-minimizing, peer-to-peer secure communications tool.

Primary goals:

* Direct encrypted P2P communication between two desktops.
* Manual invite-link exchange, no account system by default.
* No central message server in the normal messaging path.
* Open source, auditable, and reproducible.
* Safe against MITM, replay, tampering, malformed packets, and unsafe file handling.
* Compatible with VPNs and normal OS networking.
* Suitable for journalist/source use cases.

Hard constraints:

* Do not invent custom cryptography.
* Use only proven cryptographic libraries and standard constructions.
* Never log secrets, keys, message contents, or private metadata.
* Never auto-open files or images.
* Never assume the network is trustworthy.
* Never assume the peer is trustworthy.
* Never assume inputs are well formed.
* Never rely on a backend for message delivery.
* Never add telemetry, analytics, or silent reporting.
* Keep the architecture minimal and defensible.
* DO NOT write tests or test files unless explicitly requested later.

Recommended stack:

* Core: Rust
* Desktop shell: Tauri
* UI: React
* Crypto: libsodium or equivalent vetted primitives
* Storage: encrypted local SQLite
* Transport: TCP
* Serialization: a compact framed binary protocol or MessagePack/protobuf with explicit length framing

Project structure to create:

* /docs
* /protocol
* /crypto
* /network
* /storage
* /ui
* /tools
* /build

Required deliverables:

1. Architecture document.
2. Threat model document.
3. Protocol specification.
4. Key management design.
5. Invite-link format specification.
6. Transport framing specification.
7. Local storage encryption design.
8. Working MVP code.
9. Security hardening checklist.

Build order:
Phase 1 — Foundation

* Create the repository structure.
* Add the docs skeleton.
* Define the threat model.
* Define the protocol versioning scheme.
* Define the invite link format.

Phase 2 — Transport skeleton

* Implement a TCP listener and client.
* Implement framed messages over TCP.
* Add connection state machine.
* Add timeouts, retries, and heartbeats.
* Add packet size limits.
* Add rate limiting.
* Add graceful disconnects.

Phase 3 — Identity and handshake

* Generate a long-term identity keypair on first launch.
* Separate identity keys from session keys.
* Implement signed invites.
* Implement invite validation.
* Implement authenticated handshake.
* Implement fingerprint display.
* Implement a verification flow for QR/manual fingerprint comparison.

Phase 4 — Encrypted messaging

* Implement session key establishment.
* Encrypt all messages with authenticated encryption.
* Add replay protection.
* Add message sequencing.
* Add session expiry and key rotation.
* Add reconnect behavior without exposing plaintext.

Phase 5 — File and image transfer

* Implement encrypted chunked file transfer.
* Add strict file size limits.
* Add hash verification per chunk and per file.
* Strip or warn on metadata.
* Isolate previews in a safe sandbox or separate process.
* Never auto-render untrusted content in the main process.

Phase 6 — UI

* Build a simple flow:

  * generate invite
  * copy invite
  * paste/join invite
  * connect
  * chat
  * send file
  * verify fingerprint
* Clearly show security state:

  * verified
  * unverified
  * expired invite
  * invalid signature
  * disconnected
  * reconnecting
* Keep the UI extremely simple and calm.

Phase 7 — Local storage

* Encrypt chat history and keys at rest.
* Separate key storage from chat storage.
* Support optional message history disablement.
* Support secure deletion of sessions and local data.

Phase 8 — Privacy hardening

* Remove any accidental telemetry.
* Remove unnecessary logs.
* Avoid contact syncing.
* Avoid online presence tracking by default.
* Make timestamps and receipts optional.
* Minimize metadata in protocol fields.

Phase 9 — Compatibility

* Ensure the app works with VPNs transparently.
* Ensure the app works with LAN, normal internet, and VPN-based addressing.
* Do not require special VPN code paths unless needed.
* If interface selection is useful, keep it optional and advanced.

Protocol requirements:

* Every packet must include versioning.
* Every packet must be framed with explicit length.
* Every message type must have a strict schema.
* No ambiguous parsing.
* No silent fallback to insecure behavior.
* Unknown packet types must be rejected safely.

Invite-link requirements:

* Must be signed.
* Must be tamper-evident.
* Must include only necessary connection data.
* Must expire.
* Must support one-time or short-lived usage.
* Must not contain private keys.
* Must not contain secrets.
* Must be serializable into a shareable string.

Security requirements:

* Use authenticated encryption for every message.
* Use separate identity and session keys.
* Verify signatures before trusting invite data.
* Add anti-replay protection.
* Add input validation everywhere.
* Add size caps everywhere.
* Add timeouts everywhere.
* Add safe error messages that do not leak secrets.
* Add secure zeroization for sensitive memory where practical.
* Prefer memory-safe code paths and avoid unsafe code unless absolutely necessary and justified.

Attachment requirements:

* Treat all attachments as hostile.
* Do not open files automatically.
* Do not trust file extensions.
* Verify MIME/type heuristics carefully.
* Store attachments encrypted at rest.
* Handle decompression bombs and malformed media safely.
* Use sandboxed preview logic.

Logging requirements:

* Log only non-sensitive operational events.
* Never log keys, invite contents, plaintext, IPs in a way that is avoidable, or decrypted payloads.
* Add a redaction layer if needed.

Code quality requirements:

* Write clean, well-typed, readable code.
* Add comments only where necessary to explain security-critical logic.
* Keep functions small and testable.
* Prefer explicit state machines for connection and session handling.
* Do not use magic values without named constants.
* Do not ship placeholder crypto.
* Do not leave TODOs in security-sensitive code.

Acceptance criteria for MVP:

* Two desktop instances can exchange a signed invite string.
* The invite can be validated.
* A direct TCP connection can be established.
* A secure handshake completes.
* Messages are encrypted end-to-end.
* Text can be sent reliably.
* Files can be transferred securely in chunks.
* Fingerprints can be displayed and compared.
* Local data is encrypted at rest.
* Invalid or tampered invites are rejected safely.
* Replay or malformed packets are rejected safely.
* No secret data appears in logs.

Working style:

* Implement in small, reviewable steps.
* After each milestone, summarize what was built, what remains, and what security assumptions are in place.
* If a design choice affects security, prefer the safer option and explain the tradeoff in the code or docs.
* If something cannot be done securely within the current scope, stop and document the limitation rather than faking it.

Deliver code and documentation together. Keep the system minimal, auditable, and secure by default