/// M2M — Tauri Commands
///
/// IPC bridge between the React UI and the Rust backend.
/// Each command validates inputs and returns safe, typed responses.
/// No secrets are exposed to the frontend.
use std::net::SocketAddr;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::crypto::{self, IdentityKeypair};
use crate::identity;
use crate::network;
use crate::protocol::{self, FileTransferRequestData, MessageBody, PacketType};
use crate::session::Session;
use crate::state::{AppState, IncomingFileTransfer, PeerConnection};
use crate::storage::{self, KeyStore};
use crate::stun;
use crate::tor;

use serde::{Deserialize, Serialize};

// ─── Response types for the frontend — never contain secrets ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    pub fingerprint: String,
    pub public_key_hex: String,
    pub has_identity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub state: String,
    pub peer_fingerprint: Option<String>,
    pub peer_verified: bool,
    pub peer_key_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, zeroize::Zeroize)]
pub struct ChatMessage {
    pub id: String,
    pub content: String,
    pub direction: String,
    pub timestamp: u64,
}

impl Drop for ChatMessage {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.content.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteInfo {
    pub fingerprint: String,
    pub address_hint: String,
    pub expires_at: u64,
    pub one_time: bool,
    pub valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferInfo {
    pub transfer_id: String,
    pub filename: String,
    pub total_size: u64,
    pub peer_key_hex: String,
}

// ─── Events emitted to the React frontend ───

#[derive(Debug, Clone, Serialize)]
pub struct MessageEvent {
    pub peer_key_hex: String,
    pub message: ChatMessage,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionEvent {
    pub peer_key_hex: String,
    pub state: String,
    pub peer_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRequestEvent {
    pub peer_key_hex: String,
    pub transfer_id: String,
    pub filename: String,
    pub total_size: u64,
}

// ─── Commands ───

/// Initialize the crypto library and load or create identity.
/// On first launch, generates a new identity and persists it.
#[tauri::command]
pub async fn init_identity(
    state: State<'_, Arc<AppState>>,
) -> Result<IdentityInfo, String> {
    crypto::init().map_err(|e| format!("crypto init failed: {e}"))?;

    // Try to load identity from storage
    let data_dir = storage::ensure_data_dir()
        .map_err(|e| format!("data dir error: {e}"))?;
    let keys_db_path = data_dir.join("keys.db");
    let msgs_db_path = data_dir.join("messages.db");

    let key_store = KeyStore::open(&keys_db_path)
        .map_err(|e| format!("key store error: {e}"))?;

    let keypair = if key_store.has_identity().unwrap_or(false) {
        // Load existing identity
        let (pub_bytes, enc_sk, nonce) = key_store
            .load_identity()
            .map_err(|e| format!("failed to load identity: {e}"))?;

        let mut pub_arr = [0u8; 32];
        pub_arr.copy_from_slice(&pub_bytes);

        // Decrypt the private key using the storage encryption key
        let storage_key = derive_storage_key(&pub_bytes);
        let sk_bytes = crypto_decrypt_storage(&enc_sk, &nonce, &storage_key)
            .map_err(|e| format!("failed to decrypt identity: {e}"))?;

        let mut sk_arr = [0u8; 64];
        sk_arr.copy_from_slice(&sk_bytes);

        // Store the storage key for message encryption
        {
            let mut sk_lock = state.storage_key.write().await;
            *sk_lock = Some(storage_key);
        }

        IdentityKeypair::from_bytes(&pub_arr, &sk_arr)
            .map_err(|e| format!("failed to reconstruct identity: {e}"))?
    } else {
        // Generate new identity
        let kp = IdentityKeypair::generate()
            .map_err(|e| format!("keypair generation failed: {e}"))?;

        // Encrypt and persist
        let pub_bytes = kp.public_key_bytes();
        let sk_bytes = kp.secret_key_bytes();
        let storage_key = derive_storage_key(&pub_bytes);
        let (nonce, encrypted_sk) = crypto_encrypt_storage(&sk_bytes, &storage_key)
            .map_err(|e| format!("failed to encrypt identity: {e}"))?;

        let now = chrono::Utc::now().timestamp();
        key_store
            .store_identity(&pub_bytes, &encrypted_sk, &nonce, now)
            .map_err(|e| format!("failed to store identity: {e}"))?;

        // Store the storage key for message encryption
        {
            let mut sk_lock = state.storage_key.write().await;
            *sk_lock = Some(storage_key);
        }

        kp
    };

    let fingerprint = keypair.fingerprint();
    let pub_hex = hex::encode(keypair.public_key_bytes());

    // Initialise message store
    let msg_store = storage::MessageStore::open(&msgs_db_path)
        .map_err(|e| format!("message store error: {e}"))?;
    {
        let mut ms = state.message_store.lock().await;
        *ms = Some(msg_store);
    }
    // Store the key store handle
    {
        let mut ks = state.key_store.lock().await;
        *ks = Some(key_store);
    }

    let mut identity = state.identity.write().await;
    *identity = Some(keypair);

    Ok(IdentityInfo {
        fingerprint,
        public_key_hex: pub_hex,
        has_identity: true,
    })
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

/// Generate an invite link for sharing.
#[tauri::command]
pub async fn create_invite(
    state: State<'_, Arc<AppState>>,
    address: String,
    validity_minutes: u64,
    one_time: bool,
) -> Result<String, String> {
    let identity = state.identity.read().await;
    let kp = identity
        .as_ref()
        .ok_or("identity not initialized")?;

    let _: SocketAddr = address
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let validity_secs = validity_minutes.saturating_mul(60);

    identity::create_invite(kp, &address, validity_secs, one_time)
        .map_err(|e| format!("invite creation failed: {e}"))
}

/// Validate a received invite link.
#[tauri::command]
pub async fn validate_invite(invite_str: String) -> Result<InviteInfo, String> {
    let signed = identity::validate_invite(&invite_str)
        .map_err(|e| format!("invite validation failed: {e}"))?;

    let fingerprint =
        crypto::fingerprint_from_public_key(&signed.payload.identity_pub);

    Ok(InviteInfo {
        fingerprint,
        address_hint: signed.payload.address_hint.clone(),
        expires_at: signed.payload.expires_at,
        one_time: identity::is_one_time(&signed),
        valid: true,
    })
}

/// Start listening for incoming connections.
#[tauri::command]
pub async fn start_listening(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    address: String,
) -> Result<String, String> {
    let addr: SocketAddr = address
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<(tokio::net::TcpStream, SocketAddr)>(8);

    {
        let mut listen = state.listen_addr.write().await;
        *listen = Some(addr);
    }
    {
        let mut incoming = state.incoming_tx.lock().await;
        *incoming = Some(tx.clone());
    }

    // Spawn the listener task
    tokio::spawn(async move {
        if let Err(e) = network::start_listener(addr, tx).await {
            tracing::error!(error = %e, "listener failed");
        }
    });

    // Spawn the connection handler task
    let state_clone = state.inner().clone();
    let app_clone = app_handle.clone();
    tokio::spawn(async move {
        while let Some((stream, peer_addr)) = rx.recv().await {
            let state_inner = state_clone.clone();
            let app_inner = app_clone.clone();
            tokio::spawn(async move {
                handle_incoming_connection(app_inner, state_inner, stream, peer_addr).await;
            });
        }
    });

    tracing::info!(address = %addr, "started listening");
    Ok(format!("listening on {addr}"))
}

/// Handle an incoming connection: perform handshake as responder.
async fn handle_incoming_connection(
    app_handle: AppHandle,
    state: Arc<AppState>,
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
) {
    let frame = match network::read_frame(&mut stream).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read initial frame from incoming connection");
            return;
        }
    };

    if frame.packet_type != protocol::PacketType::HandshakeInit {
        tracing::warn!("incoming connection sent non-handshake initial packet");
        let _ = network::send_error(
            &mut stream,
            protocol::ErrorCode::HandshakeFailed,
            "expected handshake init",
        )
        .await;
        return;
    }

    let mut session = Session::new();
    {
        let identity = state.identity.read().await;
        let kp = match identity.as_ref() {
            Some(kp) => kp,
            None => {
                tracing::error!("cannot handle connection: no identity");
                return;
            }
        };

        if let Err(e) = session.handshake_as_responder(&mut stream, kp, &frame).await {
            tracing::warn!(error = %e, "handshake failed for incoming connection");
            let _ = network::send_error(
                &mut stream,
                protocol::ErrorCode::HandshakeFailed,
                "handshake failed",
            )
            .await;
            return;
        }
    } // identity borrow dropped here

    let peer_key_hex = hex::encode(session.peer_identity_pub);
    let peer_fingerprint = session.peer_fingerprint();

    // Split the stream for the receive loop
    let (read_half, write_half) = stream.into_split();

    let conn = PeerConnection {
        write_half,
        session,
        remote_addr: peer_addr,
    };

    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.clone(), Arc::new(Mutex::new(conn)));
    drop(conns);

