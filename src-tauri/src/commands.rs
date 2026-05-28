/// M2M — Tauri Commands
///
/// IPC bridge between the React UI and the Rust backend.
/// Each command validates inputs and returns safe, typed responses.
/// No secrets are exposed to the frontend.
use std::net::SocketAddr;
use std::sync::Arc;

use tauri::State;
use tokio::sync::Mutex;

use crate::crypto::{self, IdentityKeypair};
use crate::identity;
use crate::network::{self, ConnectionState};
use crate::protocol;
use crate::session::Session;
use crate::state::{AppState, PeerConnection};

use serde::{Deserialize, Serialize};

/// Response types for the frontend — never contain secrets.

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub content: String,
    pub direction: String,
    pub timestamp: u64,
}

/// Initialize the crypto library and load/create identity.
#[tauri::command]
pub async fn init_identity(
    state: State<'_, Arc<AppState>>,
) -> Result<IdentityInfo, String> {
    // Initialize sodiumoxide
    crypto::init().map_err(|e| format!("crypto init failed: {e}"))?;

    // For MVP, generate a new identity if none exists.
    // In production, this would load from encrypted storage with passphrase.
    let keypair = IdentityKeypair::generate()
        .map_err(|e| format!("keypair generation failed: {e}"))?;

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

    // Validate address format
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteInfo {
    pub fingerprint: String,
    pub address_hint: String,
    pub expires_at: u64,
    pub one_time: bool,
    pub valid: bool,
}

/// Start listening for incoming connections.
#[tauri::command]
pub async fn start_listening(
    state: State<'_, Arc<AppState>>,
    address: String,
) -> Result<String, String> {
    let addr: SocketAddr = address
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<(tokio::net::TcpStream, SocketAddr)>(8);

    // Store listener state
    {
        let mut listen = state.listen_addr.write().await;
        *listen = Some(addr);
    }
    {
        let mut incoming = state.incoming_tx.lock().await;
        *incoming = Some(tx.clone());
    }

    // Spawn the listener task
    let state_clone = state.inner().clone();
    tokio::spawn(async move {
        if let Err(e) = network::start_listener(addr, tx).await {
            tracing::error!(error = %e, "listener failed");
        }
    });

    // Spawn the connection handler task
    let state_clone2 = state.inner().clone();
    tokio::spawn(async move {
        while let Some((mut stream, peer_addr)) = rx.recv().await {
            let state_inner = state_clone2.clone();
            tokio::spawn(async move {
                handle_incoming_connection(state_inner, stream, peer_addr).await;
            });
        }
    });

    tracing::info!(address = %addr, "started listening");
    Ok(format!("listening on {addr}"))
}

/// Handle an incoming connection: perform handshake as responder.
async fn handle_incoming_connection(
    state: Arc<AppState>,
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
) {
    // Read the first frame (should be HandshakeInit)
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

    // Get identity for handshake
    let identity = state.identity.read().await;
    let kp = match identity.as_ref() {
        Some(kp) => kp,
        None => {
            tracing::error!("cannot handle connection: no identity");
            return;
        }
    };

    // Perform handshake as responder
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

    // Store the connection
    let peer_key_hex = hex::encode(session.peer_identity_pub);
    let conn = PeerConnection {
        stream,
        session,
        remote_addr: peer_addr,
    };

    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.clone(), Arc::new(Mutex::new(conn)));
    tracing::info!(peer = %peer_key_hex, "peer connected and authenticated");
}

/// Connect to a peer using an invite link.
#[tauri::command]
pub async fn connect_to_peer(
    state: State<'_, Arc<AppState>>,
    invite_str: String,
) -> Result<ConnectionInfo, String> {
    // Validate the invite
    let signed = identity::validate_invite(&invite_str)
        .map_err(|e| format!("invite invalid: {e}"))?;

    // Parse the address
    let addr: SocketAddr = signed
        .payload
        .address_hint
        .parse()
        .map_err(|e| format!("invalid address in invite: {e}"))?;

    // Connect
    let mut stream = network::connect(addr)
        .await
        .map_err(|e| format!("connection failed: {e}"))?;

    // Get identity
    let identity = state.identity.read().await;
    let kp = identity
        .as_ref()
        .ok_or("identity not initialized")?;

    // Perform handshake as initiator
    let mut session = Session::new();
    session
        .handshake_as_initiator(&mut stream, kp, &signed.payload.identity_pub)
        .await
        .map_err(|e| format!("handshake failed: {e}"))?;

    let peer_fingerprint = session.peer_fingerprint();
    let peer_key_hex = hex::encode(session.peer_identity_pub);

    let conn = PeerConnection {
        stream,
        session,
        remote_addr: addr,
    };

    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.clone(), Arc::new(Mutex::new(conn)));

    Ok(ConnectionInfo {
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint),
        peer_verified: false,
    })
}

/// Send a text message to a connected peer.
#[tauri::command]
pub async fn send_message(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
    content: String,
) -> Result<ChatMessage, String> {
    // Validate message size
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
    let msg_id = conn
        .session
        .send_text(&mut conn.stream, &content)
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
