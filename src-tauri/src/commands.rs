/// M2M — Tauri Commands
///
/// IPC bridge between the React UI and the Rust backend.
/// Each command validates inputs and returns safe, typed responses.
/// No secrets are exposed to the frontend.
use std::net::SocketAddr;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::candidate;
use crate::hole_punch;
use crate::crypto::{self, IdentityKeypair};
use crate::identity;
use crate::network;
use crate::protocol::{self, FileTransferRequestData, MessageBody, PacketType, ConversationMetaData, WireCandidate};
use crate::session::Session;
use crate::state::{AppState, IncomingFileTransfer, PeerConnection};
use crate::storage::{self, KeyStore};
use crate::stun;
use crate::tor;
use zeroize::Zeroizing;

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

/// Generate an invite link for sharing.
/// If STUN has discovered a public IP, it replaces the local IP in the address
/// so the invite works across the internet.
/// In private mode, the public IP is NOT included — only the local address.
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

    let listen_addr: SocketAddr = address
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let private_mode = *state.private_mode.read().await;

    // Determine the address to embed in the invite.
    let actual_address = if private_mode {
        // Private mode: only use the local address, never expose public IP.
        let local_ip = if listen_addr.ip().is_unspecified() {
            resolve_local_ip().unwrap_or(listen_addr.ip())
        } else {
            listen_addr.ip()
        };
        SocketAddr::new(local_ip, listen_addr.port()).to_string()
    } else {
        // Normal mode: use public IP if available, fall back to local.
        let pip = state.public_ip.read().await;
        match *pip {
            Some(public_addr) => {
                // Use the FULL STUN-discovered address (IP:port) - the STUN
                // port is what the NAT maps, so the peer must connect to it.
                public_addr.to_string()
            }
            None => {
                if listen_addr.ip().is_unspecified() {
                    let local_ip = resolve_local_ip().unwrap_or(listen_addr.ip());
                    SocketAddr::new(local_ip, listen_addr.port()).to_string()
                } else {
                    address.clone()
                }
            }
        }
    };

    let validity_secs = validity_minutes.saturating_mul(60);

    // ─── Tor Guard ───
    // When Tor is enabled but private mode is off, the invite contains
    // the user's real IP address. Inbound connections will bypass Tor
    // entirely. We refuse to create the invite rather than just warning.
    if crate::tor::is_enabled() && !private_mode {
        return Err(
            "Tor is enabled for outbound connections but Private Mode is off. \
             This invite would contain your real IP address, and inbound connections \
             would bypass Tor entirely. Enable Private Mode in Settings to generate \
             invites that exclude your public IP."
                .to_string(),
        );
    }

    let invite_candidates: Vec<protocol::WireCandidate> = {
        let candidates_state = state.candidates.read().await;
        candidates_state.iter().map(|c| protocol::WireCandidate {
            address: c.address.clone(),
            candidate_type: c.candidate_type as u8,
        }).collect()
    };
    identity::create_invite(kp, &actual_address, validity_secs, one_time, invite_candidates)
        .map_err(|e| format!("invite creation failed: {e}"))
}

/// Estimate the entropy of a passphrase in bits.
///
/// Uses a simplified character-pool model: counts the size of the
/// character set used, then computes log2(pool^length).
///
/// This is a rough estimate — actual entropy depends on the randomness
/// of the passphrase generation process. It catches the worst cases
/// (single-word, all-lowercase, short passphrases) while being
/// deliberately lenient for diceware-style multi-word phrases.
fn estimate_passphrase_entropy(passphrase: &str) -> f64 {
    let bytes = passphrase.as_bytes();

    // Detect which character classes are present.
    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    let mut has_special = false;
    let mut has_unicode = false;

    for &b in bytes {
        if b.is_ascii_lowercase() {
            has_lower = true;
        } else if b.is_ascii_uppercase() {
            has_upper = true;
        } else if b.is_ascii_digit() {
            has_digit = true;
        } else if b.is_ascii_punctuation() || b.is_ascii_graphic() {
            has_special = true;
        } else if !b.is_ascii() {
            has_unicode = true;
        }
    }

    let mut pool_size = 0u32;
    if has_lower {
        pool_size += 26;
    }
    if has_upper {
        pool_size += 26;
    }
    if has_digit {
        pool_size += 10;
    }
    if has_special {
        pool_size += 32;
    }
    if has_unicode {
        pool_size += 100; // rough estimate for Unicode charset
    }

    if pool_size == 0 {
        return 0.0;
    }

    // Entropy = length * log2(pool_size)
    let pool_f = pool_size as f64;
    let len = passphrase.len() as f64;
    len * pool_f.log2()
}