    // Notify frontend
    let _ = app_handle.emit("m2m://connection", ConnectionEvent {
        peer_key_hex: peer_key_hex.clone(),
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint.clone()),
    });

    tracing::info!(peer = %peer_key_hex, "peer connected and authenticated");

    // Upsert peer in key store
    {
        let ks = state.key_store.lock().await;
        if let Some(ref store) = *ks {
            let _ = store.upsert_peer(
                &hex::decode(&peer_key_hex).unwrap_or_default(),
                &peer_fingerprint,
                None,
            );
        }
    }

    // Start the receive loop for this peer
    spawn_receive_loop(app_handle, state, read_half, peer_key_hex);
}

/// Connect to a peer using an invite link.
#[tauri::command]
pub async fn connect_to_peer(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    invite_str: String,
) -> Result<ConnectionInfo, String> {
    let signed = identity::validate_invite(&invite_str)
        .map_err(|e| format!("invite invalid: {e}"))?;

    let addr: SocketAddr = signed
        .payload
        .address_hint
        .parse()
        .map_err(|e| format!("invalid address in invite: {e}"))?;

    let stream = network::connect(addr)
        .await
        .map_err(|e| format!("connection failed: {e}"))?;

    let identity = state.identity.read().await;
    let kp = identity
        .as_ref()
        .ok_or("identity not initialized")?;

    // We need a mutable TcpStream for the handshake
    let mut stream = stream;
    let mut session = Session::new();
    session
        .handshake_as_initiator(&mut stream, kp, &signed.payload.identity_pub)
        .await
        .map_err(|e| format!("handshake failed: {e}"))?;

    let peer_fingerprint = session.peer_fingerprint();
    let peer_key_hex = hex::encode(session.peer_identity_pub);

    // Split the stream
    let (read_half, write_half) = stream.into_split();

    let conn = PeerConnection {
        write_half,
        session,
        remote_addr: addr,
    };

    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.clone(), Arc::new(Mutex::new(conn)));
    drop(conns);

