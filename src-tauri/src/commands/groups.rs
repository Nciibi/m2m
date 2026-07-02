//! Group chat commands (Phase 3).
//!
//! Tauri IPC bridge for group creation, member management,
//! sending/receiving group messages, and group listing.

use std::sync::Arc;

use tauri::{Emitter, State};
use tokio::sync::Mutex;

use crate::group::{Group, GroupManager};
use crate::protocol::{self, GroupCreateData, GroupEncryptedMessageData, GroupInviteData,
    GroupLeaveData, GroupRemoveData, GroupSenderKeyData, PacketType};
use crate::state::{AppState, PeerConnection};

use super::{ChatMessage, GroupEvent, GroupMessageEvent};

/// Create a new group.
/// Generates Sender Keys for all members and distributes them over pairwise DR sessions.
#[tauri::command]
pub async fn create_group(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    group_name: String,
    member_peer_keys: Vec<String>,
) -> Result<super::GroupInfo, String> {
    // Validate: members must exist as contacts
    let our_identity = state.identity.read().await;
    let identity = our_identity.as_ref().ok_or("identity not initialized")?;
    let our_peer_key_hex = hex::encode(identity.public_key_bytes());

    if member_peer_keys.is_empty() {
        return Err("group must have at least one member besides yourself".to_string());
    }

    let group_id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Create group in GroupManager
    let bundles = {
        let mut gm = state.group_manager.write().await;
        let (gid, bundles) = gm.create_group(
            group_id.clone(),
            group_name.clone(),
            now,
            our_peer_key_hex.clone(),
            &member_peer_keys,
        ).map_err(|e| format!("group creation failed: {e}"))?;

        // Persist group to DB
        state.ensure_message_store(&state.data_dir).await
            .map_err(|e| format!("message store init: {e}"))?;
        let ms = state.message_store.lock().await;
        if let Some(store) = ms.as_ref() {
            let _ = store.upsert_group(&gid, &group_name, now as i64, "admin");
            for key in &member_peer_keys {
                let _ = store.add_group_member(&gid, key, None, "member", now as i64);
            }
        }

        bundles
    };

    // Distribute sender key bundles over existing DR sessions
    for (peer_key_hex, bundle_data) in &bundles {
        let serialized = protocol::serialize(bundle_data)
            .map_err(|e| format!("serialization failed: {e}"))?;

        let conns = state.connections.read().await;
        if let Some(conn_arc) = conns.get(peer_key_hex) {
            let mut conn = conn_arc.lock().await;
            let PeerConnection { session, write_half, .. } = &mut *conn;
            if let Err(e) = session
                .send_encrypted_typed(write_half, PacketType::GroupSenderKey, &serialized)
                .await
            {
                tracing::warn!(peer = %peer_key_hex, error = %e, "failed to send group sender key");
            }
        }
    }

    // Also send GroupCreate to each online member
    let create_payload = GroupCreateData {
        group_id: group_id.clone(),
        group_name: group_name.clone(),
        creator_peer_key_hex: our_peer_key_hex.clone(),
        created_at: now,
        initial_members: member_peer_keys.clone(),
    };
    let create_bytes = protocol::serialize(&create_payload)
        .map_err(|e| format!("serialization failed: {e}"))?;

    {
        let conns = state.connections.read().await;
        for member_key in &member_peer_keys {
            if let Some(conn_arc) = conns.get(member_key) {
                let mut conn = conn_arc.lock().await;
                let PeerConnection { session, write_half, .. } = &mut *conn;
                let _ = session
                    .send_encrypted_typed(write_half, PacketType::GroupCreate, &create_bytes)
                    .await;
            }
        }
    }

    // Emit group created event
    let _ = app_handle.emit("m2m://group-event", GroupEvent {
        group_id: group_id.clone(),
        event_type: "created".to_string(),
        peer_key_hex: None,
    });

    Ok(super::GroupInfo {
        group_id,
        group_name,
        member_count: (member_peer_keys.len() + 1) as u32,
        created_at: now,
    })
}

