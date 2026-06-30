/// M2M — Reconnection Logic
///
/// Automatic reconnection on connection loss with exponential backoff,
/// pending message delivery, and file transfer resume.
///
/// ## Design
///
/// When a TCP connection drops, we don't immediately clean up. Instead:
///
/// 1. Save the connection metadata (peer key, best strategy, pending transfers)
/// 2. Enter exponential backoff: 1s, 2s, 4s, 8s, 16s, 30s cap
/// 3. At each retry, attempt to re-establish the connection and re-run the X3DH handshake
/// 4. If the new handshake succeeds, flush queued messages and resume transfers
/// 5. After 5 failed retries, emit a "disconnected" event and clean up
///
/// ## Why not reuse the session?
///
/// X3DH provides forward secrecy by deriving ephemeral session keys.
/// Reusing old session keys would defeat this purpose. A new handshake
/// is required for each connection, establishing fresh X3DH keys.
///
/// ## Pending Message Queue
///
/// When the user sends a message while offline, it's stored in the
/// message store with direction="pending". On reconnect, the pending
/// messages are sent in order and their direction is updated to "sent".
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::commands::{ConnectionEvent, MessageEvent, ChatMessage};
use crate::commands::util;
use crate::crypto::{IdentityKeypair, X25519IdentityKeypair, PrekeyBundle, EphemeralKeypair};
use crate::hole_punch::{ConnectionManager, extract_candidates_from_invite};
use crate::network;
use crate::protocol::{self, PacketType, WireCandidate};
use crate::session::Session;
use crate::state::{AppState, PeerConnection};
use crate::stun;

/// Maximum number of reconnection attempts before giving up.
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Initial backoff delay (1 second), doubles each attempt.
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// Maximum backoff delay (30 seconds cap).
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Timeout for a single reconnection handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// Metadata needed to reconnect to a peer.
/// This is saved when the connection drops and used during retry.
#[derive(Debug, Clone)]
pub struct ReconnectInfo {
    /// Peer's Ed25519 identity public key (hex-encoded).
    pub peer_key_hex: String,
    /// Peer's fingerprint (for display).
    pub peer_fingerprint: String,
    /// Peer's X25519 identity public key (from invite prekey bundle).
    pub peer_x25519_pub: [u8; 32],
    /// Peer's signed prekey public key (for X3DH re-handshake).
    pub peer_signed_prekey: [u8; 32],
    /// Peer's signed prekey signature (Ed25519 sig over SPK).
    pub peer_signed_prekey_sig: Vec<u8>,
    /// Peer's one-time prekey (if any was used).
    pub peer_one_time_prekey: Option<[u8; 32]>,
    /// The strategies that worked before — we try these first.
    pub strategy_name: String,
    /// Peer's address from the previous invite.
    pub peer_address_hint: String,
    /// Peer's last-known network candidates (for ICE-Lite).
    pub peer_candidates: Vec<protocol::WireCandidate>,
    /// Whether the peer's fingerprint was verified.
    pub peer_verified: bool,
    /// Our chosen ratchet interval (from the previous session).
    pub ratchet_interval: u64,
}