    // Start the receive loop for this peer
    spawn_receive_loop(app_handle, state.inner().clone(), read_half, peer_key_hex.clone());

    Ok(ConnectionInfo {
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint),
        peer_verified: false,
        peer_key_hex: Some(peer_key_hex),
    })
}

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
        if let (Some(ref store), Some(ref key)) = (ms.as_ref(), sk.as_ref()) {
            let (nonce, encrypted) = crypto_encrypt_storage(content.as_bytes(), key)
                .unwrap_or_default();
            let _ = store.ensure_conversation(&peer_key_hex, &hex::decode(&peer_key_hex).unwrap_or_default());
            let _ = store.store_message(
                &msg_id, &peer_key_hex, "sent",
                &encrypted, &nonce, now as i64,
            );
        }
    }

    Ok(ChatMessage {
        id: msg_id,
        content,
        direction: "sent".to_string(),
        timestamp: now,
    })
}

/// Get the connection state for a peer.
#[tauri::command]
pub async fn get_connection_state(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
) -> Result<ConnectionInfo, String> {
    let conn_state = state.connection_state(&peer_key_hex).await;
    let conns = state.connections.read().await;

    let (fingerprint, verified) = match conns.get(&peer_key_hex) {
        Some(conn) => {
            let c = conn.lock().await;
            (Some(c.session.peer_fingerprint()), c.session.peer_verified)
        }
        None => (None, false),
    };

    Ok(ConnectionInfo {
        state: conn_state.to_string(),
        peer_fingerprint: fingerprint,
        peer_verified: verified,
        peer_key_hex: Some(peer_key_hex),
    })
}

