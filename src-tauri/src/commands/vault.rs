//! Vault and identity commands.
//!
//! Handles keypair generation, passphrase-based vault locking/unlocking,
//! and identity info queries.

use std::sync::Arc;

use tauri::{State, Emitter};
use zeroize::Zeroizing;

use crate::crypto::{self, IdentityKeypair};
use crate::state::AppState;
use crate::storage::{self, KeyStore};

use super::util;
use super::{IdentityInfo, VaultStatus};

/// Initialize the crypto library and check for existing identity.
/// Does NOT decrypt the private key — that is deferred to `unlock_vault`.
#[tauri::command]
pub async fn init_identity(
    state: State<'_, Arc<AppState>>,
) -> Result<IdentityInfo, String> {
    crypto::init().map_err(|e| format!("crypto init failed: {e}"))?;

    let data_dir = storage::ensure_data_dir()
        .map_err(|e| format!("data dir error: {e}"))?;
    let keys_db_path = data_dir.join("keys.db");

    let key_store = KeyStore::open(&keys_db_path)
        .map_err(|e| format!("key store error: {e}"))?;

    let has_identity = key_store.has_identity().unwrap_or(false);

    let result = if has_identity {
        // Load only the public key — no decryption needed
        let pub_bytes = key_store
            .load_public_key()
            .map_err(|e| format!("failed to load public key: {e}"))?;

        if pub_bytes.len() != 32 {
            return Err("invalid public key length in storage".to_string());
        }
        let mut pub_arr = [0u8; 32];
        pub_arr.copy_from_slice(&pub_bytes);

        let fingerprint = crypto::fingerprint_from_public_key(&pub_arr);
        let pub_hex = hex::encode(&pub_bytes);

        // Persist vault_initialized flag into in-memory state
        let vault_initialized = key_store.is_vault_initialized().unwrap_or(false);
        {
            let mut vi = state.vault_initialized.write().await;
            *vi = vault_initialized;
        }

        IdentityInfo {
            fingerprint,
            public_key_hex: pub_hex,
            has_identity: true,
        }
    } else {
        IdentityInfo {
            fingerprint: String::new(),
            public_key_hex: String::new(),
            has_identity: false,
        }
    };

    // Store key store handle for unlock_vault to use later
    {
        let mut ks = state.key_store.lock().await;
        *ks = Some(key_store);
    }

    Ok(result)
}

/// Get the current identity info.
#[tauri::command]
pub async fn get_identity(
    state: State<'_, Arc<AppState>>,
) -> Result<IdentityInfo, String> {
    let identity = state.identity.read().await;
    match identity.as_ref() {
        Some(kp) => Ok(IdentityInfo {
            fingerprint: kp.fingerprint(),
            public_key_hex: hex::encode(kp.public_key_bytes()),
            has_identity: true,
        }),
        None => Ok(IdentityInfo {
            fingerprint: String::new(),
            public_key_hex: String::new(),
            has_identity: false,
        }),
    }
}

/// Get the current vault lock status.
#[tauri::command]
pub async fn get_vault_status(
    state: State<'_, Arc<AppState>>,
) -> Result<VaultStatus, String> {
    let initialized = *state.vault_initialized.read().await;
    let unlocked = *state.vault_unlocked.read().await;
    Ok(VaultStatus { initialized, unlocked })
}

