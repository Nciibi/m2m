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

use super::util;
use super::{ConnectionEvent, ConnectionInfo, FamilyMember, IdentityInfo, VaultStatus};

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

    let _data_dir = storage::ensure_data_dir()
        .map_err(|e| format!("data dir error: {e}"))?;
    // Note: messages.db and transfers.db paths are used by
    // ensure_message_store / ensure_transfer_store lazy init in chat.rs/state.rs

    // Access the key store that init_identity opened
    let ks_guard = state.key_store.lock().await;
    let key_store = ks_guard
        .as_ref()
        .ok_or("key store not initialized — call init_identity first")?;

    let vault_was_initialized = key_store.is_vault_initialized().unwrap_or(false);
    let has_identity = key_store.has_identity().unwrap_or(false);

    // Pre-read X25519 existence before any async work (key_store is !Send across .await).
    let has_x25519 = has_identity && key_store.has_x25519_key().unwrap_or(false);

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
            let sk_bytes = util::crypto_decrypt_storage(&enc_sk, &nonce, &storage_key, util::AAD_KEY_STORE)
                .map_err(|_| "incorrect passphrase or corrupted data".to_string())?;

            let mut sk_arr = [0u8; 64];
            sk_arr.copy_from_slice(&sk_bytes);

            // Pre-read X25519 encrypted material (synchronous, no .await)
            let x25519_preload = if has_x25519 {
                match key_store.load_x25519_key() {
                    Ok((xp, xe, xn)) => Some((xp, xe, xn)),
                    Err(e) => return Err(format!("failed to load X25519 key: {e}")),
                }
            } else {
                None
            };

            {
                let mut sk_lock = state.storage_key.write().await;
                *sk_lock = Some(storage_key);
            }

            // Now handle X25519 (storage_key is in state)
            let x25519_kp = if let Some((x_pub, x_enc, x_nonce)) = x25519_preload {
                let sk = state.storage_key.read().await;
                let st_key = sk.as_ref().ok_or("storage key not set")?;
                let x_sk_bytes = util::crypto_decrypt_storage(&x_enc, &x_nonce, st_key, util::AAD_KEY_STORE)
                    .map_err(|_| "failed to decrypt X25519 key".to_string())?;
                drop(sk);
                let mut x_sk_arr = [0u8; 32];
                x_sk_arr.copy_from_slice(&x_sk_bytes);
                crate::crypto::X25519IdentityKeypair::from_bytes(&x_pub, &x_sk_arr)
                    .map_err(|e| format!("failed to reconstruct X25519: {e}"))?
            } else {
                // First unlock after upgrade: generate X25519 key
                let xkp = crate::crypto::X25519IdentityKeypair::generate();
                let x_sk_bytes = xkp.secret_key_bytes();
                let x_pub = xkp.public_key_bytes();
                let sk = state.storage_key.read().await;
                let st_key = sk.as_ref().ok_or("storage key not set")?;
                let (x_nonce, x_enc) = util::crypto_encrypt_storage(&x_sk_bytes, st_key, util::AAD_KEY_STORE)
                    .map_err(|e| format!("failed to encrypt X25519 key: {e}"))?;
                drop(sk);
                key_store.store_x25519_key(&x_pub, &x_enc, &x_nonce)
                    .map_err(|e| format!("failed to store X25519 key: {e}"))?;
                xkp
            };
            {
                let mut x_lock = state.x25519_identity.write().await;
                *x_lock = Some(x25519_kp);
            }

            IdentityKeypair::from_bytes(&pub_arr, &sk_arr)
                .map_err(|e| format!("failed to reconstruct identity: {e}"))?
        } else {
            // Case 2: Legacy migration — decrypt with legacy key, re-encrypt with Argon2id
            tracing::warn!("migrating legacy identity to vault — setting passphrase for first time");
            let legacy_key = util::derive_storage_key(&pub_bytes);
            let sk_bytes = util::crypto_decrypt_storage(&enc_sk, &nonce, &legacy_key, b"")
                .map_err(|e| format!("failed to decrypt legacy identity: {e}"))?;

            let mut sk_arr = [0u8; 64];
            sk_arr.copy_from_slice(&sk_bytes);

            // Re-encrypt with the new passphrase-derived key AND domain AAD.
            let new_key = util::derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
            let (new_nonce, new_enc_sk) = util::crypto_encrypt_storage(&sk_bytes, &new_key, util::AAD_KEY_STORE)
                .map_err(|e| format!("failed to re-encrypt identity: {e}"))?;

            key_store
                .update_encrypted_private_key(&new_enc_sk, &new_nonce)
                .map_err(|e| format!("failed to update identity: {e}"))?;
            key_store
                .set_vault_initialized()
                .map_err(|e| format!("failed to mark vault initialized: {e}"))?;

            {
                let mut sk_lock = state.storage_key.write().await;
                *sk_lock = Some(new_key);
            }

            // Generate X25519 key for new sessions
            let xkp = crate::crypto::X25519IdentityKeypair::generate();
            let x_sk_bytes = xkp.secret_key_bytes();
            let x_pub = xkp.public_key_bytes();
            let sk_ref = state.storage_key.read().await;
            let current_key = sk_ref.as_ref().ok_or("storage key not set")?;
            let (x_nonce, x_enc) = util::crypto_encrypt_storage(&x_sk_bytes, current_key, util::AAD_KEY_STORE)
                .map_err(|e| format!("failed to encrypt X25519 key: {e}"))?;
            drop(sk_ref);
            key_store.store_x25519_key(&x_pub, &x_enc, &x_nonce)
                .map_err(|e| format!("failed to store X25519 key: {e}"))?;
            {
                let mut x_lock = state.x25519_identity.write().await;
                *x_lock = Some(xkp);
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
        let (nonce, encrypted_sk) = util::crypto_encrypt_storage(&sk_bytes, &storage_key, util::AAD_KEY_STORE)
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
            *sk_lock = Some(storage_key);
        }

        // Generate and store X25519 key for X3DH
        let xkp = crate::crypto::X25519IdentityKeypair::generate();
        let x_sk_bytes = xkp.secret_key_bytes();
        let x_pub = xkp.public_key_bytes();
        let storage_key_ref = state.storage_key.read().await;
        let sk = storage_key_ref.as_ref().ok_or("storage key not set")?;
        let (x_nonce, x_enc) = util::crypto_encrypt_storage(&x_sk_bytes, sk, util::AAD_KEY_STORE)
            .map_err(|e| format!("failed to encrypt X25519 key: {e}"))?;
        drop(storage_key_ref);
        key_store.store_x25519_key(&x_pub, &x_enc, &x_nonce)
            .map_err(|e| format!("failed to store X25519 key: {e}"))?;
        {
            let mut x_lock = state.x25519_identity.write().await;
            *x_lock = Some(xkp);
        }

        kp
    };

    // Drop key_store lock before acquiring other locks
    drop(ks_guard);

    // Store the full keypair in state
    // NOTE: MessageStore and TransferStore are opened lazily on first use
    // (see load_messages, send_message, file transfer commands). This keeps
    // vault unlock fast and avoids unnecessary DB opens when the user only
    // changes settings without loading messages.
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