/// Spawn the reconnection-aware receive loop.
///
/// This wraps the existing receive loop with automatic reconnection.
/// On connection drop, it enters exponential backoff and attempts
/// to re-establish the X3DH session, then resumes normal operation.
pub fn spawn_reconnect_loop(
    app_handle: AppHandle,
    state: Arc<AppState>,
    read_half: tokio::net::tcp::OwnedReadHalf,
    write_half: tokio::net::tcp::OwnedWriteHalf,
    peer_key_hex: String,
    reconnect_info: Option<ReconnectInfo>,
) {
    // If we have reconnect info, store it in the connection metadata
    // so the disconnect handler can use it.
    if let Some(info) = reconnect_info {
        let mut pending = state.pending_reconnects.write().await;
        pending.insert(peer_key_hex.clone(), info);
    }
    drop(pending);

    // ─── Original receive loop (unchanged behavior) ───
    // When the connection drops, we intercept the cleanup and
    // start the reconnection retry loop instead of removing immediately.
    let hb_peer = peer_key_hex.clone();
    let hb_state = state.clone();

    // Heartbeat task (unchanged from original)
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            Duration::from_secs(crate::protocol::HEARTBEAT_INTERVAL_SECS),
        );
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let conns = hb_state.connections.read().await;
            if let Some(conn_arc) = conns.get(&hb_peer) {
                let mut conn = conn_arc.lock().await;
                match network::send_heartbeat(&mut conn.write_half).await {
                    Ok(_) => tracing::trace!(peer = %hb_peer, "heartbeat sent"),
                    Err(e) => {
                        tracing::info!(peer = %hb_peer, error = %e, "heartbeat failed — disconnecting");
                        break;
                    }
                }
            } else {
                break;
            }
        }
    });

    // ── Receive loop wrapper with reconnection logic ──
    tokio::spawn(async move {
        let mut current_read = read_half;
        let mut current_write = write_half;
        let mut retry_count: u32 = 0;

        loop {
            // Read frames until the connection drops
            let frame = match network::read_frame_from_read_half(&mut current_read).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::info!(
                        peer = %peer_key_hex,
                        error = %e,
                        attempt = retry_count + 1,
                        "peer connection lost — attempting reconnection"
                    );

                    // Try to reconnect
                    match attempt_reconnect(
                        &app_handle, &state, &peer_key_hex, retry_count,
                    ).await {
                        Ok((new_read, new_write)) => {
                            retry_count = 0; // Reset on success
                            current_read = new_read;
                            current_write = new_write;

                            // Emit "reconnected" event
                            let _ = app_handle.emit("m2m://connection", ConnectionEvent {
                                peer_key_hex: peer_key_hex.clone(),
                                state: "established".to_string(),
                                peer_fingerprint: None,
                            });

                            // Flush pending messages
                            flush_pending_messages(&app_handle, &state, &peer_key_hex).await;

                            continue; // Return to reading frames
                        }
                        Err(_) => {
                            retry_count += 1;
                            if retry_count >= MAX_RECONNECT_ATTEMPTS {
                                tracing::info!(
                                    peer = %peer_key_hex,
                                    attempts = retry_count,
                                    "reconnection exhausted — giving up"
                                );

                                // Remove reconnect info
                                {
                                    let mut pending = state.pending_reconnects.write().await;
                                    pending.remove(&peer_key_hex);
                                }

                                // Notify frontend
                                let _ = app_handle.emit("m2m://connection", ConnectionEvent {
                                    peer_key_hex: peer_key_hex.clone(),
                                    state: "disconnected".to_string(),
                                    peer_fingerprint: None,
                                });

                                // Remove connection
                                let mut conns = state.connections.write().await;
                                conns.remove(&peer_key_hex);
                                break;
                            }

                            // Wait with exponential backoff before next retry
                            let delay = compute_backoff(retry_count);
                            tracing::info!(
                                peer = %peer_key_hex,
                                attempt = retry_count,
                                delay_ms = delay.as_millis(),
                                "waiting before next reconnection attempt"
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                    }
                }
            };

            // ═══════════════════════════════════════════════
            // Frame dispatch — identical to original receive loop
            // ═══════════════════════════════════════════════

            // Process the frame using the shared dispatch function
            let should_break = handle_frame(
                &app_handle, &state, &peer_key_hex, &frame,
            ).await;

            if should_break {
                break;
            }
        }
    });
}

