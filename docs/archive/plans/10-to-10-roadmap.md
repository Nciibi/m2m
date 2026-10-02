# Phase 2 — Split `commands.rs` Plan

## Goal

Split the 2258-line `commands.rs` monolith into 8 focused files in a `commands/` module — each under 400 lines, with clean module boundaries and no dead code.

## File Structure

```
src-tauri/src/commands/
    mod.rs    — shared types + re-exports
    vault.rs      — identity init + vault lifecycle
    chat.rs       — messaging + conversation management
    files.rs      — file transfer lifecycle
    network.rs    — connection management + invite lifecycle
    settings.rs   — STUN, Tor, private mode, diagnostics
    forwards.rs   — manual port forwarding CRUD
    util.rs       — shared helpers (decode_peer_key, resolve_local_ip, entropy, storage crypto)
```

## Module Breakdown

### `mod.rs` — Shared types (~50 lines)

Move from `commands.rs`:
- `IdentityInfo`, `ConnectionInfo`, `ChatMessage` (keep `Drop` + `Zeroize` impl), `InviteInfo`, `FileTransferInfo`
- `MessageEvent`, `ConnectionEvent`, `FileRequestEvent`
- `VaultStatus`, `ConversationListItem`

Each sub-module re-exports the types it needs via `pub use super::*`.

### `util.rs` — Shared helpers (~100 lines)

- `decode_peer_key()` — hex→32-byte
- `decode_peer_key_logged()` — hex→32-byte with error logging
- `resolve_local_ip()` — UDP probe to find local IP (used by `port_mapping.rs` too)
- `estimate_passphrase_entropy()` — character-class entropy estimator
- `derive_storage_key_from_passphrase()` — Argon2id key derivation
- `derive_storage_key()` — legacy SHA-256 fallback
- `crypto_encrypt_storage()` / `crypto_decrypt_storage()` — XChaCha20-Poly1305 wrappers

### `vault.rs` — Identity & vault (~120 lines)

Commands:
- `init_identity` — load public key, detect existing identity
- `get_identity` — return current identity info
- `get_vault_status` — locked/unlocked state
- `unlock_vault` — 3-case: first-run, legacy migration, normal unlock

### `chat.rs` — Messaging & conversations (~180 lines)

Commands:
- `send_message` — encrypt + send text, persist to history
- `load_messages` — load decrypted message history
- `list_conversations` — enumerate convos with previews
- `rename_conversation` — local display name
- `delete_conversation_cmd` — secure delete
- `set_conversation_retention` — auto-delete/export policy
- `send_conversation_names` — peer naming metadata
- `export_conversation` — encrypted JSON export

### `files.rs` — File transfer (~160 lines)

Commands:
- `send_file` — initiate outgoing transfer
- `accept_file_transfer` — accept incoming, start chunk streaming
- `reject_file_transfer` — reject incoming

Private:
- `send_file_chunks` — async chunk sender
- `create_temp_file` — pre-allocated temp file

### `network.rs` — Connections & invites (~400 lines)

Commands:
- `create_invite` — generate invite with candidates, NAT-PMP, Tor guard
- `validate_invite` — parse and verify invite link
- `start_listening` — bind TCP listener + spawn accept loop
- `connect_to_peer` — hole-punch connect + handshake
- `get_connection_state` — peer connection status
- `verify_peer` — mark fingerprint verified
- `disconnect_peer` — graceful disconnect
- `list_peers` — enumerate active connections
- `get_listen_address` — bound socket address

Private:
- `handle_incoming_connection` — responder handshake + receive loop spawn
- `spawn_receive_loop` — full frame dispatch loop (handles all PacketTypes)

### `settings.rs` — Network settings (~160 lines)

Commands:
- `discover_public_ip` — STUN consensus discovery
- `get_stun_config` / `set_stun_servers` — STUN server management
- `set_private_mode` — toggle IP exposure in invites
- `check_connectivity` — NAT type + reachability
- `get_network_diagnostics` — full diagnostics payload
- `get_network_settings` — Tor + public IP status
- `set_tor_enabled` — toggle Tor proxy

### `forwards.rs` — Manual port forwarding (~70 lines)

Commands:
- `list_manual_forwards`
- `add_manual_forward`
- `remove_manual_forward`
- `reorder_manual_forwards`

## Changes to existing files

### `lib.rs`
Replace `mod commands;` with:
```rust
mod commands;  // re-exports from commands/mod.rs
```
The `invoke_handler` registration stays unchanged — all commands are re-exported from `commands::*`.

### `port_mapping.rs`
Update `crate::commands::resolve_local_ip()` → `crate::commands::util::resolve_local_ip()` (line 1098).

## Dead code cleanup

Items to clean **during the split**, not separately:

| Item | Action |
|------|--------|
| `#[allow(dead_code)]` on `state.rs:31` (`PeerConnection.remote_addr`) | Move with the struct; keep annotation |
| `#[allow(dead_code)]` on `state.rs:90` (`AppState.data_dir`) | Move with the struct; keep annotation |
| `#[allow(dead_code)]` on `hole_punch.rs:56` (`TcpHolePunch`) | **Keep** — Phase 3 wires this |
| `#[allow(dead_code)]` on `hole_punch.rs:58` (`TcpRelay`) | **Keep** — Phase 3 wires this |
| Unused `PathBuf` import in `commands.rs` | Remove during split |
| `RESERVED_VERSIONS` in `protocol.rs` | Already used (line 149) — keep, roadmap was incorrect |
| `SessionKeyContext` in `crypto.rs` | Investigate during split |

## Execution order

1. Create `src-tauri/src/commands/` directory
2. Write `mod.rs` with shared types + `pub use` re-exports
3. Write `util.rs` — pure helpers, no Tauri dependency
4. Write `vault.rs`, `chat.rs`, `files.rs`, `settings.rs`, `forwards.rs`
5. Write `network.rs` — largest file, includes `spawn_receive_loop` and `handle_incoming_connection`
6. Update `lib.rs` — import path
7. Update `port_mapping.rs` — `resolve_local_ip` path
8. Remove original `commands.rs`
9. `cargo build` — must succeed with zero warnings
10. `cargo clippy -- -D warnings` — must pass clean
11. Git commit with message "Split commands.rs into 8-module commands/ directory"

## Testing

- `cargo build` — must compile without errors
- `cargo clippy -- -D warnings` — must pass clean
- All existing integration paths work unchanged (compiler enforces via `tauri::generate_handler![]`)
