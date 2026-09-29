//! Standalone runner for the app's pure-crypto modules.
//!
//! `crypto.rs`, `group.rs`, `protocol.rs` and `secure_key.rs` have no GTK or
//! Tauri dependency, so this crate can compile **and execute** their
//! `#[cfg(test)]` modules outside the Tauri build. That is the only way to
//! actually *run* Rust tests in this environment: the app crate itself cannot
//! link, because `glib-sys` / `gio-sys` / `gdk-sys` need GTK development
//! packages that are not obtainable here.
//!
//! This is not a substitute for the app's own test suite — that still needs a
//! GTK-capable machine. It is the one place where a crypto change can be
//! executed rather than merely compiled.
//!
//! `storage.rs` is included so the storage-cap accounting and eviction tests
//! actually execute — that is the whole point of the cap, and untested eviction
//! is a silent data-loss feature. It reaches outside itself for `ChatMessage`
//! and two AEAD helpers, which `commands.rs` provides as documented stand-ins.
//!
//! Run: `./sync.sh && cargo test --offline --lib`
pub mod commands;
pub mod crypto;
pub mod error;
pub mod group;
pub mod protocol;
pub mod secure_key;
pub mod storage;
