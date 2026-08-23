//! M2M — Duress Passphrase (coercion resistance)
//!
//! Roadmap §4 "Duress passphrase": a special passphrase that, when entered
//! at unlock, silently destroys the vault instead of revealing it.
//!
//! ## Semantics
//!
//! - The user registers a DISTINCT duress passphrase while the vault is
//!   unlocked (`set_duress_passphrase`). Only an Argon2id HASH is stored.
//! - If the duress passphrase is entered at unlock, M2M:
//!   1. zeroizes every in-memory secret,
//!   2. deletes ALL local databases (crypto-shredded per-message keys make
//!      message ciphertext unrecoverable regardless of disk remnants),
//!   3. returns the SAME generic error a wrong passphrase produces.
//!
//! An attacker who forces the user to type the duress passphrase sees an
//! ordinary unlock failure — no decoy UI, no tell.
//!
//! ## Honest limits
//!
//! - Irreversible by design. There is deliberately NO confirmation prompt at
//!   unlock (that would defeat the purpose); confirmation exists only when
//!   registering.
//! - Export/backup files outside the data directory are NOT touched.
//!
//! ## Decoy accounts
//!
//! Multi-account support already provides the hidden-volume-adjacent
//! property: each account is selected solely by its own passphrase, so a
//! plausible low-sensitivity decoy account can coexist with the real one.

use crate::commands::util::derive_storage_key_from_passphrase;
use crate::storage::KeyStore;

/// vault_meta keys for the stored verifier.
pub const META_DURESS_HASH: &str = "duress_hash";
pub const META_DURESS_SALT: &str = "duress_salt";
pub const META_DURESS_SET_AT: &str = "duress_set_at";

/// Whether a duress passphrase is registered.
pub fn is_set(key_store: &KeyStore) -> bool {
    matches!(
        key_store.get_meta(META_DURESS_HASH),
        Ok(Some(h)) if h.len() == 64
    )
}

/// Register (or replace) the duress passphrase. Stores ONLY a hash:
/// Argon2id(passphrase, fresh random salt).
pub fn register(key_store: &KeyStore, passphrase: &str) -> Result<(), String> {
    let mut salt = [0u8; 16];
    {
        // libsodium CSPRNG — same randomness source as every key in M2M.
        use sodiumoxide::randombytes;
        let bytes = randombytes::randombytes(16);
        salt.copy_from_slice(&bytes);
    }
    let key = derive_storage_key_from_passphrase(passphrase, &salt)
        .map_err(|e| format!("duress hash derivation failed: {e}"))?;
    let now = chrono::Utc::now().timestamp().to_string();
    key_store.set_meta(META_DURESS_SALT, &hex::encode(salt)).map_err(|e| e.to_string())?;
    key_store.set_meta(META_DURESS_HASH, &hex::encode(key.as_bytes())).map_err(|e| e.to_string())?;
    key_store.set_meta(META_DURESS_SET_AT, &now).map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove the duress registration.
pub fn clear(key_store: &KeyStore) -> Result<(), String> {
    key_store.set_meta(META_DURESS_HASH, "").map_err(|e| e.to_string())?;
    key_store.set_meta(META_DURESS_SALT, "").map_err(|e| e.to_string())?;
    key_store.set_meta(META_DURESS_SET_AT, "").map_err(|e| e.to_string())?;
    Ok(())
}

/// Constant-time equality for equal-length byte slices.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Check whether `entered` IS the registered duress passphrase.
///
/// Runs Argon2id (expensive) — call only after the cheap `is_set` check.
/// Returns false on any malformed state; NEVER errors, so a probing
/// attacker learns nothing from error shapes.
pub fn verify(key_store: &KeyStore, entered: &str) -> bool {
    let stored_hash_hex = match key_store.get_meta(META_DURESS_HASH) {
        Ok(Some(h)) if h.len() == 64 => h,
        _ => return false,
    };
    let salt = match key_store.get_meta(META_DURESS_SALT) {
        Ok(Some(s)) => match hex::decode(&s) {
            Ok(bytes) if bytes.len() == 16 => bytes,
            _ => return false,
        },
        _ => return false,
    };
    let computed = match derive_storage_key_from_passphrase(entered, &salt) {
        Ok(k) => *k.as_bytes(),
        Err(_) => return false,
    };
    let stored = match hex::decode(&stored_hash_hex) {
        Ok(h) => h,
        Err(_) => return false,
    };
    ct_eq(&computed, &stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn mem_keystore() -> KeyStore {
        KeyStore::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_not_set_by_default() {
        let ks = mem_keystore();
        assert!(!is_set(&ks));
        assert!(!verify(&ks, "anything-at-least-twelve"));
    }

    #[test]
    fn test_register_verify_clear_roundtrip() {
        let ks = mem_keystore();
        register(&ks, "under-duress-passphrase-123").unwrap();
        assert!(is_set(&ks));
        assert!(verify(&ks, "under-duress-passphrase-123"));
        assert!(!verify(&ks, "wrong-passphrase-987654"));
        clear(&ks).unwrap();
        assert!(!is_set(&ks));
        assert!(!verify(&ks, "under-duress-passphrase-123"));
    }

    #[test]
    fn test_stored_value_is_a_hash_not_the_passphrase() {
        let ks = mem_keystore();
        register(&ks, "top-secret-duress-value").unwrap();
        let raw = ks.get_meta(META_DURESS_HASH).unwrap().unwrap();
        assert_eq!(raw.len(), 64); // 32-byte hash hex
        assert!(!raw.contains("top-secret"));
    }
}