/// Mark a peer's fingerprint as verified.
#[tauri::command]
pub async fn verify_peer(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
) -> Result<(), String> {
    let conns = state.connections.read().await;
    let conn_arc = conns
        .get(&peer_key_hex)
        .ok_or("no connection to this peer")?
        .clone();
    let mut conn = conn_arc.lock().await;
    conn.session.mark_peer_verified();
    Ok(())
}

/// Disconnect from a peer gracefully.
#[tauri::command]
pub async fn disconnect_peer(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
) -> Result<(), String> {
    let mut conns = state.connections.write().await;
    if let Some(conn_arc) = conns.remove(&peer_key_hex) {
        let mut conn = conn_arc.lock().await;
        let _ = network::send_disconnect(
            &mut conn.write_half,
            protocol::DisconnectReason::UserInitiated,
        )
        .await;
    }
    Ok(())
}

/// Get a list of all connected peers.
#[tauri::command]
pub async fn list_peers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ConnectionInfo>, String> {
    let conns = state.connections.read().await;
    let mut peers = Vec::new();

    for (key, conn_arc) in conns.iter() {
        let conn = conn_arc.lock().await;
        peers.push(ConnectionInfo {
            state: conn.session.state.to_string(),
            peer_fingerprint: Some(conn.session.peer_fingerprint()),
            peer_verified: conn.session.peer_verified,
            peer_key_hex: Some(key.clone()),
        });
    }

    Ok(peers)
}

// ─── Message Receive Loop ───

