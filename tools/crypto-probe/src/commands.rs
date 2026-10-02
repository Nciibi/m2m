//! Minimal stand-ins so `storage.rs` can be compiled and tested standalone.
//!
//! ## Why this exists
//!
//! `storage.rs` reaches outside itself for two things: the `ChatMessage` shape it
//! returns, and two AEAD helpers used by the *identity* record path. The real
//! definitions live in `commands/mod.rs` (which pulls in `tauri`) and
//! `commands/util.rs` (which returns `AppError`, which in turn maps the backend
//! error enums). Neither is reachable from this probe without dragging the whole
//! command layer in.
//!
//! ## What this does and does not buy
//!
//! It lets the **storage-cap accounting and eviction** tests run — byte counting,
//! oldest-first ordering, shred-before-delete, group-table coverage. Those paths
//! never call the functions below.
//!
//! It does **not** verify them. The two AEAD helpers here are a faithful
//! reimplementation of `commands/util.rs` (XChaCha20-Poly1305, 24-byte nonce,
//! same AAD binding) but they are a copy, not the original. A change to the real
//! helper would not be caught here. That is the same trade the
//! `tools/typecheck-harness/tauri_stub` makes, for the same reason: a stub that
//! lets real code be *executed* beats no execution at all, provided the limit is
//! written down.
//!
//! `ChatMessage`, by contrast, **is** the real definition — `sync.sh` extracts it
//! from `src-tauri/src/commands/mod.rs` into `commands_generated.rs` on every
//! run. It used to be hand-maintained in this file, which meant a field added to
//! the real struct could silently desync from the copy that `storage.rs`'s tests
//! compile against, with nothing to notice.

pub mod util {
    use crate::error::AppError;

    /// Mirror of `commands::util::crypto_encrypt_storage`.
    pub fn crypto_encrypt_storage(
        plaintext: &[u8],
        key: &crate::secure_key::StorageKey,
        aad: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), AppError> {
        use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
        let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key.as_bytes()));
        let nonce_bytes = crate::crypto::random_bytes(24);
        let nonce = chacha20poly1305::XNonce::from_slice(&nonce_bytes);
        let ct = cipher
            .encrypt(
                nonce,
                chacha20poly1305::aead::Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| AppError::new("storage.encryption_failed", "encryption failed"))?;
        Ok((nonce_bytes, ct))
    }

    /// Mirror of `commands::util::crypto_decrypt_storage`.
    pub fn crypto_decrypt_storage(
        ciphertext: &[u8],
        nonce_bytes: &[u8],
        key: &crate::secure_key::StorageKey,
        aad: &[u8],
    ) -> Result<Vec<u8>, AppError> {
        use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
        if nonce_bytes.len() != 24 {
            return Err(AppError::invalid("invalid nonce"));
        }
        let cipher = XChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(key.as_bytes()));
        cipher
            .decrypt(
                chacha20poly1305::XNonce::from_slice(nonce_bytes),
                chacha20poly1305::aead::Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| {
                AppError::new(
                    "storage.decryption_failed",
                    "decryption failed - wrong key, or the data has been modified",
                )
            })
    }
}

/// The real `ChatMessage`, generated from `src-tauri/src/commands/mod.rs` by
/// `sync.sh` into `commands_generated.rs` on every run.
///
/// It used to be hand-written here, field for field, with a comment claiming "a
/// drift here shows up as a compile error in `storage.rs`". That was not true:
/// `storage.rs` compiles against *this* copy, so drift was invisible by
/// construction, in the one file in this directory that was not machine-copied.
pub use crate::commands_generated::ChatMessage;