/// Send a message to a group.
/// Encrypts with the sender's Sender Key chain and sends to all online members.
#[tauri::command]
pub async fn send_group_message(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: String,
    content: String,
) -> Result<ChatMessage, String> {
    if content.len() > crate::protocol::MAX_TEXT_MESSAGE_SIZE {
        return Err(format!(
            "message too large: {} bytes exceeds {} byte limit",
            content.len(),
            crate::protocol::MAX_TEXT_MESSAGE_SIZE
        ));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let our_identity = state.identity.read().await;
    let identity = our_identity.as_ref().ok_or("identity not initialized")?;
    let our_peer_key_hex = hex::encode(identity.public_key_bytes());

    let msg_id = uuid::Uuid::new_v4().to_string();

    // Encrypt using group's sender key chain
    let encrypted_data = {
        let mut gm = state.group_manager.write().await;
        let group = gm.get_group_mut(&group_id)
            .ok_or("group not found")?;
        let data = group.encrypt_message(&our_peer_key_hex, content.as_bytes())
            .map_err(|e| format!("encryption failed: {e}"))?;
        data
    };

    // Send to all online group members over their DR sessions
    let members: Vec<String> = {
        let gm = state.group_manager.read().await;
        let group = gm.get_group(&group_id)
            .ok_or("group not found")?;
        group.members.iter()
            .filter(|m| m.peer_key_hex != our_peer_key_hex)
            .map(|m| m.peer_key_hex.clone())
            .collect()
    };

    let serialized = protocol::serialize(&encrypted_data)
        .map_err(|e| format!("serialization failed: {e}"))?;

    let mut delivered_count = 0u32;
    {
        let conns = state.connections.read().await;
        for member_key in &members {
            if let Some(conn_arc) = conns.get(member_key) {
                let mut conn = conn_arc.lock().await;
                let PeerConnection { session, write_half, .. } = &mut *conn;
                match session
                    .send_encrypted_typed(write_half, PacketType::GroupEncryptedMessage, &serialized)
                    .await
                {
                    Ok(_) => delivered_count += 1,
                    Err(e) => tracing::warn!(peer = %member_key, error = %e, "group message send failed"),
                }
            }
        }
    }

    let delivered = delivered_count > 0;

    // Store in DB (encrypting content for storage)
    state.ensure_message_store(&state.data_dir).await
        .map_err(|e| format!("message store init: {e}"))?;

    let sk = state.storage_key.read().await;
    let ms = state.message_store.lock().await;
    if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
        match super::util::crypto_encrypt_storage(content.as_bytes(), key, super::util::AAD_MSG_STORE) {
            Ok((nonce, encrypted)) => {
                let _ = store.store_group_message(
                    &msg_id, &group_id, &our_peer_key_hex,
                    &encrypted, &nonce, now as i64, delivered,
                );
                let preview = if content.len() > 80 {
                    format!("{}...", &content[..80])
                } else {
                    content.clone()
                };
                let _ = store.update_group_last_message(&group_id, now as i64, &preview);
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to encrypt group message for storage");
            }
        }
    }
    drop(ms);
    drop(sk);

    let message = ChatMessage::new(msg_id, content, "sent".to_string(), now);

    // Emit event for our own UI
    let _ = app_handle.emit("m2m://group-message", GroupMessageEvent {
        group_id,
        message: message.clone(),
    });

    Ok(message)
}

