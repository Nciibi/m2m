//! Vault and identity commands.
//!
//! Handles keypair generation, passphrase-based vault locking/unlocking,
//! identity info queries, family contact management, and identity export/import.

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine};
use tauri::{AppHandle, Emitter, State};

use crate::crypto::{self, IdentityKeypair};
use crate::state::AppState;
use crate::storage::{self, KeyStore};
use zeroize::Zeroize;

use super::util;
use super::{ConnectionEvent, ConnectionInfo, FamilyMember, IdentityInfo, VaultStatus};

/// Run Argon2id key derivation on the blocking thread pool so the ~100ms+
/// of CPU work never stalls the async runtime (M5).
async fn derive_key_blocking(
    passphrase: String,
    salt: Vec<u8>,
) -> Result<crate::secure_key::StorageKey, String> {
    tokio::task::spawn_blocking(move || {
        util::derive_storage_key_from_passphrase(&passphrase, &salt)
    })
    .await
    .map_err(|e| format!("key derivation task failed: {e}"))?
}

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
/// 1. **Multi-account unlock** (vault initialized, accounts exist): tries the passphrase
///    against every account's wrapped secret key — AEAD success selects the account.
/// 2. **Legacy migration** (identity exists, vault not yet initialized): decrypts with legacy
///    fallback key, re-encrypts with Argon2id key, marks vault as initialized.
/// 3. **First run** (no identity): generates a new keypair, encrypts with Argon2id key, stores it.
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

    let _data_dir = storage::ensure_data_dir()
        .map_err(|e| format!("data dir error: {e}"))?;
    // Note: messages.db and transfers.db paths are used by
    // ensure_message_store / ensure_transfer_store lazy init in chat.rs/state.rs

    // ─── Phase 1: Synchronous DB reads (key_store is !Send, must not cross .await) ───
    let ks_guard = state.key_store.lock().await;
    let key_store = ks_guard
        .as_ref()
        .ok_or("key store not initialized — call init_identity first")?;

    key_store.migrate_legacy_identity_to_account().ok();

    let vault_was_initialized = key_store.is_vault_initialized().unwrap_or(false);
    let has_identity = key_store.has_identity().unwrap_or(false);

    let accounts: Vec<storage::AccountRow> = if vault_was_initialized {
        key_store
            .list_accounts()
            .map_err(|e| format!("failed to list accounts: {e}"))?
    } else {
        Vec::new()
    };

    // Pre-vault profile needing migration — read the legacy row directly.
    let legacy_row = if !vault_was_initialized && has_identity {
        Some(
            key_store
                .load_identity()
                .map_err(|e| format!("failed to load identity: {e}"))?,
        )
    } else {
        None
    };

    // X25519 rows are vault-global (v1) — preload if present.
    let x25519_preload = if key_store.has_x25519_key().unwrap_or(false) {
        match key_store.load_x25519_key() {
            Ok((xp, xe, xn)) => Some((xp, xe, xn)),
            Err(e) => return Err(format!("failed to load X25519 key: {e}")),
        }
    } else {
        None
    };

    // ─── Duress passphrase check (coercion resistance) ───
    // Read the verifier BEFORE dropping the store; the expensive Argon2id
    // runs afterwards without holding the !Send guard.
    let duress_verifier = crate::duress::read_verifier(key_store);
    drop(ks_guard); // key_store is dead — .await is safe now

    if let Some((stored_hash_hex, salt)) = duress_verifier {
        let entered = passphrase.clone();
        let derived_hex = tauri::async_runtime::spawn_blocking(move || {
            crate::duress::derive_verifier_hex(&entered, &salt)
        })
        .await
        .unwrap_or(None);
        if derived_hex.as_deref() == Some(stored_hash_hex.as_str()) {
            // Duress confirmed: destroy everything, then answer EXACTLY like
            // a wrong passphrase ("No account matches this passphrase.").
            tracing::warn!("DURESS passphrase entered — wiping vault");
            execute_duress_wipe(&state).await;
            return Err("No account matches this passphrase.".to_string());
        }
    }

    // ─── Phase 2: Async crypto + per-branch logic (no key_store held) ───
    let (keypair, x25519_kp, needs_store_x25519, legacy_store_data): (
        _,
        _,
        _,
        Option<(Vec<u8>, Vec<u8>, [u8; 64], Vec<u8>)>,
    ) = if vault_was_initialized && !accounts.is_empty() {
        // Case 3: Multi-account unlock — passphrase selects the account.
        let mut matched: Option<(IdentityKeypair, crate::secure_key::StorageKey)> = None;
        for acct in &accounts {
            let storage_key =
                derive_key_blocking(passphrase.clone(), acct.public_key.clone()).await?;
            let sk_bytes = match util::crypto_decrypt_storage(
                &acct.encrypted_private_key,
                &acct.private_key_nonce,
                &storage_key,
                util::AAD_KEY_STORE,
            ) {
                Ok(sk) => sk,
                Err(_) => continue,
            };
            if sk_bytes.len() != 64 || acct.public_key.len() != 32 {
                continue;
            }
            let mut pub_arr = [0u8; 32];
            pub_arr.copy_from_slice(&acct.public_key);
            let mut sk_arr = [0u8; 64];
            sk_arr.copy_from_slice(&sk_bytes);
            match IdentityKeypair::from_bytes(&pub_arr, &sk_arr) {
                Ok(kp) => {
                    matched = Some((kp, storage_key));
                    break;
                }
                Err(_) => continue,
            }
        }
        let (kp, storage_key) = matched.ok_or("No account matches this passphrase.")?;

        let (xkp, x_needs_store) = if let Some((ref x_pub, ref x_enc, ref x_nonce)) = x25519_preload
        {
            match util::crypto_decrypt_storage(x_enc, x_nonce, &storage_key, util::AAD_KEY_STORE) {
                Ok(x_sk_bytes) => {
                    let mut x_sk_arr = [0u8; 32];
                    x_sk_arr.copy_from_slice(&x_sk_bytes);
                    let xkp = crate::crypto::X25519IdentityKeypair::from_bytes(x_pub, &x_sk_arr)
                        .map_err(|e| format!("failed to reconstruct X25519: {e}"))?;
                    (xkp, false)
                }
                Err(_) => (crate::crypto::X25519IdentityKeypair::generate(), true),
            }
        } else {
            (crate::crypto::X25519IdentityKeypair::generate(), true)
        };

        // Store storage_key in state (one .await write)
        {
            let mut sk_lock = state.storage_key.write().await;
            *sk_lock = Some(storage_key);
        }

        (kp, xkp, x_needs_store, None)
    } else if let Some((pub_bytes, enc_sk, nonce)) = legacy_row {
        // Case 2: Legacy migration
        tracing::warn!("migrating legacy identity to vault — setting passphrase for first time");
        let mut pub_arr = [0u8; 32];
        pub_arr.copy_from_slice(&pub_bytes);
        let legacy_key = util::derive_storage_key(&pub_bytes);
        // Historic writers used two different AADs: very old profiles
        // were sealed with an empty AAD, while older versions of
        // import_identity sealed with AAD_KEY_STORE. Try both so every
        // legacy profile and all previously-imported identities can
        // migrate (H1). Current import_identity seals under a
        // passphrase-derived key and never reaches this path.
        let sk_bytes = match util::crypto_decrypt_storage(&enc_sk, &nonce, &legacy_key, b"") {
            Ok(sk) => sk,
            Err(_) => util::crypto_decrypt_storage(&enc_sk, &nonce, &legacy_key, util::AAD_KEY_STORE)
                .map_err(|_| {
                    "failed to decrypt legacy identity — data may be corrupted".to_string()
                })?,
        };
        let mut sk_arr = [0u8; 64];
        sk_arr.copy_from_slice(&sk_bytes);
        let mut sk_bytes = sk_bytes;
        sk_bytes.zeroize();

        // Derive new key and re-encrypt
        let new_key = derive_key_blocking(passphrase.clone(), pub_bytes.to_vec()).await?;
        let (new_nonce, new_enc_sk) = util::crypto_encrypt_storage(&sk_arr, &new_key, util::AAD_KEY_STORE)
            .map_err(|e| format!("failed to re-encrypt identity: {e}"))?;
        let kp = IdentityKeypair::from_bytes(&pub_arr, &sk_arr)
            .map_err(|e| format!("failed to reconstruct identity: {e}"))?;

        let xkp = crate::crypto::X25519IdentityKeypair::generate();

        // Pack the data needed for Phase 4 DB writes
        let legacy_store_data = Some((new_nonce, new_enc_sk, kp.secret_key_bytes(), pub_bytes));

        // Store storage_key in state
        {
            let mut sk_lock = state.storage_key.write().await;
            *sk_lock = Some(new_key);
        }

        (kp, xkp, true, legacy_store_data)
    } else if !has_identity {
        // Case 1: First run
        let kp = IdentityKeypair::generate()
            .map_err(|e| format!("keypair generation failed: {e}"))?;

        let pub_bytes = kp.public_key_bytes();
        let sk_bytes = kp.secret_key_bytes();

        let storage_key = derive_key_blocking(passphrase.clone(), pub_bytes.to_vec()).await?;
        let (nonce, encrypted_sk) = util::crypto_encrypt_storage(&sk_bytes, &storage_key, util::AAD_KEY_STORE)
            .map_err(|e| format!("failed to encrypt identity: {e}"))?;

        let xkp = crate::crypto::X25519IdentityKeypair::generate();
        let x_sk_bytes = xkp.secret_key_bytes();
        let x_pub = xkp.public_key_bytes();
        let (x_nonce, x_enc) = util::crypto_encrypt_storage(&x_sk_bytes, &storage_key, util::AAD_KEY_STORE)
            .map_err(|e| format!("failed to encrypt X25519 key: {e}"))?;

        let now = chrono::Utc::now().timestamp();

        // Store both keys to DB synchronously
        let ks_guard2 = state.key_store.lock().await;
        let key_store2 = ks_guard2.as_ref().ok_or("key store not initialized")?;
        key_store2.store_identity(&pub_bytes, &encrypted_sk, &nonce, now)
            .map_err(|e| format!("failed to store identity: {e}"))?;
        key_store2.set_vault_initialized()
            .map_err(|e| format!("failed to mark vault initialized: {e}"))?;
        key_store2.store_x25519_key(&x_pub, &x_enc, &x_nonce)
            .map_err(|e| format!("failed to store X25519 key: {e}"))?;
        key_store2.insert_account(&pub_bytes, &encrypted_sk, &nonce, Some("Main"), now)
            .map_err(|e| format!("failed to store account: {e}"))?;
        drop(ks_guard2);

        // Store storage_key in state
        {
            let mut sk_lock = state.storage_key.write().await;
            *sk_lock = Some(storage_key);
        }

        (kp, xkp, false, None)
    } else {
        return Err("vault data missing — cannot unlock".to_string());
    };

    // ─── Phase 3: Async state writes ───
    {
        let mut id_lock = state.identity.write().await;
        // mlock sweep: lock seed pages now that the keypair sits at its
        // stable heap address inside the RwLock (see lock_range caveat).
        keypair.lock_memory();
        *id_lock = Some(keypair);
    }
    {
        let mut x_lock = state.x25519_identity.write().await;
        x25519_kp.lock_memory();
        *x_lock = Some(x25519_kp);
    }
    {
        let mut vi = state.vault_initialized.write().await;
        *vi = true;
    }
    {
        let mut vu = state.vault_unlocked.write().await;
        *vu = true;
    }

    // ─── Phase 4: Encrypt X25519 data (no .await, no key_store) then DB writes ───
    let x25519_store_data = if needs_store_x25519 {
        let xkp_ref = state.x25519_identity.read().await;
        // Return errors instead of panicking on IPC-reachable paths (M6).
        let xkp = xkp_ref
            .as_ref()
            .ok_or("internal error: X25519 key missing after unlock")?;
        let x_sk_bytes = xkp.secret_key_bytes();
        let x_pub = xkp.public_key_bytes();
        let sk = state.storage_key.read().await;
        let st_key = sk
            .as_ref()
            .ok_or("internal error: storage key missing after unlock")?;
        let result = util::crypto_encrypt_storage(&x_sk_bytes, st_key, util::AAD_KEY_STORE)
            .ok()
            .map(|(n, e)| (x_pub, e, n));
        drop(sk);
        drop(xkp_ref);
        result
    } else {
        None
    };

    // Acquire lock only for synchronous DB writes (no .await while holding it)
    if needs_store_x25519 || legacy_store_data.is_some() {
        let ks_guard4 = state.key_store.lock().await;
        if let Some(store) = ks_guard4.as_ref() {
            // Migration persistence must NOT fail silently: a half-migrated
            // profile would stay in legacy mode while the user believes the
            // passphrase was set (H1).
            if let Some((lnonce, lenc, _lsk, lpub)) = &legacy_store_data {
                store.update_encrypted_private_key(lenc, lnonce)
                    .map_err(|e| format!("failed to persist migrated identity: {e}"))?;
                store.set_vault_initialized()
                    .map_err(|e| format!("failed to mark vault initialized: {e}"))?;
                if store.insert_account(lpub, lenc, lnonce, Some("Main"), chrono::Utc::now().timestamp()).is_err() {
                    store.update_account_private_key(lpub, lenc, lnonce)
                        .map_err(|e| format!("failed to persist migrated account: {e}"))?;
                }
            }
            if let Some((ref x_pub, ref x_enc, ref x_nonce)) = x25519_store_data {
                store.store_x25519_key(x_pub, x_enc, x_nonce)
                    .map_err(|e| format!("failed to persist X25519 key: {e}"))?;
            }
        }
        drop(ks_guard4);
    }

    Ok(VaultStatus {
        initialized: true,
        unlocked: true,
    })
}

