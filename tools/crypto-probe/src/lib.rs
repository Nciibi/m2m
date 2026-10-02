//! Standalone runner for the app's pure-crypto modules.
//!
//! Five modules are copied in by `sync.sh`: `crypto.rs`, `group.rs`,
//! `protocol.rs`, `secure_key.rs` and `storage.rs`. They have no Tauri
//! dependency, so this crate can compile **and execute** their `#[cfg(test)]`
//! modules outside the Tauri build. That is the only way to actually *run*
//! Rust tests in an environment that cannot link the app crate — which needs a
//! C toolchain for `rusqlite` (bundled SQLite) and for Tauri itself.
//!
//! This is not a substitute for the app's own test suite — that still needs a
//! toolchain-capable machine. It is the one place where a crypto change can be
//! executed rather than merely compiled.
//!
//! `storage.rs` is included so the storage-cap accounting and eviction tests
//! actually execute — that is the whole point of the cap, and untested eviction
//! is a silent data-loss feature. It reaches outside itself for `ChatMessage`,
//! which `commands.rs` now **generates** from the live
//! `src-tauri/src/commands/mod.rs`. It used to be a hand-maintained,
//! field-for-field copy, in a directory where everything else is
//! machine-copied: adding a field to `ChatMessage` would silently desync it
//! while `storage.rs`'s tests kept passing against the wrong shape.
//!
//! Run: `./sync.sh && cargo test --lib`
pub mod commands;
pub mod commands_generated;
pub mod crypto;
pub mod error;
pub mod group;
pub mod protocol;
pub mod secure_key;
pub mod storage;