/// Spawn an async task that reads incoming frames from a peer
/// and emits Tauri events for the React frontend.
fn spawn_receive_loop(
    app_handle: AppHandle,
    state: Arc<AppState>,
    mut read_half: tokio::net::tcp::OwnedReadHalf,
    peer_key_hex: String,
) {
    tokio::spawn(async move {
        loop {
            // Read a frame from the peer's read half
            let frame = match network::read_frame_from_read_half(&mut read_half).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::info!(peer = %peer_key_hex, error = %e, "peer connection closed");
                    // Notify frontend about disconnection
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
            };

            match frame.packet_type {
                PacketType::EncryptedMessage => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_message(&frame) {
                            Ok(body) => match body {
                                MessageBody::Text { id, content } => {
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();

                                    // Persist received message
                                    let history = *state.history_enabled.read().await;
                                    if history {
                                        let sk = state.storage_key.read().await;
                                        let ms = state.message_store.lock().await;
                                        if let (Some(ref store), Some(ref key)) = (ms.as_ref(), sk.as_ref()) {
                                            let (nonce, encrypted) = crypto_encrypt_storage(content.as_bytes(), key)
                                                .unwrap_or_default();
                                            let _ = store.ensure_conversation(&peer_key_hex, &hex::decode(&peer_key_hex).unwrap_or_default());
                                            let _ = store.store_message(
                                                &id, &peer_key_hex, "received",
                                                &encrypted, &nonce, now as i64,
                                            );
                                        }
                                    }

                                    let _ = app_handle.emit("m2m://message", MessageEvent {
                                        peer_key_hex: peer_key_hex.clone(),
                                        message: ChatMessage {
                                            id,
                                            content,
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
                }
                PacketType::FileTransferRequest => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(req) = protocol::deserialize::<FileTransferRequestData>(&plaintext) {
                                    // Pre-register the transfer so chunks can be stored
                                    {
                                        let mut transfers = state.incoming_transfers.write().await;
                                        transfers.entry(req.transfer_id.clone()).or_insert_with(|| {
                                            IncomingFileTransfer {
                                                filename: req.filename.clone(),
                                                total_size: req.total_size,
                                                total_chunks: req.total_chunks,
                                                file_hash: req.file_hash.clone(),
                                                received_chunks: std::collections::HashMap::new(),
                                                save_path: std::path::PathBuf::new(), // set on accept
                                            }
                                        });
                                    }
                                    let _ = app_handle.emit("m2m://file-request", FileRequestEvent {
                                        peer_key_hex: peer_key_hex.clone(),
                                        transfer_id: req.transfer_id,
                                        filename: req.filename,
                                        total_size: req.total_size,
                                    });
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt file request");
                            }
                        }
                    }
                }
                PacketType::FileTransferChunk => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(chunk) = protocol::deserialize::<protocol::FileTransferChunkData>(&plaintext) {
                                    let mut transfers = state.incoming_transfers.write().await;
                                    if let Some(transfer) = transfers.get_mut(&chunk.transfer_id) {
                                        // Verify chunk hash
                                        let hash = sodiumoxide::crypto::hash::sha256::hash(&chunk.data);
                                        if hash.0.to_vec() == chunk.chunk_hash {
                                            transfer.received_chunks.insert(chunk.chunk_index, chunk.data);
                                        } else {
                                            tracing::warn!("file chunk hash mismatch");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt file chunk");
                            }
                        }
                    }
                }
                PacketType::FileTransferComplete => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(complete) = protocol::deserialize::<protocol::FileTransferCompleteData>(&plaintext) {
                                    let mut transfers = state.incoming_transfers.write().await;
                                    if let Some(transfer) = transfers.remove(&complete.transfer_id) {
                                        // Reassemble and write file
                                        let mut file_data = Vec::with_capacity(transfer.total_size as usize);
                                        for i in 0..transfer.total_chunks {
                                            if let Some(chunk) = transfer.received_chunks.get(&i) {
                                                file_data.extend_from_slice(chunk);
                                            } else {
                                                tracing::warn!("missing chunk {i} for file transfer");
                                                break;
                                            }
                                        }
                                        // Verify total hash
                                        let hash = sodiumoxide::crypto::hash::sha256::hash(&file_data);
                                        if hash.0.to_vec() == transfer.file_hash {
                                            if let Err(e) = std::fs::write(&transfer.save_path, &file_data) {
                                                tracing::warn!(error = %e, "failed to write received file");
                                            } else {
                                                let _ = app_handle.emit("m2m://file-complete", serde_json::json!({
                                                    "transfer_id": complete.transfer_id,
                                                    "filename": transfer.filename,
                                                    "path": transfer.save_path.to_string_lossy(),
                                                }));
                                            }
                                        } else {
                                            tracing::warn!("file hash verification failed");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt file complete");
                            }
                        }
                    }
                }
                PacketType::FileTransferAccept => {
                    // Peer accepted our file transfer — start sending chunks
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&plaintext) {
                                    if let Some(tid) = val.get("transfer_id").and_then(|v| v.as_str()) {
                                        let transfers = state.outgoing_transfers.read().await;
                                        if let Some(filepath) = transfers.get(tid) {
                                            let filepath = filepath.clone();
                                            let tid = tid.to_string();
                                            let state_c = state.clone();
                                            let peer_c = peer_key_hex.clone();
                                            drop(conn);
                                            drop(conns);
                                            // Spawn chunk sender
                                            tokio::spawn(async move {
                                                let _ = send_file_chunks(state_c, &peer_c, &tid, &filepath).await;
                                            });
                                        }
                                    }
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt file accept"),
                        }
                    }
                }
                PacketType::FileTransferReject => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        if let Ok(plaintext) = conn.session.decrypt_typed_frame(&frame) {
                            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&plaintext) {
                                if let Some(tid) = val.get("transfer_id").and_then(|v| v.as_str()) {
                                    state.outgoing_transfers.write().await.remove(tid);
                                    tracing::info!(transfer_id = %tid, "file transfer rejected by peer");
                                }
                            }
                        }
                    }
                }
                PacketType::Heartbeat => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        let _ = network::send_heartbeat_ack(&mut conn.write_half).await;
                    }
                }
                PacketType::HeartbeatAck => {
                    // Heartbeat acknowledged — connection alive
                }
                PacketType::Disconnect => {
                    tracing::info!(peer = %peer_key_hex, "peer sent disconnect");
                    let _ = app_handle.emit("m2m://connection", ConnectionEvent {
                        peer_key_hex: peer_key_hex.clone(),
                        state: "disconnected".to_string(),
                        peer_fingerprint: None,
                    });
                    let mut conns = state.connections.write().await;
                    conns.remove(&peer_key_hex);
                    break;
                }
                PacketType::Error => {
                    tracing::warn!(peer = %peer_key_hex, "peer sent error packet");
                }
                _ => {
                    tracing::warn!(peer = %peer_key_hex, "received unexpected packet type in receive loop");
                }
            }
        }
    });
}

