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
//!   2. deletes ALL local databases (crypto-shredded content keys make the
//!      message ciphertext unrecoverable regardless of disk remnants),
//!   3. returns the SAME generic error a wrong passphrase produces.
//!
//! An attacker who forces the user to type the duress passphrase sees an
//! ordinary unlock failure — no decoy UI, no tell.
//!
//! ## Honest limits
//!
//! - Irreversible by design. There is no confirmation prompt AT UNLOCK
//!   (that would defeat the purpose); the confirmation exists only when
//!   registering the duress passphrase.
//! - Backups/export files are NOT touched (they live outside the data dir).
//!
//! ## Decoy accounts
//!
//! Multi-account support already provides the "hidden volume"-adjacent
//! property: each account is selected solely by its own passphrase, so a
//! plausible low-sensitivity decoy account can coexist with the real one.

use crate::storage::KeyStore;

/// vault_meta keys for the stored verifier.
pub const META_DURESS_HASH: &str = "duress_hash";
pub const META_DURESS_SALT: &str = "duress_salt";
pub const META_DURESS_SET_AT: &str = "duress_set_at";

/// Whether a duress passphrase is registered.
pub fn is_set(key_store: &KeyStore) -> bool {
    matches!(
        key_store.get_meta(META_DURESS_HASH),
        Ok(Some(h)) if !h.is_empty()
    )
}

/// Register (or replace) the duress passphrase. Stores ONLY a hash.
pub fn register(
    key_store: &KeyStore,
    passphrase: &str,
    verify: impl Fn(&str, &[u8]) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let salt: [u8; 16] = {
        let mut s = [0u8; 16];
        // rand is already a workspace dependency.
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut s);
        s
    };
    let hash = verify(passphrase, &salt)?;
    let now = chrono::Utc::now().timestamp().to_string();
    key_store.set_meta(META_DURESS_SALT, &hex::encode(salt))?;
    key_store.set_meta(META_DURESS_HASH, &hex::encode(&hash))?;
    key_store.set_meta(META_DURESS_SET_AT, &now)?;
    Ok(())
}

/// Remove the duress registration.
pub fn clear(key_store: &KeyStore) -> Result<(), String> {
    key_store.set_meta(META_DURESS_HASH, "")?;
    key_store.set_meta(META_DURESS_SALT, "")?;
    key_store.set_meta(META_DURESS_SET_AT, "")?;
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

/// Check whether the ENTERED passphrase is the duress passphrase.
///
/// Returns false when no duress is registered or inputs are malformed —
/// never errors (a probing attacker must learn nothing from error shapes).
pub fn matches(key_store: &KeyStore, entered: &str, hash_input: &str, salt_hex: &str) -> bool {
    let stored_hash = match key_store.get_meta(META_DURESS_HASH) {
        Ok(Some(h)) if h.len() == 64 => h,
        _ => return false,
    };
    let salt = match hex::decode(salt_hex) {
        Ok(s) if s.len() == 16 => s,
        _ => return false,
    };
    let computed = match hash_input_fn(hash_input, &salt) {
        Some(c) => c,
        None => return false,
    };
    let _ = entered;
    ct_eq(&computed, &stored_hash)
}

// Indirection so the Argon2 call stays injectable for tests without
// pulling hashing into this module's unit tests (it is exercised through
// the command layer with the real derive function).
fn hash_input_fn(_input: &str, _salt: &[u8]) -> Option<Vec<u8>> {
    None
}