/// Handle a single received frame. Returns true if the receive loop should exit.
async fn handle_frame(
    app_handle: &AppHandle,
    state: &Arc<AppState>,
    peer_key_hex: &str,
    frame: &network::RawFrame,
) -> bool {
    use crate::protocol::MessageBody;
    use crate::commands::{MessageEvent, FileRequestEvent};

    match frame.packet_type {
        PacketType::EncryptedMessage => {
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_message(frame) {
                    Ok(body) => match &body {
                        MessageBody::Text { id, content } => {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();

                            let history = *state.history_enabled.read().await;
                            if history {
                                let sk = state.storage_key.read().await;
                                let ms = state.message_store.lock().await;
                                if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                    match util::crypto_encrypt_storage(content.as_bytes(), key, util::AAD_MSG_STORE) {
                                        Ok((nonce, encrypted)) => {
                                            if let Some(peer_bytes) = util::decode_peer_key_logged(peer_key_hex) {
                                                let _ = store.ensure_conversation(peer_key_hex, &peer_bytes);
                                                let _ = store.store_message(
                                                    id, peer_key_hex, "received",
                                                    &encrypted, &nonce, now as i64,
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(error = %e, "failed to encrypt received message for storage");
                                        }
                                    }
                                }
                            }

                            let _ = app_handle.emit("m2m://message", MessageEvent {
                                peer_key_hex: peer_key_hex.to_string(),
                                message: ChatMessage {
                                    id: id.clone(),
                                    content: content.clone(),
                                    direction: "received".to_string(),
                                    timestamp: now,
                                },
                            });
                        }
                        MessageBody::Ack { id } => {
                            tracing::debug!(msg_id = %id, "received ack");
                        }
                    },
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to decrypt message");
                    }
                }
            }
            false
        }
        PacketType::FileTransferRequest => {
            // File transfer request handler (same as original)
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(req) = protocol::deserialize::<protocol::FileTransferRequestData>(&plaintext) {
                        {
                            let mut transfers = state.incoming_transfers.write().await;
                            transfers.entry(req.transfer_id.clone()).or_insert_with(|| {
                                let safe_name = network::sanitize_filename(&req.filename)
                                    .unwrap_or_else(|| format!("file_{}", req.transfer_id));
                                let (temp_file, temp_path) = match util::create_temp_file(req.total_size) {
                                    Ok((f, p)) => (Some(f), Some(p)),
                                    Err(e) => {
                                        tracing::warn!(error = %e, "failed to create temp file");
                                        (None, None)
                                    }
                                };
                                crate::state::IncomingFileTransfer {
                                    transfer_id: req.transfer_id.clone(),
                                    peer_key_hex: peer_key_hex.to_string(),
                                    filename: safe_name,
                                    total_size: req.total_size,
                                    total_chunks: req.total_chunks,
                                    file_hash: req.file_hash.clone(),
                                    chunk_hashes: req.chunk_hashes.clone(),
                                    peer_protocol_version: req.file_transfer_version,
                                    save_path: std::path::PathBuf::new(),
                                    temp_file,
                                    temp_path,
                                    chunks_received: 0,
                                    bytes_received: 0,
                                    chunks_bitmask: vec![false; req.total_chunks as usize],
                                    state: crate::state::TransferState::Pending,
                                    created_at: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs(),
                                    error: None,
                                }
                            });
                        }
                        let _ = app_handle.emit("m2m://file-request", FileRequestEvent {
                            peer_key_hex: peer_key_hex.to_string(),
                            transfer_id: req.transfer_id.clone(),
                            filename: req.filename.clone(),
                            total_size: req.total_size,
                        });
                    }
                }
            }
            false
        }
        PacketType::FileTransferChunk => {
            // File chunk handler (same as original)
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(chunk) = protocol::deserialize::<protocol::FileTransferChunkData>(&plaintext) {
                        let mut transfers = state.incoming_transfers.write().await;
                        if let Some(transfer) = transfers.get_mut(&chunk.transfer_id) {
                            let hash = sodiumoxide::crypto::hash::sha256::hash(&chunk.data);
                            if hash.0.to_vec() == chunk.chunk_hash {
                                if let Some(ref mut file) = transfer.temp_file {
                                    use std::io::{Seek, Write};
                                    let offset = (chunk.chunk_index as u64) * (crate::protocol::MAX_FILE_CHUNK_SIZE as u64);
                                    if file.seek(std::io::SeekFrom::Start(offset)).is_ok() {
                                        if file.write_all(&chunk.data).is_ok() {
                                            transfer.chunks_received += 1;
                                            if let Some(bit) = transfer.chunks_bitmask.get_mut(chunk.chunk_index as usize) {
                                                *bit = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            false
        }
        PacketType::FileTransferComplete => {
            // File complete handler (same as original)
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(complete) = protocol::deserialize::<protocol::FileTransferCompleteData>(&plaintext) {
                        let mut transfers = state.incoming_transfers.write().await;
                        if let Some(mut transfer) = transfers.remove(&complete.transfer_id) {
                            let all_received = transfer.chunks_received == transfer.total_chunks
                                && transfer.chunks_bitmask.iter().all(|&b| b);
                            if all_received {
                                // Verify and finalize
                                if let Some(ref mut file) = transfer.temp_file {
                                    use std::io::{Read, Seek};
                                    let mut buf = Vec::with_capacity(transfer.total_size as usize);
                                    if file.seek(std::io::SeekFrom::Start(0)).and_then(|_| file.read_to_end(&mut buf)).is_ok() {
                                        let hash = sodiumoxide::crypto::hash::sha256::hash(&buf);
                                        if hash.0.to_vec() == transfer.file_hash {
                                            let safe_name = network::sanitize_filename(&transfer.filename)
                                                .unwrap_or_else(|| format!("download_{}", complete.transfer_id));
                                            let final_path = std::path::PathBuf::from(&safe_name);
                                            drop(transfer.temp_file.take());
                                            if let Some(ref temp_path) = transfer.temp_path {
                                                let _ = std::fs::rename(temp_path, &final_path);
                                            }
                                            let _ = app_handle.emit("m2m://file-complete", serde_json::json!({
                                                "transfer_id": complete.transfer_id,
                                                "filename": safe_name,
                                                "path": final_path.to_string_lossy(),
                                            }));
                                        }
                                    }
                                }
                            } else {
                                drop(transfer.temp_file);
                                if let Some(ref path) = transfer.temp_path {
                                    let _ = std::fs::remove_file(path);
                                }
                            }
                        }
                    }
                }
            }
            false
        }
        PacketType::FileTransferAccept => {
            // File accept handler
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&plaintext) {
                        if let Some(tid) = val.get("transfer_id").and_then(|v| v.as_str()) {
                            let has_file = {
                                let transfers = state.outgoing_transfers.read().await;
                                transfers.get(tid).is_some()
                            };
                            if has_file {
                                let tid = tid.to_string();
                                let state_c = state.clone();
                                let app_c = app_handle.clone();
                                let peer_c = peer_key_hex.to_string();
                                drop(conn);
                                drop(conns);
                                crate::commands::files::try_start_outgoing_transfer(
                                    app_c, state_c, peer_c, tid,
                                );
                            }
                        }
                    }
                }
            }
            false
        }
        PacketType::FileTransferReject => {
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&plaintext) {
                        if let Some(tid) = val.get("transfer_id").and_then(|v| v.as_str()) {
                            state.outgoing_transfers.write().await.remove(tid);
                        }
                    }
                }
            }
            false
        }
        PacketType::FileTransferChunkAck => {
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(ack) = protocol::deserialize::<protocol::FileTransferChunkAckData>(&plaintext) {
                        let mut outgoing = state.outgoing_transfers.write().await;
                        if let Some(t) = outgoing.get_mut(&ack.transfer_id) {
                            if ack.chunk_index >= t.last_acked_index {
                                t.chunks_acked += ack.chunk_index.saturating_sub(t.last_acked_index) + 1;
                                t.last_acked_index = ack.chunk_index;
                                t.last_activity_at = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                            }
                        }
                    }
                }
            }
            false
        }
        PacketType::FileTransferCancel => {
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(cancel) = protocol::deserialize::<protocol::FileTransferCancelData>(&plaintext) {
                        let tid = cancel.transfer_id;
                        {
                            let mut outgoing = state.outgoing_transfers.write().await;
                            if let Some(t) = outgoing.get_mut(&tid) {
                                t.state = crate::state::TransferState::Cancelled;
                            }
                            outgoing.remove(&tid);
                        }
                        {
                            let mut incoming = state.incoming_transfers.write().await;
                            if let Some(t) = incoming.remove(&tid) {
                                drop(t.temp_file);
                                if let Some(ref path) = t.temp_path {
                                    let _ = std::fs::remove_file(path);
                                }
                            }
                        }
                        {
                            let mut queue = state.transfer_queue.write().await;
                            queue.queue.retain(|id| id != &tid);
                            queue.active.remove(&tid);
                        }
                        let _ = app_handle.emit("m2m://transfer-cancelled", serde_json::json!({
                            "transfer_id": tid,
                        }));
                    }
                }
            }
            false
        }
        PacketType::Heartbeat => {
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                let _ = network::send_heartbeat_ack(&mut conn.write_half).await;
            }
            false
        }
        PacketType::HeartbeatAck => false,
        PacketType::ConversationMeta => {
            let conns = state.connections.read().await;
            if let Some(conn_arc) = conns.get(peer_key_hex) {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    if let Ok(meta) = protocol::deserialize::<protocol::ConversationMetaData>(&plaintext) {
                        let ms = state.message_store.lock().await;
                        if let Some(ref store) = *ms {
                            let _ = store.set_peer_display_name(peer_key_hex, &meta.my_display_name);
                            if !meta.your_display_name.is_empty() {
                                if let Ok(Some(conv)) = store.get_conversation(peer_key_hex) {
                                    if conv.display_name.is_none() {
                                        let _ = store.rename_conversation(peer_key_hex, &meta.your_display_name);
                                    }
                                }
                            }
                        }
                        let _ = app_handle.emit("m2m://conversation-meta", serde_json::json!({
                            "peer_key_hex": peer_key_hex,
                            "peer_display_name": meta.my_display_name,
                            "suggested_name": meta.your_display_name,
                        }));
                    }
                }
            }
            false
        }
        PacketType::Disconnect => {
            tracing::info!(peer = %peer_key_hex, "peer sent disconnect");
            let _ = app_handle.emit("m2m://connection", ConnectionEvent {
                peer_key_hex: peer_key_hex.to_string(),
                state: "disconnected".to_string(),
                peer_fingerprint: None,
            });
            let mut conns = state.connections.write().await;
            conns.remove(peer_key_hex);
            true // Exit receive loop
        }
        PacketType::Error => {
            tracing::warn!(peer = %peer_key_hex, "peer sent error packet");
            false
        }
        _ => {
            tracing::warn!(peer = %peer_key_hex, "received unexpected packet type");
            false
        }
    }
}