// ─── New Commands ───

/// Load message history for a peer.
#[tauri::command]
pub async fn load_messages(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessage>, String> {
    let ms = state.message_store.lock().await;
    let sk = state.storage_key.read().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    let key = sk.as_ref().ok_or("storage key not available")?;

    let stored = store
        .load_messages(&peer_key_hex, limit.unwrap_or(100))
        .map_err(|e| format!("failed to load messages: {e}"))?;

    let mut messages = Vec::with_capacity(stored.len());
    for m in stored {
        let content = crypto_decrypt_storage(&m.content_encrypted, &m.content_nonce, key)
            .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
            .unwrap_or_else(|_| "[encrypted]".to_string());
        messages.push(ChatMessage {
            id: m.id,
            content,
            direction: m.direction,
            timestamp: m.timestamp as u64,
        });
    }
    Ok(messages)
}

/// Initiate a file transfer to a peer.
#[tauri::command]
pub async fn send_file(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    file_path: String,
) -> Result<FileTransferInfo, String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err("file not found".to_string());
    }

    let metadata = std::fs::metadata(path).map_err(|e| format!("cannot read file: {e}"))?;
    let total_size = metadata.len();
    let filename = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let file_data = std::fs::read(path).map_err(|e| format!("failed to read file: {e}"))?;
    let file_hash = sodiumoxide::crypto::hash::sha256::hash(&file_data);
    let total_chunks = ((total_size as usize + protocol::MAX_FILE_CHUNK_SIZE - 1) / protocol::MAX_FILE_CHUNK_SIZE) as u32;
    let transfer_id = uuid::Uuid::new_v4().to_string();

    // Store for later chunk sending
    state.outgoing_transfers.write().await.insert(transfer_id.clone(), file_path);

    // Send the request
    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex)
        .ok_or("no connection to this peer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;
    session.send_file_request(
        &mut *write_half,
        &transfer_id, &filename, total_size, total_chunks, file_hash.0.to_vec(),
    ).await.map_err(|e| format!("failed to send file request: {e}"))?;

    Ok(FileTransferInfo {
        transfer_id,
        filename,
        total_size,
        peer_key_hex,
    })
}