/// Create an additional vault account wrapped under its own passphrase.
/// The passphrase selects the account on subsequent unlocks.
#[tauri::command]
pub async fn create_vault_account(
    state: State<'_, Arc<AppState>>,
    passphrase: String,
) -> Result<IdentityInfo, String> {
    // ─── Passphrase Strength Check ───
    if passphrase.len() < 12 {
        return Err(
            "passphrase must be at least 12 characters — longer is more secure".to_string(),
        );
    }
    let entropy = util::estimate_passphrase_entropy(&passphrase);
    if entropy < 40.0 {
        return Err(format!(
            "passphrase too weak: ~{:.0} bits of entropy. \
             Use a longer passphrase (aim for 60+ bits). \
             Try a diceware phrase with 5+ random words.",
            entropy
        ));
    }

    let kp = IdentityKeypair::generate()
        .map_err(|e| format!("keypair generation failed: {e}"))?;
    let fingerprint = kp.fingerprint();
    let pub_bytes = kp.public_key_bytes();
    let sk_bytes = kp.secret_key_bytes();

    let xkp = crate::crypto::X25519IdentityKeypair::generate();

    let storage_key = derive_key_blocking(passphrase, pub_bytes.to_vec()).await?;
    let (nonce, encrypted_sk) = util::crypto_encrypt_storage(&sk_bytes, &storage_key, util::AAD_KEY_STORE)
        .map_err(|e| format!("failed to encrypt identity: {e}"))?;

    let now = chrono::Utc::now().timestamp();
    {
        let ks_guard = state.key_store.lock().await;
        let key_store = ks_guard
            .as_ref()
            .ok_or("key store not initialized — call init_identity first")?;
        key_store.set_vault_initialized()
            .map_err(|e| format!("failed to mark vault initialized: {e}"))?;
        key_store.insert_account(&pub_bytes, &encrypted_sk, &nonce, None, now)
            .map_err(|e| format!("failed to create account: {e}"))?;
    }

    {
        let mut id_lock = state.identity.write().await;
        *id_lock = Some(kp);
    }
    {
        let mut x_lock = state.x25519_identity.write().await;
        *x_lock = Some(xkp);
    }
    {
        let mut sk_lock = state.storage_key.write().await;
        *sk_lock = Some(storage_key);
    }
    {
        let mut vi = state.vault_initialized.write().await;
        *vi = true;
    }
    {
        let mut vu = state.vault_unlocked.write().await;
        *vu = true;
    }

    Ok(IdentityInfo {
        fingerprint,
        public_key_hex: hex::encode(&pub_bytes),
        has_identity: true,
    })
}