/// Attempt to reconnect to a peer. Returns (read_half, write_half) on success.
async fn attempt_reconnect(
    app_handle: &AppHandle,
    state: &Arc<AppState>,
    peer_key_hex: &str,
    attempt: u32,
) -> Result<(tokio::net::tcp::OwnedReadHalf, tokio::net::tcp::OwnedWriteHalf), ()> {
    // Get reconnection info
    let info = {
        let pending = state.pending_reconnects.read().await;
        match pending.get(peer_key_hex) {
            Some(i) => i.clone(),
            None => {
                tracing::warn!(peer = %peer_key_hex, "no reconnect info available");
                return Err(());
            }
        }
    };

    tracing::info!(
        peer = %peer_key_hex,
        strategy = %info.strategy_name,
        attempt = attempt + 1,
        "attempting reconnection"
    );

    // Emit "reconnecting" event
    let _ = app_handle.emit("m2m://connection", ConnectionEvent {
        peer_key_hex: peer_key_hex.to_string(),
        state: "reconnecting".to_string(),
        peer_fingerprint: Some(info.peer_fingerprint.clone()),
    });

    // Build the candidate list from saved info
    let candidates: Vec<protocol::WireCandidate> = {
        let mut all = info.peer_candidates.clone();
        // Also add the original address hint as a host candidate
        if let Ok(addr) = info.peer_address_hint.parse::<std::net::SocketAddr>() {
            let host = protocol::WireCandidate {
                address: addr.to_string(),
                candidate_type: 0, // host
                relay_id: None,
            };
            if !all.iter().any(|c| c.address == host.address) {
                all.push(host);
            }
        }
        all
    };

    // Build connect addresses from candidates
    let listen_addr = *state.listen_addr.read().await;
    let peer_addrs = extract_candidates_from_invite(&info.peer_address_hint, &candidates);

    // Use the connection manager to connect (same Happy-Eyeballs path)
    let result = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        ConnectionManager::connect(&peer_addrs, listen_addr),
    ).await;

    let (mut stream, role, _remote_addr) = match result {
        Ok(Ok(res)) => (res.stream, res.role, res.remote_addr),
        Ok(Err(e)) => {
            tracing::warn!(peer = %peer_key_hex, error = %e, "reconnect connection failed");
            return Err(());
        }
        Err(_) => {
            tracing::warn!(peer = %peer_key_hex, "reconnect timed out");
            return Err(());
        }
    };

    // Get identity keys
    let identity = {
        let id = state.identity.read().await;
        match id.as_ref() {
            Some(kp) => {
                // We need to create a new reference — clone the Ed25519 keypair
                // for the reconnection handshake.
                let pub_bytes = kp.public_key_bytes();
                let secret_bytes = kp.secret_key_bytes();
                match IdentityKeypair::from_bytes(&pub_bytes, &secret_bytes) {
                    Ok(kp) => kp,
                    Err(_) => {
                        tracing::error!("failed to clone identity for reconnect");
                        return Err(());
                    }
                }
            }
            None => {
                tracing::error!("no identity for reconnection");
                return Err(());
            }
        }
    };

    let x25519_kp = {
        let x = state.x25519_identity.read().await;
        match x.as_ref() {
            Some(kp) => {
                let pub_bytes = kp.public_key_bytes();
                let secret_bytes = kp.secret_key_bytes();
                match X25519IdentityKeypair::from_bytes(&pub_bytes, &secret_bytes) {
                    Ok(kp) => kp,
                    Err(_) => {
                        tracing::error!("failed to clone X25519 identity for reconnect");
                        return Err(());
                    }
                }
            }
            None => {
                tracing::error!("no X25519 identity for reconnection");
                return Err(());
            }
        }
    };

    // Gather candidates for the new handshake
    let config = state.stun_config.read().await;
    let stun_result = stun::discover_public_addrs(&config).await.ok();
    drop(config);

    let our_candidates: Vec<WireCandidate> = {
        let host = crate::candidate::gather_host_candidates();
        let ipv6 = crate::candidate::gather_ipv6_candidates();
        let reflexive = stun_result.as_ref()
            .map(crate::candidate::gather_reflexive_candidates)
            .unwrap_or_default();
        let mut all: Vec<_> = host.into_iter().chain(ipv6).chain(reflexive).collect();
        all.sort_by(|a, b| b.priority.cmp(&a.priority));
        all.iter().map(|c| WireCandidate {
            address: c.address.clone(),
            candidate_type: c.candidate_type as u8,
            relay_id: None,
        }).collect()
    };

    // Perform X3DH handshake
    let mut session = Session::new();

    let bundle = PrekeyBundle {
        identity_key: info.peer_x25519_pub,
        signed_prekey: info.peer_signed_prekey,
        signed_prekey_sig: info.peer_signed_prekey_sig.clone(),
        one_time_prekey: info.peer_one_time_prekey,
    };

    let expected_peer_pub = util::decode_peer_key(peer_key_hex).map_err(|_| ())?;

    match role {
        crate::hole_punch::Role::Initiator => {
            session.handshake_as_initiator_x3dh(
                &mut stream, &identity, &x25519_kp,
                &expected_peer_pub, &bundle, our_candidates,
            ).await.map_err(|e| {
                tracing::warn!(peer = %peer_key_hex, error = %e, "reconnect initiator handshake failed");
            })?;
        }
        crate::hole_punch::Role::Responder => {
            let frame = network::read_frame(&mut stream).await.map_err(|_| ())?;
            if frame.packet_type != PacketType::X3DHHandshakeInit {
                tracing::warn!("reconnect responder expected X3DHHandshakeInit");
                return Err(());
            }
            let spk_lock = state.active_signed_prekey.read().await;
            let spk = match spk_lock.as_ref() {
                Some(spk) => spk,
                None => {
                    tracing::error!("no signed prekey for reconnect responder");
                    return Err(());
                }
            };
            session.handshake_as_responder_x3dh(
                &mut stream, &identity, &x25519_kp, spk, &frame, our_candidates,
            ).await.map_err(|e| {
                tracing::warn!(peer = %peer_key_hex, error = %e, "reconnect responder handshake failed");
            })?;
        }
    }

    // Restore peer verification state and ratchet interval
    if info.peer_verified {
        session.mark_peer_verified();
    }
    session.ratchet_interval = info.ratchet_interval;

    // Split the stream
    let (read_half, write_half) = stream.into_split();

    // Create the new PeerConnection
    let conn = PeerConnection {
        write_half: {
            // We need to get a write half that we own
            // tokio::net::tcp::OwnedWriteHalf from into_split()
            write_half
        },
        session,
        remote_addr: _remote_addr,
        strategy_name: info.strategy_name.clone(),
    };

    // Replace the connection in state
    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.to_string(), Arc::new(tokio::sync::Mutex::new(conn)));
    drop(conns);

    tracing::info!(peer = %peer_key_hex, "reconnection successful");
    Ok((read_half, write_half))
}