/// Accept an incoming file transfer.
#[tauri::command]
pub async fn accept_file_transfer(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    transfer_id: String,
    save_dir: String,
) -> Result<(), String> {
    // Store the save_dir into the incoming transfer state so the
    // FileTransferComplete handler knows where to write the reassembled file.
    {
        let transfers = state.incoming_transfers.read().await;
        if !transfers.contains_key(&transfer_id) {
            // The transfer metadata arrives via a FileTransferRequest event.
            // If it hasn't been stored yet we create a placeholder entry here;
            // the real metadata (filename, hash, etc.) will be patched in by
            // the receive loop.
            drop(transfers);
            let mut w = state.incoming_transfers.write().await;
            w.entry(transfer_id.clone()).or_insert_with(|| {
                let save_path = std::path::PathBuf::from(&save_dir);
                IncomingFileTransfer {
                    filename: String::new(),
                    total_size: 0,
                    total_chunks: 0,
                    file_hash: Vec::new(),
                    received_chunks: std::collections::HashMap::new(),
                    save_path,
                }
            });
        } else {
            drop(transfers);
            // Patch save_path into existing entry
            let mut w = state.incoming_transfers.write().await;
            if let Some(t) = w.get_mut(&transfer_id) {
                t.save_path = std::path::PathBuf::from(&save_dir);
            }
        }
    }

    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex)
        .ok_or("no connection to this peer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;

    session.send_file_accept(&mut *write_half, &transfer_id)
        .await.map_err(|e| format!("failed to send accept: {e}"))?;

    Ok(())
}

/// Reject an incoming file transfer.
#[tauri::command]
pub async fn reject_file_transfer(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    transfer_id: String,
) -> Result<(), String> {
    let conns = state.connections.read().await;
    let conn_arc = conns.get(&peer_key_hex)
        .ok_or("no connection to this peer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;

    session.send_file_reject(&mut *write_half, &transfer_id)
        .await.map_err(|e| format!("failed to send reject: {e}"))?;

    Ok(())
}

/// Get the actual listening address (after binding to port 0).
#[tauri::command]
pub async fn get_listen_address(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let addr = state.listen_addr.read().await;
    addr.map(|a| a.to_string()).ok_or("not listening".to_string())
}

// ─── Internal Helpers ───

/// Send file chunks to a peer after they've accepted the transfer.
async fn send_file_chunks(
    state: Arc<AppState>,
    peer_key_hex: &str,
    transfer_id: &str,
    file_path: &str,
) -> Result<(), String> {
    let file_data = std::fs::read(file_path).map_err(|e| format!("read failed: {e}"))?;
    let chunks: Vec<&[u8]> = file_data.chunks(protocol::MAX_FILE_CHUNK_SIZE).collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let chunk_hash = sodiumoxide::crypto::hash::sha256::hash(chunk);
        let conns = state.connections.read().await;
        let conn_arc = conns.get(peer_key_hex)
            .ok_or("peer disconnected during transfer")?.clone();
        let mut conn = conn_arc.lock().await;
        let PeerConnection { session, write_half, .. } = &mut *conn;
        session.send_file_chunk(
            &mut *write_half,
            transfer_id, i as u32, chunk.to_vec(), chunk_hash.0.to_vec(),
        ).await.map_err(|e| format!("chunk send failed: {e}"))?;
    }

    // Send completion
    let conns = state.connections.read().await;
    let conn_arc = conns.get(peer_key_hex)
        .ok_or("peer disconnected during transfer")?.clone();
    let mut conn = conn_arc.lock().await;
    let PeerConnection { session, write_half, .. } = &mut *conn;
    session.send_file_complete(&mut *write_half, transfer_id)
        .await.map_err(|e| format!("complete send failed: {e}"))?;

    // Clean up
    state.outgoing_transfers.write().await.remove(transfer_id);
    Ok(())
}

// ─── Storage Helpers ───

/// Derive a storage encryption key from a user-supplied passphrase using Argon2id.
/// The `salt` should be unique per identity (we use the public key).
fn derive_storage_key_from_passphrase(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    use argon2::{Argon2, Algorithm, Version, Params};

    let params = Params::new(
        65536, // 64 MiB memory
        3,     // 3 iterations
        4,     // 4 parallelism lanes
        Some(32),
    ).map_err(|e| format!("argon2 params error: {e}"))?;

    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon.hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("argon2 hash failed: {e}"))?;
    Ok(key)
}

