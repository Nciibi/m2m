//! Chat messaging and conversation management commands.

use std::sync::Arc;

use tauri::State;

use crate::protocol::{self};
use crate::state::{AppState, PeerConnection};

use super::util;
use super::{ChatMessage, ConversationListItem};

use crate::protocol::MessageReactionData;

/// Send a text message to a connected peer.
#[tauri::command]
pub async fn send_message(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    content: String,
) -> Result<ChatMessage, String> {
    if content.len() > protocol::MAX_TEXT_MESSAGE_SIZE {
        return Err(format!(
            "message too large: {} bytes exceeds {} byte limit",
            content.len(),
            protocol::MAX_TEXT_MESSAGE_SIZE
        ));
    }

    let conns = state.connections.read().await;
    let conn_arc = conns
        .get(&peer_key_hex)
        .ok_or("no connection to this peer")?
        .clone();

    let mut conn = conn_arc.lock().await;
    let msg_id = {
        let PeerConnection { session, write_half, .. } = &mut *conn;
        session
            .send_text(write_half, &content)
            .await
            .map_err(|e| format!("send failed: {e}"))?
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Persist message to local storage if history is enabled
    let history = *state.history_enabled.read().await;
    if history {
        let sk = state.storage_key.read().await;
        let ms = state.message_store.lock().await;
        if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
            match util::crypto_encrypt_storage(content.as_bytes(), key, util::AAD_MSG_STORE) {
                Ok((nonce, encrypted)) => {
                    if let Some(peer_bytes) = util::decode_peer_key_logged(&peer_key_hex) {
                        let _ = store.ensure_conversation(&peer_key_hex, &peer_bytes);
                        let _ = store.store_message(
                            &msg_id, &peer_key_hex, "sent",
                            &encrypted, &nonce, now as i64,
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to encrypt message for storage — message NOT persisted");
                }
            }
        }
    }

    Ok(ChatMessage::new(msg_id, content, "sent".to_string(), now))
}

/// Load message history for a peer.
#[tauri::command]
pub async fn load_messages(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessage>, String> {
    let sk = state.storage_key.read().await;
    let ms = state.message_store.lock().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    let key = sk.as_ref().ok_or("storage key not available")?;

    let stored = store
        .load_messages(&peer_key_hex, limit.unwrap_or(100))
        .map_err(|e| format!("failed to load messages: {e}"))?;

    let mut messages = Vec::with_capacity(stored.len());
    let msg_ids: Vec<String> = stored.iter().map(|m| m.id.clone()).collect();

    // Load reactions for all messages at once
    let all_reactions = store.get_reactions(&msg_ids)
        .unwrap_or_default();

    for m in stored {
        let content = util::crypto_decrypt_storage(&m.content_encrypted, &m.content_nonce, key, util::AAD_MSG_STORE)
            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
            .unwrap_or_else(|_| "[encrypted]".to_string());

        // Build reactions map: reaction → [peer_key_hex, ...]
        let mut reactions: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        if let Some(msg_reactions) = all_reactions.get(&m.id) {
            for (rxn, peer, _ts) in msg_reactions {
                reactions.entry(rxn.clone()).or_default().push(peer.clone());
            }
        }

        messages.push(ChatMessage {
            id: m.id,
            content,
            direction: m.direction,
            timestamp: m.timestamp as u64,
            read_at: m.read_at,
            reactions,
        });
    }
    Ok(messages)
}

/// List all conversations with metadata.
#[tauri::command]
pub async fn list_conversations(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ConversationListItem>, String> {
    let conns = state.connections.read().await;
    let sk = state.storage_key.read().await;

    let ms = state.message_store.lock().await;
    let store = match ms.as_ref() {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let convos = store
        .list_conversations()
        .map_err(|e| format!("failed to list conversations: {e}"))?;

    let mut items = Vec::with_capacity(convos.len());
    for c in convos {
        let peer_key_hex = hex::encode(&c.peer_id);
        let is_online = conns.contains_key(&c.id);

        // Try to decrypt the last message for a preview
        let last_message_preview = if let Some(key) = sk.as_ref() {
            store
                .load_messages(&c.id, 1)
                .ok()
                .and_then(|msgs| msgs.into_iter().last())
                .and_then(|m| {
                    util::crypto_decrypt_storage(&m.content_encrypted, &m.content_nonce, key, util::AAD_MSG_STORE)
                        .ok()
                        .map(|bytes| {
                            let text = String::from_utf8_lossy(&bytes).to_string();
                            if text.len() > 80 {
                                format!("{}…", &text[..77])
                            } else {
                                text
                            }
                        })
                })
        } else {
            None
        };

        items.push(ConversationListItem {
            id: c.id,
            peer_key_hex,
            display_name: c.display_name,
            peer_display_name: c.peer_display_name,
            last_message_at: c.last_message_at,
            last_message_preview,
            message_count: c.message_count,
            is_online,
            auto_delete_at: c.auto_delete_at,
            retention_policy: c.retention_policy,
            created_at: c.created_at,
        });
    }

    Ok(items)
}

/// Rename a conversation (local display name).
#[tauri::command]
pub async fn rename_conversation(
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
    display_name: String,
) -> Result<(), String> {
    let ms = state.message_store.lock().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    store
        .rename_conversation(&conversation_id, &display_name)
        .map_err(|e| format!("rename failed: {e}"))
}

/// Delete a conversation and all its messages (securely).
#[tauri::command]
pub async fn delete_conversation_cmd(
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
) -> Result<(), String> {
    let ms = state.message_store.lock().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    store
        .delete_conversation(&conversation_id)
        .map_err(|e| format!("delete failed: {e}"))
}

/// Set per-conversation retention policy.
/// `policy`: "none", "delete", or "export"
/// `duration_secs`: seconds until auto-action (null for "none")
#[tauri::command]
pub async fn set_conversation_retention(
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
    policy: String,
    duration_secs: Option<i64>,
) -> Result<(), String> {
    let valid_policies = ["none", "delete", "export"];
    if !valid_policies.contains(&policy.as_str()) {
        return Err(format!("invalid policy: {policy}. Must be one of: none, delete, export"));
    }
    let ms = state.message_store.lock().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    store
        .set_conversation_retention(&conversation_id, &policy, duration_secs)
        .map_err(|e| format!("retention update failed: {e}"))
}

/// Send conversation naming metadata to a connected peer.
/// This tells the peer what name we chose for ourselves and what name we suggest for them.
#[tauri::command]
pub async fn send_conversation_names(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    my_name: String,
    their_name: String,
) -> Result<(), String> {
    let conns = state.connections.read().await;
    let conn_arc = conns
        .get(&peer_key_hex)
        .ok_or("no connection to this peer")?
        .clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;
    session
        .send_conversation_meta(&mut *write_half, &my_name, &their_name)
        .await
        .map_err(|e| format!("failed to send conversation meta: {e}"))
}

/// Export a conversation as an encrypted JSON file.
/// The export is encrypted with the same storage key (XChaCha20-Poly1305)
/// so it can only be read by someone with the vault passphrase.
#[tauri::command]
pub async fn export_conversation(
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
    export_path: String,
) -> Result<String, String> {
    let sk = state.storage_key.read().await;
    let ms = state.message_store.lock().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    let key = sk.as_ref().ok_or("storage key not available")?;

    // Get conversation metadata
    let conv = store
        .get_conversation(&conversation_id)
        .map_err(|e| format!("failed to get conversation: {e}"))?
        .ok_or("conversation not found")?;

    // Load all messages
    let messages = store
        .export_conversation_messages(&conversation_id)
        .map_err(|e| format!("failed to export messages: {e}"))?;

    // Build the export payload (messages stay encrypted — the export
    // is a faithful copy of the encrypted blobs plus metadata)
    let export_data = serde_json::json!({
        "version": "m2m-export-v1",
        "conversation_id": conversation_id,
        "display_name": conv.display_name,
        "peer_display_name": conv.peer_display_name,
        "created_at": conv.created_at,
        "exported_at": chrono::Utc::now().timestamp(),
        "message_count": messages.len(),
        "messages": messages.iter().map(|m| {
            serde_json::json!({
                "id": m.id,
                "direction": m.direction,
                "content_encrypted": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &m.content_encrypted,
                ),
                "content_nonce": base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &m.content_nonce,
                ),
                "timestamp": m.timestamp,
            })
        }).collect::<Vec<_>>(),
    });

    // Serialize the JSON, then encrypt the entire export with the storage key
    let export_json = serde_json::to_vec_pretty(&export_data)
        .map_err(|e| format!("serialization failed: {e}"))?;
    let (nonce, ciphertext) = util::crypto_encrypt_storage(&export_json, key, util::AAD_EXPORT)
        .map_err(|e| format!("encryption failed: {e}"))?;

    // Build the final file: nonce (24 bytes) || ciphertext
    let mut file_data = Vec::with_capacity(nonce.len() + ciphertext.len());
    file_data.extend_from_slice(&nonce);
    file_data.extend_from_slice(&ciphertext);

    std::fs::write(&export_path, &file_data)
        .map_err(|e| format!("failed to write export: {e}"))?;

    Ok(export_path)
}