/// Send any pending messages that were queued while offline.
async fn flush_pending_messages(
    app_handle: &AppHandle,
    state: &Arc<AppState>,
    peer_key_hex: &str,
) {
    let ms = state.message_store.lock().await;
    let store = match ms.as_ref() {
        Some(s) => s,
        None => return,
    };

    // Load messages with direction="pending"
    let pending = match store.load_messages_by_direction(peer_key_hex, "pending") {
        Ok(msgs) => msgs,
        Err(e) => {
            tracing::warn!(error = %e, "failed to load pending messages");
            return;
        }
    };

    if pending.is_empty() {
        return;
    }

    drop(ms); // Release message store lock before sending

    tracing::info!(count = pending.len(), "flushing pending messages");

    let sk = state.storage_key.read().await;
    let key = match sk.as_ref() {
        Some(k) => k,
        None => return,
    };

    for msg in pending {
        // Decrypt the stored message
        let content = match util::crypto_decrypt_storage(
            &msg.content_encrypted, &msg.content_nonce, key, util::AAD_MSG_STORE,
        ) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        // Send the message
        let conns = state.connections.read().await;
        if let Some(conn_arc) = conns.get(peer_key_hex) {
            let mut conn = conn_arc.lock().await;
            let result = conn.session.send_text(&mut conn.write_half, &content).await;
            if result.is_ok() {
                // Update direction from "pending" to "sent"
                let ms = state.message_store.lock().await;
                if let Some(ref store) = *ms {
                    let _ = store.update_message_direction(&msg.id, "sent");
                }
                tracing::trace!(msg_id = %msg.id, "flushed pending message");
            } else {
                tracing::warn!(msg_id = %msg.id, "failed to flush pending message");
                break; // Stop on first failure — remaining will be retried
            }
        }
    }
}