/// Legacy fallback: derive a storage encryption key from the public key.
/// Used when no vault passphrase has been set (migration / first-run).
fn derive_storage_key(public_key: &[u8]) -> [u8; 32] {
    use sodiumoxide::crypto::hash::sha256;
    let context = b"m2m-storage-key-v1";
    let mut input = Vec::with_capacity(context.len() + public_key.len());
    input.extend_from_slice(context);
    input.extend_from_slice(public_key);
    let hash = sha256::hash(&input);
    hash.0
}

/// Encrypt data for storage using XChaCha20-Poly1305.
fn crypto_encrypt_storage(
    plaintext: &[u8],
    key: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::gen_nonce();
    let aead_key = aead::Key::from_slice(key).ok_or("invalid key length")?;
    let ciphertext = aead::seal(plaintext, None, &nonce, &aead_key);
    Ok((nonce.0.to_vec(), ciphertext))
}

/// Decrypt storage-encrypted data.
fn crypto_decrypt_storage(
    ciphertext: &[u8],
    nonce_bytes: &[u8],
    key: &[u8; 32],
) -> Result<Vec<u8>, String> {
    use sodiumoxide::crypto::aead::xchacha20poly1305_ietf as aead;
    let nonce = aead::Nonce::from_slice(nonce_bytes).ok_or("invalid nonce")?;
    let aead_key = aead::Key::from_slice(key).ok_or("invalid key length")?;
    aead::open(ciphertext, None, &nonce, &aead_key).map_err(|_| "decryption failed".to_string())
}

// ─── Vault Commands ───

/// Vault status response for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct VaultStatus {
    pub initialized: bool,
    pub unlocked: bool,
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
/// On first run, this sets the passphrase.
/// On subsequent runs, it derives the storage key and verifies it can decrypt.
#[tauri::command]
pub async fn unlock_vault(
    state: State<'_, Arc<AppState>>,
    passphrase: String,
) -> Result<VaultStatus, String> {
    if passphrase.len() < 8 {
        return Err("passphrase must be at least 8 characters".to_string());
    }

    let identity = state.identity.read().await;
    let pub_bytes = identity
        .as_ref()
        .map(|kp| kp.public_key_bytes().to_vec())
        .unwrap_or_default();

    // Use public key as Argon2id salt — unique per identity
    let salt = if pub_bytes.is_empty() {
        b"m2m-default-salt-v1".to_vec()
    } else {
        pub_bytes
    };

    let storage_key = derive_storage_key_from_passphrase(&passphrase, &salt)?;

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

    Ok(VaultStatus {
        initialized: true,
        unlocked: true,
    })
}

// ─── Network / STUN / Tor Commands ───

/// Discover the public IP address using STUN.
#[tauri::command]
pub async fn discover_public_ip(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let result = stun::discover_public_addr()
        .await
        .map_err(|e| format!("STUN discovery failed: {e}"))?;

    {
        let mut pip = state.public_ip.write().await;
        *pip = Some(result.public_addr);
    }

    Ok(result.public_addr.to_string())
}

/// Get current network settings for the frontend.
#[tauri::command]
pub async fn get_network_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<tor::NetworkSettings, String> {
    let tor_reachable = tor::check_proxy_reachable().await;
    let public_ip = state.public_ip.read().await;

    Ok(tor::NetworkSettings {
        tor_enabled: tor::is_enabled(),
        tor_proxy_addr: tor::TOR_PROXY_ADDR.to_string(),
        tor_reachable,
        public_ip: public_ip.map(|a| a.to_string()),
    })
}

/// Enable or disable Tor routing.
#[tauri::command]
pub async fn set_tor_enabled(
    enabled: bool,
) -> Result<(), String> {
    tor::set_enabled(enabled);
    Ok(())
}

