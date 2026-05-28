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
use crate::state::{AppState, PeerConnection};
use crate::storage::{self, KeyStore};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub content: String,
    pub direction: String,
    pub timestamp: u64,
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

    let key_store = KeyStore::open(&keys_db_path)
        .map_err(|e| format!("key store error: {e}"))?;

    let keypair = if key_store.has_identity().unwrap_or(false) {
        // Load existing identity
        let (pub_bytes, enc_sk, nonce) = key_store
            .load_identity()
            .map_err(|e| format!("failed to load identity: {e}"))?;

        // For MVP, the private key is stored with a hardcoded derivation.
        // In production, this would prompt for a passphrase.
        let mut pub_arr = [0u8; 32];
        pub_arr.copy_from_slice(&pub_bytes);

        // Decrypt the private key using the storage encryption key
        let storage_key = derive_storage_key(&pub_bytes);
        let sk_bytes = crypto_decrypt_storage(&enc_sk, &nonce, &storage_key)
            .map_err(|e| format!("failed to decrypt identity: {e}"))?;

        let mut sk_arr = [0u8; 64];
        sk_arr.copy_from_slice(&sk_bytes);

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

        kp
    };

    let fingerprint = keypair.fingerprint();
    let pub_hex = hex::encode(keypair.public_key_bytes());

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

    let identity = state.identity.read().await;
    let kp = match identity.as_ref() {
        Some(kp) => kp,
        None => {
            tracing::error!("cannot handle connection: no identity");
            return;
        }
    };

    let mut session = Session::new();
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
    
    drop(identity);

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
        peer_fingerprint: Some(peer_fingerprint),
    });

    tracing::info!(peer = %peer_key_hex, "peer connected and authenticated");

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
    let PeerConnection { session, write_half, .. } = &mut *conn;
    let msg_id = session
        .send_text(write_half, &content)
        .await
        .map_err(|e| format!("send failed: {e}"))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

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

// ─── Storage Helpers ───

/// Derive a storage encryption key from the public key.
/// In production, this should use Argon2id with a user passphrase.
/// For MVP, we use a deterministic derivation so the app works without a passphrase prompt.
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