/// Unlock (or initialise) the vault with a passphrase.
///
/// Three cases:
/// 1. **First run** (no identity): generates a new keypair, encrypts with Argon2id key, stores it.
/// 2. **Legacy migration** (identity exists, vault not yet initialized): decrypts with legacy
///    fallback key, re-encrypts with Argon2id key, marks vault as initialized.
/// 3. **Normal unlock** (identity exists, vault initialized): decrypts with Argon2id key.
///
/// In all cases, the full `IdentityKeypair` and `MessageStore` are loaded into state.
#[tauri::command]
pub async fn unlock_vault(
    state: State<'_, Arc<AppState>>,
    passphrase: String,
) -> Result<VaultStatus, String> {
    // ─── Passphrase Strength Check ───
    if passphrase.len() < 12 {
        return Err(
            "passphrase must be at least 12 characters — longer is more secure".to_string(),
        );
    }
    // Estimate entropy: if weaker than 40 bits, reject.
    let entropy = util::estimate_passphrase_entropy(&passphrase);
    if entropy < 40.0 {
        return Err(format!(
            "passphrase too weak: ~{:.0} bits of entropy. \
             Use a longer passphrase (aim for 60+ bits). \
             Try a diceware phrase with 5+ random words.",
            entropy
        ));
    }

    let data_dir = storage::ensure_data_dir()
        .map_err(|e| format!("data dir error: {e}"))?;
    let msgs_db_path = data_dir.join("messages.db");

    // Access the key store that init_identity opened
    let ks_guard = state.key_store.lock().await;
    let key_store = ks_guard
        .as_ref()
        .ok_or("key store not initialized — call init_identity first")?;

    let vault_was_initialized = key_store.is_vault_initialized().unwrap_or(false);
    let has_identity = key_store.has_identity().unwrap_or(false);

    let keypair = if has_identity {
        // ── Existing identity ──
        let (pub_bytes, enc_sk, nonce) = key_store
            .load_identity()
            .map_err(|e| format!("failed to load identity: {e}"))?;

        let mut pub_arr = [0u8; 32];
        pub_arr.copy_from_slice(&pub_bytes);

        if vault_was_initialized {
            // Case 3: Normal unlock — decrypt with Argon2id passphrase key
            let storage_key = util::derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
            let sk_bytes = util::crypto_decrypt_storage(&enc_sk, &nonce, &storage_key)
                .map_err(|_| "incorrect passphrase or corrupted data".to_string())?;

            let mut sk_arr = [0u8; 64];
            sk_arr.copy_from_slice(&sk_bytes);

            {
                let mut sk_lock = state.storage_key.write().await;
                *sk_lock = Some(Zeroizing::new(storage_key));
            }

            IdentityKeypair::from_bytes(&pub_arr, &sk_arr)
                .map_err(|e| format!("failed to reconstruct identity: {e}"))?
        } else {
            // Case 2: Legacy migration — decrypt with legacy key, re-encrypt with Argon2id
            tracing::warn!("migrating legacy identity to vault — setting passphrase for first time");
            let legacy_key = util::derive_storage_key(&pub_bytes);
            let sk_bytes = util::crypto_decrypt_storage(&enc_sk, &nonce, &legacy_key)
                .map_err(|e| format!("failed to decrypt legacy identity: {e}"))?;

            let mut sk_arr = [0u8; 64];
            sk_arr.copy_from_slice(&sk_bytes);

            // Re-encrypt with the new passphrase-derived key
            let new_key = util::derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
            let (new_nonce, new_enc_sk) = util::crypto_encrypt_storage(&sk_bytes, &new_key)
                .map_err(|e| format!("failed to re-encrypt identity: {e}"))?;

            key_store
                .update_encrypted_private_key(&new_enc_sk, &new_nonce)
                .map_err(|e| format!("failed to update identity: {e}"))?;
            key_store
                .set_vault_initialized()
                .map_err(|e| format!("failed to mark vault initialized: {e}"))?;

            {
                let mut sk_lock = state.storage_key.write().await;
                *sk_lock = Some(Zeroizing::new(new_key));
            }

            IdentityKeypair::from_bytes(&pub_arr, &sk_arr)
                .map_err(|e| format!("failed to reconstruct identity: {e}"))?
        }
    } else {
        // ── Case 1: First run — generate new identity ──
        let kp = IdentityKeypair::generate()
            .map_err(|e| format!("keypair generation failed: {e}"))?;

        let pub_bytes = kp.public_key_bytes();
        let sk_bytes = kp.secret_key_bytes();

        let storage_key = util::derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
        let (nonce, encrypted_sk) = util::crypto_encrypt_storage(&sk_bytes, &storage_key)
            .map_err(|e| format!("failed to encrypt identity: {e}"))?;

        let now = chrono::Utc::now().timestamp();
        key_store
            .store_identity(&pub_bytes, &encrypted_sk, &nonce, now)
            .map_err(|e| format!("failed to store identity: {e}"))?;
        key_store
            .set_vault_initialized()
            .map_err(|e| format!("failed to mark vault initialized: {e}"))?;

        {
            let mut sk_lock = state.storage_key.write().await;
            *sk_lock = Some(Zeroizing::new(storage_key));
        }

        kp
    };

    // Drop key_store lock before acquiring other locks
    drop(ks_guard);

    // Initialize message store (deferred from init_identity to here)
    let msg_store = storage::MessageStore::open(&msgs_db_path)
        .map_err(|e| format!("message store error: {e}"))?;
    {
        let mut ms = state.message_store.lock().await;
        *ms = Some(msg_store);
    }

    // Store the full keypair in state
    {
        let mut id_lock = state.identity.write().await;
        *id_lock = Some(keypair);
    }
    {
        let mut vi = state.vault_initialized.write().await;
        *vi = true;
    }
    {
        let mut vu = state.vault_unlocked.write().await;
        *vu = true;
    }

    Ok(VaultStatus {
        initialized: true,
        unlocked: true,
    })
}