/// List all groups.
#[tauri::command]
pub async fn list_groups(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<super::GroupInfo>, String> {
    let gm = state.group_manager.read().await;
    let groups = gm.list_groups();

    let infos = groups.into_iter().map(|g| super::GroupInfo {
        group_id: g.group_id,
        group_name: g.group_name,
        member_count: g.member_count,
        created_at: g.created_at,
    }).collect();

    Ok(infos)
}

/// Get detailed group info (with members).
#[tauri::command]
pub async fn get_group_info(
    state: State<'_, Arc<AppState>>,
    group_id: String,
) -> Result<super::GroupDetail, String> {
    let gm = state.group_manager.read().await;
    let group = gm.get_group(&group_id)
        .ok_or("group not found")?;

    let our_identity = state.identity.read().await;
    let identity = our_identity.as_ref().ok_or("identity not initialized")?;
    let our_peer_key_hex = hex::encode(identity.public_key_bytes());

    let our_role = if group.is_admin(&our_peer_key_hex) { "admin" } else { "member" };

    let detail = super::GroupDetail {
        group_id: group.group_id.clone(),
        group_name: group.name.clone(),
        member_count: group.member_count(),
        created_at: group.created_at,
        our_role: our_role.to_string(),
        members: group.members.clone(),
    };

    Ok(detail)
}

/// Invite a new member to an existing group.
/// Requires admin privileges.
#[tauri::command]
pub async fn invite_to_group(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: String,
    peer_key_hex: String,
) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let our_identity = state.identity.read().await;
    let identity = our_identity.as_ref().ok_or("identity not initialized")?;
    let our_peer_key_hex = hex::encode(identity.public_key_bytes());

    // Add member in GroupManager
    let bundles = {
        let mut gm = state.group_manager.write().await;
        gm.add_member(&group_id, &peer_key_hex, now)
            .map_err(|e| format!("add member failed: {e}"))?
    };

    // Persist to DB
    state.ensure_message_store(&state.data_dir).await
        .map_err(|e| format!("message store init: {e}"))?;
    let ms = state.message_store.lock().await;
    if let Some(store) = ms.as_ref() {
        let _ = store.add_group_member(&group_id, &peer_key_hex, None, "member", now as i64);
    }
    drop(ms);

    // Send sender key bundles to the new member
    let conns = state.connections.read().await;
    if let Some(conn_arc) = conns.get(&peer_key_hex) {
        let mut conn = conn_arc.lock().await;
        let PeerConnection { session, write_half, .. } = &mut *conn;
        for bundle in &bundles {
            let serialized = protocol::serialize(bundle)
                .map_err(|e| format!("serialization failed: {e}"))?;
            let _ = session
                .send_encrypted_typed(write_half, PacketType::GroupSenderKey, &serialized)
                .await;
        }

        // Send GroupInvite
        let gm_read = state.group_manager.read().await;
        let group = gm_read.get_group(&group_id)
            .ok_or("group not found")?;
        let existing: Vec<String> = group.members.iter()
            .map(|m| m.peer_key_hex.clone())
            .collect();

        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(group_id.as_bytes());
        sign_data.extend_from_slice(&(existing.len() as u32).to_be_bytes());
        let signature = identity.sign(&sign_data);

        let invite = GroupInviteData {
            group_id: group_id.clone(),
            group_name: group.name.clone(),
            inviter_peer_key_hex: our_peer_key_hex.clone(),
            member_count: group.member_count(),
            existing_members: existing,
            signature,
        };
        drop(gm_read);

        let invite_bytes = protocol::serialize(&invite)
            .map_err(|e| format!("serialization failed: {e}"))?;
        let _ = session
            .send_encrypted_typed(write_half, PacketType::GroupInvite, &invite_bytes)
            .await;
    }

    let _ = app_handle.emit("m2m://group-event", GroupEvent {
        group_id,
        event_type: "member_added".to_string(),
        peer_key_hex: Some(peer_key_hex),
    });

    Ok(())
}

