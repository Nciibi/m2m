# M2M Full Analysis: What's Done vs What's Not

## ✅ Phase 1: Polishing the Desktop Experience

| Task | Status | Detail |
|------|--------|--------|
| **Tauri Dialogs** | ❌ Not done | Frontend uses `prompt()` for file paths — needs `@tauri-apps/plugin-dialog` |
| **System Tray & Notifications** | ⚠️ Partial | OS notifications work via `tauri-plugin-notification`. No system tray icon. |
| **Vault Passphrase UI** | ✅ Done | Argon2id key derivation with 64 MiB memory, 3 iterations, 4 lanes. Full `unlock_vault` flow in the UI. |

## ✅ Phase 2: Networking & True P2P

| Task | Status | Detail |
|------|--------|--------|
| **STUN NAT Traversal** | ✅ Done | RFC 8489 compliant, multi-server parallel queries, cross-server consensus detection, NAT type classification |
| **Tor SOCKS5 Proxy** | ✅ Done | Outbound routing via local SOCKS5 proxy with Tor Guard (hard blocks invites when Tor enabled without Private Mode) |
| **TCP Hole Punch** | ✅ Done | Simultaneous open via `tokio::select!` racing listener.accept() vs connect(peer_candidates) |
| **UPnP / NAT-PMP / PCP** | ✅ Done | All three port-mapping protocols behind a unified `PortMapper` facade with automatic lease renewal |
| **IPv6 Support** | ✅ Done | IPv6 global unicast candidate discovery via UDP probe against IPv6 DNS servers |
| **Manual Port Forwarding** | ✅ Done | User-configured forwards stored as type-4 candidates, managed via Tauri commands (add/remove/reorder) |
| **Happy Eyeballs Connection Manager** | ✅ Done | Parallel race across all connection strategies via `tokio::task::JoinSet` |

## ✅ Phase 3: Hardening & Auditing

| Task | Status | Detail |
|------|--------|--------|
| **File Transfer** | ✅ Done | Chunked streaming with per-chunk SHA-256 hash verification, temp-file streaming (no RAM buffering), path-traversal sanitization |
| **Memory Zeroization** | ✅ Done | `Session`, `SessionKeys`, `MessageBody`, `ChatMessage` — all zeroize on drop via `zeroize` + `Zeroizing` |
| **CI/CD** | ⚠️ Partial | `cargo test` and `cargo fmt` — needs `cargo audit` and reproducible build automation |

## ✅ Phase 4: Encryption Upgrades

| Task | Status | Detail |
|------|--------|--------|
| **Per-message Ratchet** | ✅ Done | SHA-256 KDF ratchet after every message. `tx_key` and `rx_key` evolve independently per direction. |
| **Message Padding** | ✅ Done | Exponential-tier padding (128 B/256 B/512 B/1 KiB/2 KiB tiers) — obfuscates plaintext length on the wire |
| **Replay Protection** | ✅ Done | Monotonic counter + AEAD AAD binding. Counter check + authentication tag verification before decryption. |

## ✅ Phase 5 (Roadmap): Future Work

| Task | Priority | Detail |
|------|----------|--------|
| **Double Ratchet + X3DH** | Phase 1 | Full Signal-style self-healing forward secrecy |
| **Split commands.rs** | Phase 2 | Break the 2100-line monolith into domain-specific command modules |
| **TURN Relay Server** | Phase 3 | Lightweight TCP relay for symmetric NAT fallback |
| **Fuzzing + property tests** | Phase 4 | Fuzz harness for protocol parsing, storage property tests |
| **mlock() for sensitive memory** | Phase 4 | Prevent session keys from being paged to disk |
| **Frontend lift** | Phase 5 | TypeScript strict mode, state extraction, component tests |
| **Protocol v0x02** | Phase 6 | Entropy estimation, keepalive cleanup, protocol negotiation |