// ─── Family Commands ───────────────────────────────────────────────────────

/// List all non-expired family members.
#[tauri::command]
pub async fn list_family(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<FamilyMember>, String> {
    let ks = state.key_store.lock().await;
    let store = ks.as_ref().ok_or("key store not initialized")?;
    store.list_family().map_err(|e| format!("failed to list family: {e}"))
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
        let ks = state.key_store.lock().await;
        let store = ks.as_ref().ok_or("key store not initialized")?;
        store.add_family_member(&pk_bytes, &nickname, expires_in_days, None)
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
        let ks = state.key_store.lock().await;
        let store = ks.as_ref().ok_or("key store not initialized")?;
        store.set_family_nickname(&pk_bytes, &nickname)
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
        let ks = state.key_store.lock().await;
        let store = ks.as_ref().ok_or("key store not initialized")?;
        if !store.is_family_member(&pk_bytes).map_err(|e| format!("family check: {e}"))? {
            return Err("peer is not a family member".to_string());
        }
        let members = store.list_family().map_err(|e| format!("list family: {e}"))?;
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

    let ks = state.key_store.lock().await;
    let store = ks.as_ref().ok_or("key store not initialized")?;

    let updated = store.update_family_member(&old_key, &new_public_key, Some(&new_address))
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
    let ks = state.key_store.lock().await;
    let store = ks.as_ref().ok_or("key store not initialized")?;
    let family = store.list_family_all().map_err(|e| format!("list family: {e}"))?;
    drop(ks);

    // Encrypt the secret key with export passphrase
    let export_key = util::derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
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
#[tauri::command]
pub async fn import_identity(
    state: State<'_, Arc<AppState>>,
    path: String,
    passphrase: String,
) -> Result<IdentityInfo, String> {
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

    let export_key = util::derive_storage_key_from_passphrase(&passphrase, &pub_arr)?;
    let sk_bytes = util::crypto_decrypt_storage(&enc_sk, &nonce, &export_key, crate::commands::util::AAD_EXPORT_V2)
        .map_err(|_| "wrong export passphrase or corrupted backup file".to_string())?;

    let sk_arr = {
        let mut arr = [0u8; 64];
        if sk_bytes.len() != 64 {
            return Err("invalid secret key length in backup".to_string());
        }
        arr.copy_from_slice(&sk_bytes);
        arr
    };

    // Reconstruct keypair
    let kp = IdentityKeypair::from_bytes(&pub_arr, &sk_arr)
        .map_err(|e| format!("failed to reconstruct identity: {e}"))?;

    let fingerprint = kp.fingerprint();
    let pub_hex = hex::encode(&pub_bytes);

    // Store to vault
    let data_dir = storage::ensure_data_dir()
        .map_err(|e| format!("data dir error: {e}"))?;
    let keys_db_path = data_dir.join("keys.db");
    let key_store = KeyStore::open(&keys_db_path)
        .map_err(|e| format!("key store error: {e}"))?;

    // Encrypt the private key with a storage key derived from the public key.
    // The user will set a vault passphrase on next unlock.
    let storage_key = util::derive_storage_key(&pub_bytes);

    let (new_nonce, new_enc_sk) = util::crypto_encrypt_storage(&sk_bytes, &storage_key, util::AAD_KEY_STORE)
        .map_err(|e| format!("encryption failed: {e}"))?;

    let now = chrono::Utc::now().timestamp();
    key_store.store_identity(&pub_bytes, &new_enc_sk, &new_nonce, now)
        .map_err(|e| format!("failed to store identity: {e}"))?;

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

/// Check if this is the first launch (onboarding not yet shown).
#[tauri::command]
pub async fn is_first_run(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let fr = state.first_run.read().await;
    Ok(*fr)
}

/// Mark first-run onboarding as complete.
#[tauri::command]
pub async fn set_first_run_complete(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut fr = state.first_run.write().await;
    *fr = false;
    Ok(())
}