/// Remove a member from a group (admin only).
/// Triggers key rotation for remaining members.
#[tauri::command]
pub async fn remove_from_group(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: String,
    peer_key_hex: String,
) -> Result<(), String> {
    let our_identity = state.identity.read().await;
    let identity = our_identity.as_ref().ok_or("identity not initialized")?;
    let our_peer_key_hex = hex::encode(identity.public_key_bytes());

    let bundles = {
        let mut gm = state.group_manager.write().await;
        gm.remove_member(&group_id, &peer_key_hex, &our_peer_key_hex)
            .map_err(|e| format!("remove member failed: {e}"))?
    };

    // Persist removal
    state.ensure_message_store(&state.data_dir).await
        .map_err(|e| format!("message store init: {e}"))?;
    let ms = state.message_store.lock().await;
    if let Some(store) = ms.as_ref() {
        let _ = store.remove_group_member(&group_id, &peer_key_hex);
    }
    drop(ms);

    // Send new sender key bundles to remaining members
    let conns = state.connections.read().await;
    for (member_key, bundle_data) in &bundles {
        let serialized = protocol::serialize(bundle_data)
            .map_err(|e| format!("serialization failed: {e}"))?;

        if let Some(conn_arc) = conns.get(member_key) {
            let mut conn = conn_arc.lock().await;
            let PeerConnection { session, write_half, .. } = &mut *conn;
            let _ = session
                .send_encrypted_typed(write_half, PacketType::GroupSenderKey, &serialized)
                .await;
        }

        // Send GroupRemove notification
        let remove_msg = GroupRemoveData {
            group_id: group_id.clone(),
            removed_peer_key_hex: peer_key_hex.clone(),
            removed_by_peer_key_hex: our_peer_key_hex.clone(),
            new_sender_key: Some(bundle_data.clone()),
        };
        let remove_bytes = protocol::serialize(&remove_msg)
            .map_err(|e| format!("serialization failed: {e}"))?;
        if let Some(conn_arc) = conns.get(member_key) {
            let mut conn = conn_arc.lock().await;
            let PeerConnection { session, write_half, .. } = &mut *conn;
            let _ = session
                .send_encrypted_typed(write_half, PacketType::GroupRemove, &remove_bytes)
                .await;
        }
    }

    let _ = app_handle.emit("m2m://group-event", GroupEvent {
        group_id,
        event_type: "member_removed".to_string(),
        peer_key_hex: Some(peer_key_hex),
    });

    Ok(())
}

/// Leave a group voluntarily.
#[tauri::command]
pub async fn leave_group(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: String,
) -> Result<(), String> {
    let our_identity = state.identity.read().await;
    let identity = our_identity.as_ref().ok_or("identity not initialized")?;
    let our_peer_key_hex = hex::encode(identity.public_key_bytes());

    {
        let mut gm = state.group_manager.write().await;
        gm.leave_group(&group_id, &our_peer_key_hex)
            .map_err(|e| format!("leave group failed: {e}"))?;
    }

    // Persist: remove ourselves as member
    state.ensure_message_store(&state.data_dir).await
        .map_err(|e| format!("message store init: {e}"))?;
    let ms = state.message_store.lock().await;
    if let Some(store) = ms.as_ref() {
        let _ = store.remove_group_member(&group_id, &our_peer_key_hex);
    }
    drop(ms);

    // Send leave notification to online members
    let leave_msg = GroupLeaveData {
        group_id: group_id.clone(),
        leaving_peer_key_hex: our_peer_key_hex.clone(),
    };
    let leave_bytes = protocol::serialize(&leave_msg)
        .map_err(|e| format!("serialization failed: {e}"))?;

    let gm_read = state.group_manager.read().await;
    let group = gm_read.get_group(&group_id);
    let member_keys: Vec<String> = group
        .map(|g| g.members.iter().map(|m| m.peer_key_hex.clone()).collect())
        .unwrap_or_default();
    drop(gm_read);

    let conns = state.connections.read().await;
    for member_key in &member_keys {
        if member_key == &our_peer_key_hex {
            continue;
        }
        if let Some(conn_arc) = conns.get(member_key) {
            let mut conn = conn_arc.lock().await;
            let PeerConnection { session, write_half, .. } = &mut *conn;
            let _ = session
                .send_encrypted_typed(write_half, PacketType::GroupLeave, &leave_bytes)
                .await;
        }
    }

    let _ = app_handle.emit("m2m://group-event", GroupEvent {
        group_id,
        event_type: "member_left".to_string(),
        peer_key_hex: Some(our_peer_key_hex),
    });

    Ok(())
}

