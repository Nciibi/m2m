# M2M Full Analysis: What's Done vs What's Not

## ✅ Phase 1: Polishing the Desktop Experience

| Task | Status | Detail |
|------|--------|--------|
| **Tauri Dialogs** | ❌ Not done | Frontend uses `prompt()` for file paths — needs `@tauri-apps/plugin-dialog` |
| **System Tray & Notifications** | ❌ Not done | No tray icon, no OS notifications on incoming messages |
| **Passphrase Key Encryption** | ⚠️ Stubbed | Argon2id is in `Cargo.toml` but never used. `derive_storage_key()` uses a weak SHA-256 of the public key instead. No vault password UI exists. |

## ✅ Phase 2: Networking & True P2P

| Task | Status | Detail |
|------|--------|--------|
| **NAT Traversal (STUN)** | ❌ Not done | Connections are direct TCP only — works on LAN, fails through NAT |
| **Tor Proxy Support** | ❌ Not done | No SOCKS5 proxy support exists |

## ✅ Phase 3: Hardening & Auditing

| Task | Status | Detail |
|------|--------|--------|
| **File I/O Loop** | ⚠️ 80% done | Chunk reassembly exists in `commands.rs:714-737` but `accept_file_transfer` doesn't store `save_dir` into `IncomingFileTransfer`, so chunks have nowhere to write. |
| **Memory Auditing (Zeroize)** | ⚠️ Partial | `Session` and `SessionKeys` zeroize on drop, but `MessageBody`, `ChatMessage`, and decrypted plaintext buffers do NOT. |
| **Reproducible Builds (CI/CD)** | ❌ Not done | No `.github/workflows/` directory exists |

## ⚠️ Compilation Blockers (Must Fix First)

1. **`commands.rs:129,133`** — `IdentityKeypair` used without import (user reverted the fix)
2. **`crypto.rs:130`** — `Signature::new()` is deprecated (warning)
3. **`commands.rs:906`** — `save_dir` is unused (warning, but we'll USE it properly now)

---

## Implementation Plan

### Order of operations:
1. Fix the 2 compilation errors + 2 warnings
2. Install Tauri plugins (dialog, notification) — Rust + JS side
3. Implement Passphrase/Vault Password flow (Argon2id)
4. Add STUN module for NAT traversal
5. Add Tor/SOCKS5 proxy support
6. Complete file I/O loop (use `save_dir`, streaming writes)
7. Add Zeroize to all sensitive structs
8. Create GitHub Actions CI/CD pipeline
9. Update frontend with new UI flows (vault password, native dialogs, settings, notifications)