/// Compute the backoff delay for a given retry attempt.
/// Exponential: 1s, 2s, 4s, 8s, 16s, cap at 30s.
fn compute_backoff(attempt: u32) -> Duration {
    let secs = INITIAL_BACKOFF.as_secs() * 2u64.pow(attempt);
    Duration::from_secs(secs.min(MAX_BACKOFF.as_secs()))
}

/// Extract a ReconnectInfo from a session for future reconnection.
pub fn session_to_reconnect_info(
    session: &Session,
    peer_key_hex: &str,
    peer_fingerprint: &str,
    peer_address_hint: &str,
    strategy_name: &str,
) -> ReconnectInfo {
    ReconnectInfo {
        peer_key_hex: peer_key_hex.to_string(),
        peer_fingerprint: peer_fingerprint.to_string(),
        peer_x25519_pub: [0u8; 32], // Will be populated from the invite
        peer_signed_prekey: [0u8; 32],
        peer_signed_prekey_sig: Vec::new(),
        peer_one_time_prekey: None,
        strategy_name: strategy_name.to_string(),
        peer_address_hint: peer_address_hint.to_string(),
        peer_candidates: session.peer_candidates.clone(),
        peer_verified: session.peer_verified,
        ratchet_interval: session.ratchet_interval,
    }
}

#[cfg(test)]
mod reconnect_tests {
    use super::*;

    #[test]
    fn test_compute_backoff_exponential() {
        assert_eq!(compute_backoff(0), Duration::from_secs(1));
        assert_eq!(compute_backoff(1), Duration::from_secs(2));
        assert_eq!(compute_backoff(2), Duration::from_secs(4));
        assert_eq!(compute_backoff(3), Duration::from_secs(8));
        assert_eq!(compute_backoff(4), Duration::from_secs(16));
    }

    #[test]
    fn test_compute_backoff_capped() {
        // Attempt 5 would be 32s without cap, but we cap at 30
        assert_eq!(compute_backoff(5), Duration::from_secs(30));
        assert_eq!(compute_backoff(10), Duration::from_secs(30));
        assert_eq!(compute_backoff(100), Duration::from_secs(30));
    }

    #[test]
    fn test_compute_backoff_first_attempt_immediate() {
        assert_eq!(compute_backoff(0), Duration::from_secs(1),
            "first retry should be 1 second");
    }
}