/// Load group messages with pagination.
#[tauri::command]
pub async fn load_group_messages(
    state: State<'_, Arc<AppState>>,
    group_id: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessage>, String> {
    state.ensure_message_store(&state.data_dir).await
        .map_err(|e| format!("message store init: {e}"))?;

    let sk = state.storage_key.read().await;
    let ms = state.message_store.lock().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    let key = sk.as_ref().ok_or("storage key not available")?;

    // Load stored messages with encrypted content
    let stored = store
        .load_group_messages_with_content(&group_id, limit.unwrap_or(100), 0)
        .map_err(|e| format!("failed to load group messages: {e}"))?;

    let our_identity = state.identity.read().await;
    let our_peer_key_hex = our_identity.as_ref()
        .map(|id| hex::encode(id.public_key_bytes()));
    drop(our_identity);

    let mut messages: Vec<ChatMessage> = Vec::with_capacity(stored.len());
    for (mut m, enc_content, enc_nonce) in stored {
        // Decrypt content from storage
        let content = match super::util::crypto_decrypt_storage(
            &enc_content, &enc_nonce, key, super::util::AAD_MSG_STORE,
        ) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(e) => {
                tracing::warn!(error = %e, "failed to decrypt group message content");
                "[encrypted]".to_string()
            }
        };
        m.content = content;

        // Determine direction
        let direction = match &our_peer_key_hex {
            Some(our) if m.sender_peer_key_hex == *our => "sent",
            _ => "received",
        };
        m.direction = direction.to_string();

        messages.push(m);
    }

    Ok(messages)
}

/// Update group name (admin only).
#[tauri::command]
pub async fn update_group_name(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: String,
    new_name: String,
) -> Result<(), String> {
    let our_identity = state.identity.read().await;
    let identity = our_identity.as_ref().ok_or("identity not initialized")?;
    let our_peer_key_hex = hex::encode(identity.public_key_bytes());

    {
        let mut gm = state.group_manager.write().await;
        let group = gm.get_group_mut(&group_id)
            .ok_or("group not found")?;
        if !group.is_admin(&our_peer_key_hex) {
            return Err("only admins can change the group name".to_string());
        }
        group.name = new_name.clone();
    }

    // Persist
    state.ensure_message_store(&state.data_dir).await
        .map_err(|e| format!("message store init: {e}"))?;
    let ms = state.message_store.lock().await;
    if let Some(store) = ms.as_ref() {
        let _ = store.update_group_name(&group_id, &new_name);
    }
    drop(ms);

    // Notify online members
    let info_msg = crate::protocol::GroupInfoData {
        group_id: group_id.clone(),
        new_name: Some(new_name),
        changed_by_peer_key_hex: our_peer_key_hex,
    };
    let info_bytes = protocol::serialize(&info_msg)
        .map_err(|e| format!("serialization failed: {e}"))?;

    let gm_read = state.group_manager.read().await;
    let member_keys: Vec<String> = gm_read.get_group(&group_id)
        .map(|g| g.members.iter().map(|m| m.peer_key_hex.clone()).collect())
        .unwrap_or_default();
    drop(gm_read);

    let conns = state.connections.read().await;
    for member_key in &member_keys {
        if let Some(conn_arc) = conns.get(member_key) {
            let mut conn = conn_arc.lock().await;
            let PeerConnection { session, write_half, .. } = &mut *conn;
            let _ = session
                .send_encrypted_typed(write_half, PacketType::GroupInfo, &info_bytes)
                .await;
        }
    }

    let _ = app_handle.emit("m2m://group-event", GroupEvent {
        group_id,
        event_type: "name_changed".to_string(),
        peer_key_hex: None,
    });

    Ok(())
}