// ─── Family Commands ───────────────────────────────────────────────────────

/// List all non-expired family members.
#[tauri::command]
pub async fn list_family(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<FamilyMember>, String> {
    let sk = state.storage_key.read().await;
    let ks = state.key_store.lock().await;
    let store = ks.as_ref().ok_or("key store not initialized")?;
    store.list_family(sk.as_ref()).map_err(|e| format!("failed to list family: {e}"))
}

/// Add a peer to the family list.
/// `peer_key_hex` must belong to a peer we've connected with before.
#[tauri::command]
pub async fn add_family_member(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    nickname: String,
    expires_in_days: Option<u64>,
) -> Result<FamilyMember, String> {
    if nickname.trim().is_empty() {
        return Err("nickname cannot be empty".to_string());
    }
    let pk_bytes = util::decode_peer_key(&peer_key_hex)
        .map_err(|e| format!("invalid peer key: {e}"))?;

    // Check peer has a conversation (must have connected at least once)
    {
        let ms = state.message_store.lock().await;
        let has_conversation = ms.as_ref()
            .and_then(|m| m.get_conversation(&peer_key_hex).ok())
            .flatten()
            .is_some();
        if !has_conversation {
            return Err("no conversation with this peer".to_string());
        }
    }

    // Add to family (no .await while holding key_store lock)
    let member = {
        let sk = state.storage_key.read().await;
        let ks = state.key_store.lock().await;
        let store = ks.as_ref().ok_or("key store not initialized")?;
        store.add_family_member(&pk_bytes, &nickname, expires_in_days, None, sk.as_ref())
            .map_err(|e| format!("failed to add family member: {e}"))?
    };

    Ok(member)
}

/// Remove a peer from the family list.
#[tauri::command]
pub async fn remove_family_member(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
) -> Result<(), String> {
    let pk_bytes = util::decode_peer_key(&peer_key_hex)
        .map_err(|e| format!("invalid peer key: {e}"))?;
    {
        let ks = state.key_store.lock().await;
        let store = ks.as_ref().ok_or("key store not initialized")?;
        store.remove_family_member(&pk_bytes)
            .map_err(|e| format!("failed to remove family member: {e}"))?
    }
    Ok(())
}

/// Update a family member's nickname.
#[tauri::command]
pub async fn set_family_nickname(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    nickname: String,
) -> Result<(), String> {
    if nickname.trim().is_empty() {
        return Err("nickname cannot be empty".to_string());
    }
    let pk_bytes = util::decode_peer_key(&peer_key_hex)
        .map_err(|e| format!("invalid peer key: {e}"))?;
    {
        let sk = state.storage_key.read().await;
        let ks = state.key_store.lock().await;
        let store = ks.as_ref().ok_or("key store not initialized")?;
        store.set_family_nickname(&pk_bytes, &nickname, sk.as_ref())
            .map_err(|e| format!("failed to set nickname: {e}"))?
    }
    Ok(())
}

/// Try to connect to a family member using their saved info.
/// If the address is stale, returns a user-friendly error.
#[tauri::command]
pub async fn connect_family_member(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
) -> Result<ConnectionInfo, String> {
    let pk_bytes = util::decode_peer_key(&peer_key_hex)
        .map_err(|e| format!("invalid peer key: {e}"))?;

    // Extract the identity keypair bytes — drop guard before any .await
    let identity_keypair = {
        let identity = state.identity.read().await;
        let kp = identity.as_ref().ok_or("identity not initialized")?;
        let pub_bytes = kp.public_key_bytes();
        let sk_bytes = kp.secret_key_bytes();
        IdentityKeypair::from_bytes(&pub_bytes, &sk_bytes)
            .map_err(|e| format!("identity error: {e}"))?
    };

    // Look up the family member — drop key_store lock before any .await
    let saved_addr_str: Option<String> = {
        let sk = state.storage_key.read().await;
        let ks = state.key_store.lock().await;
        let store = ks.as_ref().ok_or("key store not initialized")?;
        if !store.is_family_member(&pk_bytes).map_err(|e| format!("family check: {e}"))? {
            return Err("peer is not a family member".to_string());
        }
        let members = store.list_family(sk.as_ref()).map_err(|e| format!("list family: {e}"))?;
        members.into_iter()
            .find(|m| m.public_key_hex == peer_key_hex)
            .and_then(|m| m.last_address)
    };

    let saved_addr: Option<std::net::SocketAddr> = saved_addr_str
        .as_ref()
        .and_then(|s| s.parse().ok());

    // Try connecting if we have an address
    if let Some(addr) = saved_addr {
        match crate::tor::connect(addr).await {
            Ok(mut stream) => {
                let mut session = crate::session::Session::new();

                // Gather candidates
                let config = state.stun_config.read().await;
                let stun_result = crate::stun::discover_public_addrs(&config).await.ok();
                drop(config);

                let host_candidates = crate::candidate::gather_host_candidates();
                let ipv6_candidates = crate::candidate::gather_ipv6_candidates();
                let reflexive_candidates = stun_result
                    .as_ref()
                    .map(crate::candidate::gather_reflexive_candidates)
                    .unwrap_or_default();

                let mut all = host_candidates;
                all.extend(ipv6_candidates);
                all.extend(reflexive_candidates);
                all.sort_by(|a, b| b.priority.cmp(&a.priority));
                let our_candidates: Vec<crate::protocol::WireCandidate> = all.iter().map(|c| {
                    crate::protocol::WireCandidate {
                        address: c.address.clone(),
                        candidate_type: c.candidate_type as u8,
                        relay_id: None,
                    }
                }).collect();

                let x25519 = state.x25519_identity.read().await;
                let x25519_pub = x25519.as_ref()
                    .map(|k| k.public_key_bytes())
                    .unwrap_or([0u8; 32]);

                // Skip identity pre-check — we already know this peer
                let expected = [0u8; 32];
                session.handshake_as_initiator(&mut stream, &identity_keypair, &expected, our_candidates, x25519_pub)
                    .await
                    .map_err(|e| format!("handshake failed: {e}"))?;

                let actual_peer_key = hex::encode(session.peer_identity_pub);
                let peer_fingerprint = session.peer_fingerprint();
                session.mark_peer_verified();

                let (read_half, write_half) = stream.into_split();
                let conn = crate::state::PeerConnection {
                    write_half,
                    session,
                    remote_addr: addr,
                    strategy_name: "family".to_string(),
                    last_hb_sent: None,
                    last_hb_ack: None,
                };

                {
                    let mut conns = state.connections.write().await;
                    conns.insert(actual_peer_key.clone(), Arc::new(tokio::sync::Mutex::new(conn)));
                }

                let _ = app_handle.emit("m2m://connection", ConnectionEvent {
                    peer_key_hex: actual_peer_key.clone(),
                    state: "established".to_string(),
                    peer_fingerprint: Some(peer_fingerprint.clone()),
                    peer_verified: false,
                });

                // Upsert peer in key store
                if let Some(pk) = util::decode_peer_key_logged(&actual_peer_key) {
                    let ks2 = state.key_store.lock().await;
                    if let Some(ref s) = *ks2 {
                        let _ = s.upsert_peer(&pk, &peer_fingerprint, None);
                    }
                }

                crate::commands::network::spawn_receive_loop(
                    app_handle,
                    state.inner().clone(),
                    read_half,
                    actual_peer_key.clone(),
                    None,
                );

                return Ok(ConnectionInfo {
                    state: "established".to_string(),
                    peer_fingerprint: Some(peer_fingerprint),
                    peer_verified: true,
                    peer_key_hex: Some(actual_peer_key),
                });
            }
            Err(_) => {
                // Connection failed — address is stale
                return Err("CANNOT_REACH".to_string());
            }
        }
    }

    Err("CANNOT_REACH".to_string())
}

/// Update a family member with a fresh invite (new key + address).
/// Replaces everything except nickname and expiry.
#[tauri::command]
pub async fn update_family_member(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    invite_str: String,
) -> Result<FamilyMember, String> {
    let old_key = util::decode_peer_key(&peer_key_hex)
        .map_err(|e| format!("invalid peer key: {e}"))?;

    // Validate the invite to extract the new peer key and address
    let signed = crate::identity::validate_invite(&invite_str)
        .map_err(|e| format!("invalid invite: {e}"))?;

    let new_public_key = signed.payload.identity_pub;
    let new_address = signed.payload.address_hint.clone();

    let sk = state.storage_key.read().await;
    let ks = state.key_store.lock().await;
    let store = ks.as_ref().ok_or("key store not initialized")?;

    let updated = store.update_family_member(&old_key, &new_public_key, Some(&new_address), sk.as_ref())
        .map_err(|e| format!("failed to update family member: {e}"))?;

    Ok(updated)
}

// ─── Identity Export/Import ────────────────────────────────────────────────

/// Export identity + family list to an encrypted file.
#[tauri::command]
pub async fn export_identity(
    state: State<'_, Arc<AppState>>,
    path: String,
    passphrase: String,
) -> Result<(), String> {
    if passphrase.len() < 12 {
        return Err("passphrase must be at least 12 characters".to_string());
    }
    let entropy = util::estimate_passphrase_entropy(&passphrase);
    if entropy < 40.0 {
        return Err(format!(
            "passphrase too weak: ~{:.0} bits. Use a stronger passphrase (aim for 60+).",
            entropy
        ));
    }

    // Get identity from state
    let identity = state.identity.read().await;
    let kp = identity.as_ref().ok_or("vault not unlocked — unlock first")?;

    let pub_bytes = kp.public_key_bytes();
    let sk_bytes = kp.secret_key_bytes();

    // Get family list
    let sk = state.storage_key.read().await;
    let ks = state.key_store.lock().await;
    let store = ks.as_ref().ok_or("key store not initialized")?;
    let family = store.list_family_all(sk.as_ref()).map_err(|e| format!("list family: {e}"))?;
    drop(ks);

    // Encrypt the secret key with export passphrase
    let export_key = derive_key_blocking(passphrase, pub_bytes.to_vec()).await?;
    let (nonce, encrypted_sk) = util::crypto_encrypt_storage(&sk_bytes, &export_key, crate::commands::util::AAD_EXPORT_V2)
        .map_err(|e| format!("encryption failed: {e}"))?;

    // Build the export payload
    let payload = serde_json::json!({
        "version": 1,
        "created_at": chrono::Utc::now().timestamp(),
        "identity": {
            "public_key": STANDARD.encode(pub_bytes),
            "encrypted_secret_key": STANDARD.encode(&encrypted_sk),
            "nonce": STANDARD.encode(&nonce),
        },
        "family": family.iter().map(|m| serde_json::json!({
            "public_key": m.public_key_hex,
            "nickname": m.nickname,
            "added_at": m.added_at,
            "expires_at": m.expires_at,
            "last_address": m.last_address,
        })).collect::<Vec<_>>(),
    });

    let payload_bytes = serde_json::to_vec(&payload)
        .map_err(|e| format!("serialization failed: {e}"))?;

    // Write: nonce || ciphertext
    std::fs::write(&path, &payload_bytes)
        .map_err(|e| format!("failed to write export file: {e}"))?;

    Ok(())
}

/// Import identity + family list from an encrypted file.
///
/// The backup passphrase doubles as the initial vault passphrase: the imported
/// secret key is re-sealed under Argon2id(passphrase, salt = public key) —
/// never under the deprecated publicly-computable legacy key — and registered
/// as a vault account, so the vault is unlocked and initialized immediately.
#[tauri::command]
pub async fn import_identity(
    state: State<'_, Arc<AppState>>,
    path: String,
    passphrase: String,
) -> Result<IdentityInfo, String> {
    // The passphrase becomes the vault passphrase for the imported identity,
    // so it must meet the same strength requirements as unlock_vault.
    if passphrase.len() < 12 {
        return Err(
            "passphrase must be at least 12 characters — longer is more secure".to_string(),
        );
    }
    let entropy = util::estimate_passphrase_entropy(&passphrase);
    if entropy < 40.0 {
        return Err(format!(
            "passphrase too weak: ~{:.0} bits of entropy. \
             Use a longer passphrase (aim for 60+ bits). \
             Try a diceware phrase with 5+ random words.",
            entropy
        ));
    }

    let data = std::fs::read(&path)
        .map_err(|e| format!("failed to read import file: {e}"))?;

    // Parse JSON payload
    let payload: serde_json::Value = serde_json::from_slice(&data)
        .map_err(|_| "invalid or corrupted backup file".to_string())?;

    let identity_obj = payload.get("identity")
        .ok_or("invalid backup: missing identity data")?;

    let pub_bytes_base64 = identity_obj.get("public_key")
        .and_then(|v| v.as_str())
        .ok_or("invalid backup: missing public_key")?;
    let enc_sk_base64 = identity_obj.get("encrypted_secret_key")
        .and_then(|v| v.as_str())
        .ok_or("invalid backup: missing encrypted_secret_key")?;
    let nonce_base64 = identity_obj.get("nonce")
        .and_then(|v| v.as_str())
        .ok_or("invalid backup: missing nonce")?;

    let pub_bytes = STANDARD.decode(pub_bytes_base64)
        .map_err(|_| "invalid backup: corrupted public_key")?;
    let enc_sk = STANDARD.decode(enc_sk_base64)
        .map_err(|_| "invalid backup: corrupted encrypted_secret_key")?;
    let nonce = STANDARD.decode(nonce_base64)
        .map_err(|_| "invalid backup: corrupted nonce")?;

    // Derive key from passphrase + public key
    let pub_arr = {
        let mut arr = [0u8; 32];
        if pub_bytes.len() != 32 {
            return Err("invalid public key length".to_string());
        }
        arr.copy_from_slice(&pub_bytes);
        arr
    };

    let export_key = derive_key_blocking(passphrase, pub_arr.to_vec()).await?;
    let mut sk_bytes = util::crypto_decrypt_storage(&enc_sk, &nonce, &export_key, crate::commands::util::AAD_EXPORT_V2)
        .map_err(|_| "wrong export passphrase or corrupted backup file".to_string())?;

    let mut sk_arr = [0u8; 64];
    if sk_bytes.len() != 64 {
        sk_bytes.zeroize();
        return Err("invalid secret key length in backup".to_string());
    }
    sk_arr.copy_from_slice(&sk_bytes);
    sk_bytes.zeroize();

    // Reconstruct keypair
    let kp = match IdentityKeypair::from_bytes(&pub_arr, &sk_arr) {
        Ok(kp) => kp,
        Err(e) => {
            sk_arr.zeroize();
            return Err(format!("failed to reconstruct identity: {e}"));
        }
    };

    let fingerprint = kp.fingerprint();
    let pub_hex = hex::encode(&pub_bytes);

    // Store to vault
    let data_dir = storage::ensure_data_dir()
        .map_err(|e| format!("data dir error: {e}"))?;
    let keys_db_path = data_dir.join("keys.db");
    let key_store = KeyStore::open(&keys_db_path)
        .map_err(|e| format!("key store error: {e}"))?;

    // Seal the private key under Argon2id(passphrase, salt = public key) —
    // the exact derivation unlock_vault uses for account lookup — so the
    // imported identity is protected at rest and unlocks normally on the
    // next unlock screen. NEVER seal under derive_storage_key(&pub_bytes):
    // that key is SHA-256 of a public value and anyone with keys.db can
    // compute it (H1).
    //
    // The export file was sealed with AAD_EXPORT_V2 and keys.db uses
    // AAD_KEY_STORE, so reusing the derived key material for both domains
    // stays cryptographically domain-separated.
    let storage_key = export_key;

    seal_imported_identity(&key_store, &pub_bytes, &sk_arr, &storage_key)?;
    sk_arr.zeroize();

    let now = chrono::Utc::now().timestamp();

    // Import family members
    if let Some(family_arr) = payload.get("family").and_then(|v| v.as_array()) {
        key_store.clear_family().ok(); // Clear existing family
        for entry in family_arr {
            let member_pk_hex = entry.get("public_key")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let nickname = entry.get("nickname")
                .and_then(|v| v.as_str())
                .unwrap_or("Imported");
            let added_at = entry.get("added_at")
                .and_then(|v| v.as_i64())
                .unwrap_or(now);
            let expires_at = entry.get("expires_at").and_then(|v| v.as_i64());
            let last_address = entry.get("last_address").and_then(|v| v.as_str());

            if let Ok(pk) = util::decode_peer_key(member_pk_hex) {
                let _ = key_store.insert_family_member_raw(
                    &pk, nickname, added_at, expires_at, last_address,
                    Some(&storage_key),
                );
            }
        }
    }

    // Load into state
    {
        let mut id_lock = state.identity.write().await;
        *id_lock = Some(kp);
    }
    {
        let mut sk_lock = state.storage_key.write().await;
        *sk_lock = Some(storage_key);
    }
    {
        let mut vi = state.vault_initialized.write().await;
        *vi = true;
    }
    {
        let mut vu = state.vault_unlocked.write().await;
        *vu = true;
    }
    {
        let mut ks = state.key_store.lock().await;
        *ks = Some(key_store);
    }

    // Initialize message store
    let msgs_db_path = data_dir.join("messages.db");
    let msg_store = storage::MessageStore::open(&msgs_db_path)
        .map_err(|e| format!("message store error: {e}"))?;
    {
        let mut ms = state.message_store.lock().await;
        *ms = Some(msg_store);
    }

    // Initialize transfer store
    let transfers_db_path = data_dir.join("transfers.db");
    let transfer_store = storage::TransferStore::open(&transfers_db_path)
        .map_err(|e| format!("transfer store error: {e}"))?;
    {
        let mut ts = state.transfer_store.lock().await;
        *ts = Some(transfer_store);
    }

    Ok(IdentityInfo {
        fingerprint,
        public_key_hex: pub_hex,
        has_identity: true,
    })
}

/// Seal an imported identity's secret key into the key store under a
/// passphrase-derived storage key (H1).
///
/// This is the single write path for imported identities. It must NEVER use
/// `util::derive_storage_key(&pub_bytes)`: that legacy key is SHA-256 of the
/// public identity key, so anyone with keys.db can compute it and any
/// ciphertext sealed under it is plaintext-equivalent at rest.
///
/// Persists three things:
/// 1. The identity row (public key + wrapped secret key)
/// 2. The vault-initialized flag (the passphrase is now set)
/// 3. An account row labeled "Imported" so multi-account unlock can select
///    it by passphrase — or, if this public key already has an account row,
///    refreshes that row's wrapped secret key instead.
fn seal_imported_identity(
    key_store: &KeyStore,
    pub_bytes: &[u8],
    sk_bytes: &[u8],
    storage_key: &crate::secure_key::StorageKey,
) -> Result<(), String> {
    let (new_nonce, new_enc_sk) =
        util::crypto_encrypt_storage(sk_bytes, storage_key, util::AAD_KEY_STORE)
            .map_err(|e| format!("encryption failed: {e}"))?;

    let now = chrono::Utc::now().timestamp();
    key_store
        .store_identity(pub_bytes, &new_enc_sk, &new_nonce, now)
        .map_err(|e| format!("failed to store identity: {e}"))?;
    key_store
        .set_vault_initialized()
        .map_err(|e| format!("failed to mark vault initialized: {e}"))?;
    // insert_account fails only on the UNIQUE(public_key) conflict — i.e.
    // this identity was already registered as an account. Refresh it.
    if key_store
        .insert_account(pub_bytes, &new_enc_sk, &new_nonce, Some("Imported"), now)
        .is_err()
    {
        key_store
            .update_account_private_key(pub_bytes, &new_enc_sk, &new_nonce)
            .map_err(|e| format!("failed to persist imported account: {e}"))?;
    }
    Ok(())
}

/// Duress wipe: zeroize every in-memory secret, close all stores, and
/// delete every local database + persisted config. Called ONLY from the
/// duress path in `unlock_vault`. Deleting keys.db destroys every wrapped
/// per-message content key (H7), so message ciphertext on disk becomes
/// unrecoverable regardless of remnants.
async fn execute_duress_wipe(state: &Arc<AppState>) {
    // 1. Drop store handles first so SQLite releases file locks.
    {
        state.key_store.lock().await.take();
        state.message_store.lock().await.take();
        state.transfer_store.lock().await.take();
    }

    // 2. Zeroize all in-memory secrets (Drop impls zeroize).
    state.identity.write().await.take();
    state.x25519_identity.write().await.take();
    state.active_signed_prekey.write().await.take();
    state.active_one_time_prekey.write().await.take();
    state.storage_key.write().await.take();

    // 3. Mark locked/initialized so a subsequent launch is a clean first-run.
    *state.vault_unlocked.write().await = false;
    *state.vault_initialized.write().await = false;

    // 4. Delete databases (+ WAL/SHM sidecars) and the security config.
    let dir = std::path::Path::new(&state.data_dir);
    const FILES: &[&str] = &[
        "keys.db", "keys.db-wal", "keys.db-shm",
        "messages.db", "messages.db-wal", "messages.db-shm",
        "transfers.db", "transfers.db-wal", "transfers.db-shm",
        "security.json",
    ];
    let mut removed = 0;
    for name in FILES {
        match std::fs::remove_file(dir.join(name)) {
            Ok(()) => removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::error!(file = name, error = %e, "duress wipe: failed to remove file"),
        }
    }
    tracing::warn!(files_removed = removed, "duress wipe complete");
}

/// Lock the vault — zeroizes keys in memory and marks vault as locked.
///
/// After calling this, the user must unlock the vault again to perform
/// sensitive operations. Active connections remain open.
#[tauri::command]
pub async fn lock_vault(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // Zeroize storage key
    let mut sk = state.storage_key.write().await;
    sk.take(); // Drop + zeroize via StorageKey's Drop impl

    // Drop key store (closes database connection)
    let mut ks = state.key_store.lock().await;
    *ks = None;
    drop(ks);

    // Drop message store
    let mut ms = state.message_store.lock().await;
    *ms = None;
    drop(ms);

    // Drop transfer store
    let mut ts = state.transfer_store.lock().await;
    *ts = None;
    drop(ts);

    // Mark vault as locked
    let mut vu = state.vault_unlocked.write().await;
    *vu = false;
    drop(vu);

    tracing::info!("Vault locked — keys zeroized, stores closed");
    Ok(())
}

/// Register the duress passphrase (coercion resistance). Stores only an
/// Argon2id hash. Entering it at unlock silently WIPES all local data and
/// reports a normal wrong-passphrase error.
///
/// IRREVERSIBLE by design — the frontend must show an explicit confirmation
/// before calling this.
#[tauri::command]
pub async fn set_duress_passphrase(
    state: State<'_, Arc<AppState>>,
    passphrase: String,
) -> Result<(), String> {
    let unlocked = *state.vault_unlocked.read().await;
    if !unlocked {
        return Err("vault must be unlocked to register a duress passphrase".to_string());
    }
    // Same strength gates as unlock: the duress passphrase must be able to
    // pass them too, or it could never trigger (unlock checks run first).
    if passphrase.len() < 12 {
        return Err("duress passphrase must be at least 12 characters".to_string());
    }
    let entropy = util::estimate_passphrase_entropy(&passphrase);
    if entropy < 40.0 {
        return Err(format!("duress passphrase too weak: ~{:.0} bits", entropy));
    }

    let ks_guard = state.key_store.lock().await;
    let key_store = ks_guard.as_ref().ok_or("key store not initialized")?;
    let ks_ref: &storage::KeyStore = key_store;
    crate::duress::register(ks_ref, &passphrase)?;
    tracing::info!("duress passphrase registered");
    Ok(())
}

/// Remove the duress passphrase registration.
#[tauri::command]
pub async fn clear_duress_passphrase(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let unlocked = *state.vault_unlocked.read().await;
    if !unlocked {
        return Err("vault must be unlocked to change duress settings".to_string());
    }
    let ks_guard = state.key_store.lock().await;
    let key_store = ks_guard.as_ref().ok_or("key store not initialized")?;
    crate::duress::clear(key_store)
}

/// Whether a duress passphrase is registered (UI display only).
#[tauri::command]
pub async fn is_duress_configured(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let unlocked = *state.vault_unlocked.read().await;
    if !unlocked {
        return Ok(false);
    }
    let ks_guard = state.key_store.lock().await;
    match ks_guard.as_ref() {
        Some(key_store) => Ok(crate::duress::is_set(key_store)),
        None => Ok(false),
    }
}

/// Check if this is the first launch (onboarding not yet shown).
#[tauri::command]
pub async fn is_first_run(state: State<'_, Arc<AppState>>) -> Result<bool, String> {    let fr = state.first_run.read().await;
    Ok(*fr)
}

/// Mark first-run onboarding as complete.
#[tauri::command]
pub async fn set_first_run_complete(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut fr = state.first_run.write().await;
    *fr = false;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const TEST_PASSPHRASE: &str = "correct-horse-battery-staple-clock";
    const TEST_PUB: [u8; 32] = [0x42; 32];
    const TEST_SK: [u8; 64] = [0x77; 64];

    fn passphrase_key() -> crate::secure_key::StorageKey {
        util::derive_storage_key_from_passphrase(TEST_PASSPHRASE, &TEST_PUB).unwrap()
    }

    /// H1 regression: the imported identity must NOT be decryptable with the
    /// publicly-computable legacy key (SHA-256 of context + public key).
    /// An attacker who steals keys.db can compute that key from the public
    /// `identity.public_key` column alone.
    #[test]
    fn test_imported_identity_not_recoverable_with_legacy_key() {
        let key_store = KeyStore::open(Path::new(":memory:")).unwrap();
        let storage_key = passphrase_key();

        seal_imported_identity(&key_store, &TEST_PUB, &TEST_SK, &storage_key).unwrap();

        let (_pub_loaded, enc, nonce) = key_store.load_identity().unwrap();

        let legacy_key = util::derive_storage_key(&TEST_PUB);
        assert!(
            util::crypto_decrypt_storage(&enc, &nonce, &legacy_key, util::AAD_KEY_STORE).is_err(),
            "imported secret key was sealed under the publicly-computable legacy key"
        );
    }

    /// H1 regression: the passphrase-derived key (the same derivation
    /// unlock_vault uses for account lookup) must recover the exact
    /// secret-key bytes — round-trip integrity.
    #[test]
    fn test_imported_identity_roundtrips_with_passphrase_key() {
        let key_store = KeyStore::open(Path::new(":memory:")).unwrap();
        let storage_key = passphrase_key();

        seal_imported_identity(&key_store, &TEST_PUB, &TEST_SK, &storage_key).unwrap();

        let (_pub_loaded, enc, nonce) = key_store.load_identity().unwrap();
        let recovered =
            util::crypto_decrypt_storage(&enc, &nonce, &storage_key, util::AAD_KEY_STORE).unwrap();
        assert_eq!(recovered, TEST_SK.to_vec());
    }

    /// Sealing an import must mark the vault initialized and register an
    /// account row so multi-account unlock can select it by passphrase.
    #[test]
    fn test_import_registers_account_and_marks_vault_initialized() {
        let key_store = KeyStore::open(Path::new(":memory:")).unwrap();
        let storage_key = passphrase_key();

        seal_imported_identity(&key_store, &TEST_PUB, &TEST_SK, &storage_key).unwrap();

        assert!(key_store.is_vault_initialized().unwrap());
        let accounts = key_store.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].public_key, TEST_PUB.to_vec());
        assert_eq!(accounts[0].label.as_deref(), Some("Imported"));
    }

    /// Re-importing the same identity must not duplicate the account row —
    /// the existing row's wrapped key is refreshed instead.
    #[test]
    fn test_reimport_refreshes_existing_account_row() {
        let key_store = KeyStore::open(Path::new(":memory:")).unwrap();
        let storage_key = passphrase_key();

        seal_imported_identity(&key_store, &TEST_PUB, &TEST_SK, &storage_key).unwrap();
        // Second import with a different wrapping (fresh encryption) — same pubkey.
        seal_imported_identity(&key_store, &TEST_PUB, &TEST_SK, &passphrase_key()).unwrap();

        // The refreshed account blob must still decrypt under the new wrap.
        let accounts = key_store.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        let recovered = util::crypto_decrypt_storage(
            &accounts[0].encrypted_private_key,
            &accounts[0].private_key_nonce,
            &storage_key,
            util::AAD_KEY_STORE,
        )
        .unwrap();
        assert_eq!(recovered, TEST_SK.to_vec());
    }
}