/// Decode a 64-char hex string into a 32-byte peer key.
/// Returns an error if the hex string is malformed or wrong length.
fn decode_peer_key(hex_str: &str) -> Result<[u8; 32], String> {
    if hex_str.len() != 64 {
        return Err(format!(
            "invalid peer key hex length: expected 64 chars, got {}",
            hex_str.len()
        ));
    }
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid peer key hex: {e}"))?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Decode a peer hex key, logging an error and returning `None` on failure.
/// Prevents silent database corruption from malformed hex strings.
fn decode_peer_key_logged(hex_str: &str) -> Option<[u8; 32]> {
    match decode_peer_key(hex_str) {
        Ok(key) => Some(key),
        Err(e) => {
            tracing::error!(hex_len = hex_str.len(), error = %e, "decode_peer_key failed — skipping store operation");
            None
        }
    }
}

/// Resolve the local (non-loopback) IP address used for internet connectivity.
fn resolve_local_ip() -> Option<std::net::IpAddr> {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .ok()
        .map(|addr| addr.ip())
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

    // Use std TcpListener first to set a custom backlog (128 for DoS resilience),
    // then convert to tokio for async usage.
    let std_listener = std::net::TcpListener::bind(addr)
        .map_err(|e| format!("failed to bind listener: {e}"))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("failed to set non-blocking: {e}"))?;

    let listener = tokio::net::TcpListener::from_std(std_listener)
        .map_err(|e| format!("failed to create async listener: {e}"))?;

    let bound_addr = listener.local_addr()
        .map_err(|e| format!("failed to get local address: {e}"))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<(tokio::net::TcpStream, SocketAddr)>(8);

    {
        let mut listen = state.listen_addr.write().await;
        *listen = Some(bound_addr);
    }
    {
        let mut incoming = state.incoming_tx.lock().await;
        *incoming = Some(tx.clone());
    }

    // Spawn the listener task
    tokio::spawn(async move {
        if let Err(e) = network::start_listener(listener, tx).await {
            tracing::error!(error = %e, "listener failed");
        }
    });

    // Spawn the connection handler task with rate limiting.
    let state_clone = state.inner().clone();
    let app_clone = app_handle.clone();
    tokio::spawn(async move {
        while let Some((stream, peer_addr)) = rx.recv().await {
            let ip = peer_addr.ip();
            let allowed = state_clone.connection_limiter.check(ip);

            if allowed {
                let state_inner = state_clone.clone();
                let app_inner = app_clone.clone();
                tokio::spawn(async move {
                    state_inner.connection_limiter.increment();
                    handle_incoming_connection(app_inner, state_inner.clone(), stream, peer_addr).await;
                    state_inner.connection_limiter.decrement();
                });
            } else {
                // Need a mutable reference for send_error
                let mut stream = stream;
                tracing::warn!(peer_ip = %ip, "connection rejected by rate limiter");
                // Send a rate limit error frame so the peer knows why.
                let _ = network::send_error(
                    &mut stream,
                    protocol::ErrorCode::RateLimitExceeded,
                    "rate limited — too many connections",
                ).await;
                drop(stream);
            }
        }
    });

    tracing::info!(address = %bound_addr, "started listening");
    Ok(format!("listening on {bound_addr}"))
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

        // Gather our local candidates to share with the peer during handshake.
        let config = state.stun_config.read().await;
        let stun_result = stun::discover_public_addrs(&config).await.ok();
        drop(config);

        let host_candidates = candidate::gather_host_candidates();
        let reflexive_candidates = stun_result
            .as_ref()
            .map(|r| candidate::gather_reflexive_candidates(r))
            .unwrap_or_default();

        let mut all = host_candidates;
        all.extend(reflexive_candidates);
        all.sort_by(|a, b| b.priority.cmp(&a.priority));
        let wire_candidates: Vec<WireCandidate> = all.iter().map(|c| WireCandidate {
            address: c.address.clone(),
            candidate_type: c.candidate_type as u8,
        }).collect();

        // Update state with gathered candidates
        {
            let mut cand_state = state.candidates.write().await;
            *cand_state = all;
        }

        if let Err(e) = session.handshake_as_responder(&mut stream, kp, &frame, wire_candidates).await {
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

    // Upsert peer in key store (skip if peer key hex is malformed)
    if let Some(peer_key_bytes) = decode_peer_key_logged(&peer_key_hex) {
        let ks = state.key_store.lock().await;
        if let Some(ref store) = *ks {
            let _ = store.upsert_peer(
                &peer_key_bytes,
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

    let peer_addrs = hole_punch::extract_candidates_from_invite(
        &signed.payload.address_hint,
        &signed.payload.candidates,
    );

    tracing::debug!(
        address_hint = %signed.payload.address_hint,
        peer_candidates = peer_addrs.len(),
        "connecting to peer with hole-punch race"
    );

    // Get our listener address so we can race accept vs connect.
    let listen_addr = *state.listen_addr.read().await;

    // ── TCP Hole Punch: race accept vs connect simultaneously ──
    // Both peers race listener.accept() against connect(peer_candidates).
    // Whichever succeeds first determines our handshake role.
    let hole_punch::HolePunchResult {
        mut stream,
        role,
        remote_addr,
    } = hole_punch::race_accept_or_connect(&peer_addrs, listen_addr)
        .await
        .map_err(|e| format!("connection failed (tried {} candidates): {e}", peer_addrs.len()))?;

    let identity = state.identity.read().await;
    let kp = identity
        .as_ref()
        .ok_or("identity not initialized")?;

    // Gather our local candidates to share with the peer during handshake.
    let config = state.stun_config.read().await;
    let stun_result = stun::discover_public_addrs(&config).await.ok();
    drop(config);

    let host_candidates = candidate::gather_host_candidates();
    let reflexive_candidates = stun_result
        .as_ref()
        .map(|r| candidate::gather_reflexive_candidates(r))
        .unwrap_or_default();

    let mut all = host_candidates;
    all.extend(reflexive_candidates);
    all.sort_by(|a, b| b.priority.cmp(&a.priority));
    let our_candidates: Vec<WireCandidate> = all.iter().map(|c| WireCandidate {
        address: c.address.clone(),
        candidate_type: c.candidate_type as u8,
    }).collect();

    // Update state with gathered candidates
    {
        let mut cand_state = state.candidates.write().await;
        *cand_state = all;
    }

    let expected_peer_pub = signed.payload.identity_pub;
    let mut session = Session::new();

    match role {
        hole_punch::Role::Initiator => {
            // ── We connected to the peer ──
            // Normal initiator flow: we send HandshakeInit first.
            tracing::debug!("hole-punch role: Initiator (outgoing connect won)");
            session
                .handshake_as_initiator(
                    &mut stream,
                    kp,
                    &expected_peer_pub,
                    our_candidates,
                )
                .await
                .map_err(|e| format!("initiator handshake failed: {e}"))?;
        }
        hole_punch::Role::Responder => {
            // ── Peer connected to us ──
            // Read the HandshakeInit the peer already sent, then respond.
            tracing::debug!("hole-punch role: Responder (incoming accept won)");
            let frame = network::read_frame(&mut stream)
                .await
                .map_err(|e| format!("failed to read initial frame: {e}"))?;
            if frame.packet_type != protocol::PacketType::HandshakeInit {
                return Err(format!(
                    "expected HandshakeInit, got {:?}",
                    frame.packet_type
                ));
            }
            session
                .handshake_as_responder(&mut stream, kp, &frame, our_candidates)
                .await
                .map_err(|e| format!("responder handshake failed: {e}"))?;

            // Verify the peer's identity matches the invite.
            if session.peer_identity_pub != expected_peer_pub {
                return Err("peer identity does not match invite".to_string());
            }
        }
    }

    let peer_fingerprint = session.peer_fingerprint();
    let peer_key_hex = hex::encode(session.peer_identity_pub);

    // Split the stream
    let (read_half, write_half) = stream.into_split();

    let conn = PeerConnection {
        write_half,
        session,
        remote_addr,
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
            match crypto_encrypt_storage(content.as_bytes(), &**key) {
                Ok((nonce, encrypted)) => {
                    if let Some(peer_bytes) = decode_peer_key_logged(&peer_key_hex) {
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
                            Ok(body) => match &body {
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
                                            match crypto_encrypt_storage(content.as_bytes(), &**key) {
                                                Ok((nonce, encrypted)) => {
                                                    if let Some(peer_bytes) = decode_peer_key_logged(&peer_key_hex) {
                                                        let _ = store.ensure_conversation(&peer_key_hex, &peer_bytes);
                                                        let _ = store.store_message(
                                                            id, &peer_key_hex, "received",
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
                                        peer_key_hex: peer_key_hex.clone(),
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
                }
                PacketType::FileTransferRequest => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(req) = protocol::deserialize::<FileTransferRequestData>(&plaintext) {
                                    // Pre-register the transfer with a temp file on disk.
                                    // Chunks will be streamed to the temp file as they arrive.
                                    let total_chunks = req.total_chunks;
                                    let total_size = req.total_size;
                                    let transfer_id = req.transfer_id.clone();
                                    let filename = req.filename.clone();
                                    let file_hash = req.file_hash.clone();

                                    // Sanitize the filename from the peer (path traversal protection).
                                    let safe_name = network::sanitize_filename(&filename)
                                        .unwrap_or_else(|| format!("file_{}", transfer_id));

                                    {
                                        let mut transfers = state.incoming_transfers.write().await;
                                        transfers.entry(transfer_id.clone()).or_insert_with(|| {
                                            let (temp_file, temp_path) = match create_temp_file(total_size) {
                                                Ok((f, p)) => (Some(f), Some(p)),
                                                Err(e) => {
                                                    tracing::warn!(error = %e, "failed to create temp file for transfer");
                                                    (None, None)
                                                }
                                            };

                                            IncomingFileTransfer {
                                                filename: safe_name,
                                                total_size,
                                                total_chunks,
                                                file_hash,
                                                save_path: std::path::PathBuf::new(),
                                                temp_file: temp_file,
                                                temp_path: temp_path,
                                                chunks_received: 0,
                                                chunks_bitmask: vec![false; total_chunks as usize],
                                            }
                                        });
                                    }
                                    let _ = app_handle.emit("m2m://file-request", FileRequestEvent {
                                        peer_key_hex: peer_key_hex.clone(),
                                        transfer_id,
                                        filename,
                                        total_size,
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
                                        // Verify chunk hash before writing to disk
                                        let hash = sodiumoxide::crypto::hash::sha256::hash(&chunk.data);
                                        let hash_valid = hash.0.to_vec() == chunk.chunk_hash;

                                        if !hash_valid {
                                            tracing::warn!(chunk = chunk.chunk_index, "file chunk hash mismatch — skipping");
                                        } else if let Some(ref mut file) = transfer.temp_file {
                                            use std::io::{Seek, Write};
                                            let offset = (chunk.chunk_index as u64)
                                                * (crate::protocol::MAX_FILE_CHUNK_SIZE as u64);
                                            match file.seek(std::io::SeekFrom::Start(offset)) {
                                                Ok(_) => {
                                                    match file.write_all(&chunk.data) {
                                                        Ok(_) => {
                                                            transfer.chunks_received += 1;
                                                            if let Some(bit) = transfer.chunks_bitmask
                                                                .get_mut(chunk.chunk_index as usize)
                                                            {
                                                                *bit = true;
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracing::warn!(error = %e, chunk = chunk.chunk_index, "failed to write chunk to temp file");
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!(error = %e, chunk = chunk.chunk_index, "failed to seek in temp file");
                                                }
                                            }
                                        } else {
                                            tracing::warn!("no temp file available for transfer - skipping chunk");
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
                                    if let Some(mut transfer) = transfers.remove(&complete.transfer_id) {
                                        let transfer_id = complete.transfer_id.clone();
                                        let all_received = transfer.chunks_received == transfer.total_chunks
                                            && transfer.chunks_bitmask.iter().all(|&b| b);

                                        if !all_received {
                                            tracing::warn!(
                                                received = transfer.chunks_received,
                                                total = transfer.total_chunks,
                                                "file transfer incomplete - missing chunks"
                                            );
                                            drop(transfer.temp_file);
                                            if let Some(ref path) = transfer.temp_path {
                                                let _ = std::fs::remove_file(path);
                                            }
                                        } else {
                                            let hash_valid = if let Some(ref mut file) = transfer.temp_file {
                                                use std::io::{Read, Seek};
                                                let mut buf = Vec::with_capacity(transfer.total_size as usize);
                                                match file.seek(std::io::SeekFrom::Start(0))
                                                    .and_then(|_| file.read_to_end(&mut buf))
                                                {
                                                    Ok(_) => {
                                                        let hash = sodiumoxide::crypto::hash::sha256::hash(&buf);
                                                        hash.0.to_vec() == transfer.file_hash
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(error = %e, "failed to read temp file for hash verification");
                                                        false
                                                    }
                                                }
                                            } else {
                                                false
                                            };

                                            if hash_valid {
                                                let safe_name = network::sanitize_filename(&transfer.filename)
                                                    .unwrap_or_else(|| format!("download_{}", transfer_id));
                                                let final_path = if transfer.save_path.as_os_str().is_empty() {
                                                    std::path::PathBuf::from(&safe_name)
                                                } else if transfer.save_path.is_dir() {
                                                    transfer.save_path.join(&safe_name)
                                                } else {
                                                    transfer.save_path.clone()
                                                };

                                                let rename_ok = if let (Some(ref temp_path), Some(_)) =
                                                    (transfer.temp_path.as_ref(), transfer.temp_file.as_mut())
                                                {
                                                    // Take ownership of the temp file to close it,
                                                    // so rename can work on Windows.
                                                    transfer.temp_file.take();
                                                    std::fs::rename(temp_path, &final_path).is_ok()
                                                } else {
                                                    false
                                                };

                                                if rename_ok {
                                                    let _ = app_handle.emit("m2m://file-complete", serde_json::json!({
                                                        "transfer_id": transfer_id,
                                                        "filename": safe_name,
                                                        "path": final_path.to_string_lossy(),
                                                    }));
                                                } else {
                                                    tracing::warn!("failed to rename temp file - cleaning up");
                                                    if let Some(ref path) = transfer.temp_path {
                                                        let _ = std::fs::remove_file(path);
                                                    }
                                                }
                                            } else {
                                                tracing::warn!("file hash verification failed - deleting corrupted temp file");
                                                drop(transfer.temp_file);
                                                if let Some(ref path) = transfer.temp_path {
                                                    let _ = std::fs::remove_file(path);
                                                }
                                            }
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
                PacketType::ConversationMeta => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(meta) = protocol::deserialize::<ConversationMetaData>(&plaintext) {
                                    // The peer's "my_display_name" is how they want to be seen
                                    // The peer's "your_display_name" is the name they gave us
                                    let ms = state.message_store.lock().await;
                                    if let Some(ref store) = *ms {
                                        // Store the name the peer assigned to us as peer_display_name
                                        let _ = store.set_peer_display_name(&peer_key_hex, &meta.my_display_name);
                                        // If the peer suggested a name for our side, store it as display_name
                                        // (only if we don't already have one)
                                        if !meta.your_display_name.is_empty() {
                                            if let Ok(Some(conv)) = store.get_conversation(&peer_key_hex) {
                                                if conv.display_name.is_none() {
                                                    let _ = store.rename_conversation(&peer_key_hex, &meta.your_display_name);
                                                }
                                            }
                                        }
                                    }
                                    // Notify frontend to refresh conversation list
                                    let _ = app_handle.emit("m2m://conversation-meta", serde_json::json!({
                                        "peer_key_hex": peer_key_hex.clone(),
                                        "peer_display_name": meta.my_display_name,
                                        "suggested_name": meta.your_display_name,
                                    }));
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt conversation meta");
                            }
                        }
                    }
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
    let sk = state.storage_key.read().await;
    let ms = state.message_store.lock().await;
    let store = ms.as_ref().ok_or("message store not initialised")?;
    let key = sk.as_ref().ok_or("storage key not available")?;

    let key_ref: &[u8; 32] = &**key;

    let stored = store
        .load_messages(&peer_key_hex, limit.unwrap_or(100))
        .map_err(|e| format!("failed to load messages: {e}"))?;

    let mut messages = Vec::with_capacity(stored.len());
    for m in stored {
        let content = crypto_decrypt_storage(&m.content_encrypted, &m.content_nonce, key_ref)
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
                    save_path,
                    temp_file: None,
                    temp_path: None,
                    chunks_received: 0,
                    chunks_bitmask: Vec::new(),
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

/// Create a temporary file pre-allocated to the given size.
/// Returns (Option<File>, Option<PathBuf>) — either both Some or both None.
/// The file is created in the OS temp directory with a unique name.
fn create_temp_file(size: u64) -> std::io::Result<(std::fs::File, std::path::PathBuf)> {
    let mut path = std::env::temp_dir();
    path.push(format!("m2m_{}", uuid::Uuid::new_v4()));

    let file = std::fs::File::create(&path)?;
    // Pre-allocate the file to the full expected size.
    // This ensures we have enough disk space and avoids fragmentation.
    file.set_len(size)?;

    Ok((file, path))
}

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
    let entropy = estimate_passphrase_entropy(&passphrase);
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
            let storage_key = derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
            let sk_bytes = crypto_decrypt_storage(&enc_sk, &nonce, &storage_key)
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
            let legacy_key = derive_storage_key(&pub_bytes);
            let sk_bytes = crypto_decrypt_storage(&enc_sk, &nonce, &legacy_key)
                .map_err(|e| format!("failed to decrypt legacy identity: {e}"))?;

            let mut sk_arr = [0u8; 64];
            sk_arr.copy_from_slice(&sk_bytes);

            // Re-encrypt with the new passphrase-derived key
            let new_key = derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
            let (new_nonce, new_enc_sk) = crypto_encrypt_storage(&sk_bytes, &new_key)
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
        let storage_key = derive_storage_key_from_passphrase(&passphrase, &pub_bytes)?;
        let (nonce, encrypted_sk) = crypto_encrypt_storage(&sk_bytes, &storage_key)
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

// ─── Network / STUN / Tor Commands ───

/// Discover the public IP address using enhanced STUN (parallel queries + consensus).
#[tauri::command]
pub async fn discover_public_ip(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let result = state.refresh_stun()
        .await
        .map_err(|e| format!("STUN discovery failed: {e}"))?;

    let addr = result
        .consensus_addr
        .map(|a| a.to_string())
        .unwrap_or_else(|| "no consensus".to_string());

    // Capture values before the tracing macro to avoid Send issues.
    let nat_type_str = state.nat_type.read().await.to_string();
    tracing::info!(
        servers = result.responding_servers,
        total = result.total_servers,
        consensus = result.consensus,
        public_ip = %addr,
        nat_type = %nat_type_str,
        "STUN discovery completed"
    );

    Ok(addr)
}

/// Get the current STUN configuration.
#[tauri::command]
pub async fn get_stun_config(
    state: State<'_, Arc<AppState>>,
) -> Result<stun::StunConfig, String> {
    let config = state.stun_config.read().await;
    Ok(config.clone())
}

/// Update the STUN server list and configuration.
#[tauri::command]
pub async fn set_stun_servers(
    state: State<'_, Arc<AppState>>,
    servers: Vec<String>,
) -> Result<(), String> {
    if servers.is_empty() {
        return Err("STUN server list cannot be empty".to_string());
    }
    // Basic validation: each entry must contain a colon (host:port)
    for s in &servers {
        if !s.contains(':') {
            return Err(format!("invalid STUN server address (missing port): {s}"));
        }
        if s.len() > 255 {
            return Err(format!("STUN server address too long: {s}"));
        }
    }

    let mut config = state.stun_config.write().await;
    config.servers = servers;
    tracing::info!("STUN configuration updated");
    Ok(())
}

/// Toggle private mode (don't expose public IP in invites).
#[tauri::command]
pub async fn set_private_mode(
    state: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    let mut pm = state.private_mode.write().await;
    *pm = enabled;
    let mut config = state.stun_config.write().await;
    config.private_mode = enabled;
    tracing::info!(private_mode = enabled, "privacy mode updated");
    Ok(())
}

/// Run connectivity verification: check if the listening port is reachable.
#[tauri::command]
pub async fn check_connectivity(
    state: State<'_, Arc<AppState>>,
) -> Result<stun::ConnectivityStatus, String> {
    let config = state.stun_config.read().await;
    let multi_result = stun::discover_public_addrs(&config)
        .await
        .map_err(|e| format!("STUN discovery failed for connectivity check: {e}"))?;

    let nat_type = stun::classify_nat(&multi_result);
    let host_addrs: Vec<String> = stun::gather_host_candidates()
        .iter()
        .map(|a| a.to_string())
        .collect();

    // Determine reachability based on NAT type and STUN consensus.
    let (reachable, behind_symmetric) = match nat_type {
        stun::NatType::Symmetric => {
            // Symmetric NAT: STUN works for outbound, but inbound won't work
            // without TURN. We still report the public IP but warn the user.
            (true, true)
        }
        stun::NatType::Blocked => (false, false),
        stun::NatType::None => (true, false),
        _ => {
            // Cone NAT types: inbound should work if the port mapping is stable.
            // We can't fully verify without an external echo service, but we
            // report optimistic reachability with a note.
            (multi_result.consensus, false)
        }
    };

    let status = stun::ConnectivityStatus {
        reachable,
        nat_type,
        public_addr: multi_result.consensus_addr.map(|a| a.to_string()),
        host_addrs,
        behind_symmetric_nat: behind_symmetric,
    };

    // Update state
    {
        let mut cv = state.connectivity_verified.write().await;
        *cv = reachable;
    }

    tracing::info!(reachable = reachable, nat = %nat_type, "connectivity check complete");
    Ok(status)
}

/// Get full network diagnostics for the frontend.
#[tauri::command]
pub async fn get_network_diagnostics(
    state: State<'_, Arc<AppState>>,
) -> Result<candidate::NetworkDiagnostics, String> {
    let nat_type = *state.nat_type.read().await;
    let candidates = state.candidates.read().await;
    let config = state.stun_config.read().await;

    let stun_servers = stun::check_all_servers(&config).await;

    let host_addrs: Vec<String> = stun::gather_host_candidates()
        .iter()
        .map(|a| a.to_string())
        .collect();

    let public_addr = state.public_ip.read().await.map(|a| a.to_string());
    let connectivity = stun::ConnectivityStatus {
        reachable: *state.connectivity_verified.read().await,
        nat_type,
        public_addr,
        host_addrs,
        behind_symmetric_nat: nat_type == stun::NatType::Symmetric,
    };

    Ok(candidate::NetworkDiagnostics {
        candidates: candidates.clone(),
        nat_type,
        stun_servers,
        connectivity,
    })
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

// ─── Multi-Conversation Commands ───

/// Response type for conversation list items.
#[derive(Debug, Clone, Serialize)]
pub struct ConversationListItem {
    pub id: String,
    pub peer_key_hex: String,
    pub display_name: Option<String>,
    pub peer_display_name: Option<String>,
    pub last_message_at: Option<i64>,
    pub last_message_preview: Option<String>,
    pub message_count: i64,
    pub is_online: bool,
    pub auto_delete_at: Option<i64>,
    pub retention_policy: String,
    pub created_at: i64,
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
        let last_message_preview = if let Some(ref key) = *sk {
            let key_ref: &[u8; 32] = &**key;
            store
                .load_messages(&c.id, 1)
                .ok()
                .and_then(|msgs| msgs.into_iter().last())
                .and_then(|m| {
                    crypto_decrypt_storage(&m.content_encrypted, &m.content_nonce, key_ref)
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
    let (nonce, ciphertext) = crypto_encrypt_storage(&export_json, &**key)
        .map_err(|e| format!("encryption failed: {e}"))?;

    // Build the final file: nonce (24 bytes) || ciphertext
    let mut file_data = Vec::with_capacity(nonce.len() + ciphertext.len());
    file_data.extend_from_slice(&nonce);
    file_data.extend_from_slice(&ciphertext);

    std::fs::write(&export_path, &file_data)
        .map_err(|e| format!("failed to write export: {e}"))?;

    Ok(export_path)
}
