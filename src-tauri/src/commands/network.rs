//! Network connection commands.
//!
//! Handles invite creation/validation, TCP listening, peer connection
//! (via hole-punch race), connection state management, and the async
//! receive loop that dispatches all inbound packet types.

use std::net::SocketAddr;
use std::sync::Arc;

use chrono::Utc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::candidate;
use crate::crypto;
use crate::hole_punch;
use crate::identity;
use crate::network;
use crate::relay;
use crate::protocol::{self, FileTransferRequestData, MessageBody, PacketType, ConversationMetaData, WireCandidate};
use crate::session::Session;
use crate::state::{AppState, PeerConnection, IncomingFileTransfer};
use crate::stun;

use super::util;
use super::{ConnectionEvent, ConnectionInfo, FileRequestEvent, InviteInfo, MessageEvent, ChatMessage, GroupEvent, GroupMessageEvent};

/// Generate an invite link for sharing.
/// If STUN has discovered a public IP, it replaces the local IP in the address
/// so the invite works across the internet.
/// In private mode, the public IP is NOT included — only the local address.
#[tauri::command]
pub async fn create_invite(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    address: String,
    validity_minutes: u64,
    one_time: bool,
) -> Result<String, String> {
    let identity = state.identity.read().await;
    let kp = identity
        .as_ref()
        .ok_or("identity not initialized")?;

    // ─── X3DH Prekey Bundle ───
    let x25519 = state.x25519_identity.read().await;
    let x25519_kp = x25519.as_ref()
        .ok_or("X25519 identity not initialized")?;
    // Generate a signed prekey for this invite
    let spk = crate::crypto::EphemeralKeypair::generate();
    let spk_pub = spk.public_key_bytes();
    let spk_sig = kp.sign(&spk_pub);
    // Store the signed prekey private key for incoming handshakes
    {
        let mut active_spk = state.active_signed_prekey.write().await;
        *active_spk = Some(spk);
    }

    let listen_addr: SocketAddr = address
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let private_mode = *state.private_mode.read().await;

    // Determine the address to embed in the invite.
    let actual_address = if private_mode {
        // Private mode: only use the local address, never expose public IP.
        let local_ip = if listen_addr.ip().is_unspecified() {
            util::resolve_local_ip().unwrap_or(listen_addr.ip())
        } else {
            listen_addr.ip()
        };
        SocketAddr::new(local_ip, listen_addr.port()).to_string()
    } else {
        // Normal mode: use public IP if available, fall back to local.
        let pip = state.public_ip.read().await;
        match *pip {
            Some(public_addr) => {
                // Use the FULL STUN-discovered address (IP:port) — the STUN
                // port is what the NAT maps, so the peer must connect to it.
                public_addr.to_string()
            }
            None => {
                if listen_addr.ip().is_unspecified() {
                    let local_ip = util::resolve_local_ip().unwrap_or(listen_addr.ip());
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

    // ─── Try NAT port mapping (UPnP / NAT-PMP / PCP) ───
    // If the router supports port mapping protocols we can obtain a
    // guaranteed public address. This is more reliable than STUN's
    // UDP-only discovery and gives the peer a direct TCP path.
    let port_mapping = if !private_mode {
        match crate::port_mapping::PortMapper::add_port_mapping(
            listen_addr.port(),
            3600, // 1 hour — the router may grant less
        )
        .await
        {
            Ok(mapping) => {
                tracing::info!(
                    protocol = mapping.protocol,
                    external = %mapping.external_addr,
                    "NAT port mapping obtained"
                );
                Some(mapping)
            }
            Err(e) => {
                tracing::debug!(error = %e, "NAT port mapping unavailable");
                None
            }
        }
    } else {
        None
    };

    // ─── Relay Registration ───
    // If a relay server is configured, register to get a relay_id and add
    // a relay candidate as a fallback. The relay stream is passed to a
    // background listener task that waits for incoming bridges.
    let mut relay_registered_id: Option<String> = None;
    let mut relay_addr_str: Option<String> = None;
    if !private_mode {
        let relay_cfg = state.relay_config.read().await;
        if let Some(ref config) = *relay_cfg {
            match relay::register(config).await {
                Ok((relay_stream, rid)) => {
                    tracing::info!(relay_id = %rid, relay = %config.addr_str(), "relay registered for invite");

                    // Spawn the relay listener task
                    let state_clone = state.inner().clone();
                    let app = app_handle.clone();
                    tokio::spawn(async move {
                        relay::wait_for_bridge(relay_stream, state_clone, app).await;
                    });

                    // Update relay state
                    {
                        let mut rs = state.relay_state.write().await;
                        *rs = relay::RelayState {
                            connected: true,
                            relay_id: Some(rid.clone()),
                            error: None,
                        };
                    }

                    relay_registered_id = Some(rid);
                    relay_addr_str = Some(config.addr_str());
                }
                Err(e) => {
                    tracing::warn!(error = %e, "relay registration failed, continuing without relay");
                }
            }
        }
    }

    let invite_candidates: Vec<protocol::WireCandidate> = {
        let candidates_state = state.candidates.read().await;
        let mut all: Vec<protocol::WireCandidate> = candidates_state
            .iter()
            .map(|c| protocol::WireCandidate {
                address: c.address.clone(),
                candidate_type: c.candidate_type as u8,
                relay_id: None,
            })
            .collect();

        // If we obtained a NAT port mapping, add it as a high-priority
        // candidate (type 4 = port-mapped).
        if let Some(ref pm) = port_mapping {
            let addr_str = pm.external_addr.to_string();
            if !all.iter().any(|c| c.address == addr_str) {
                all.push(protocol::WireCandidate {
                    address: addr_str,
                    candidate_type: 4,
                    relay_id: None,
                });
            }
        }

        // Append user-configured manual port forwards as type 4 candidates.
        let mf = state.manual_forwards.read().await;
        for fwd in mf.iter() {
            if fwd.listen_port == listen_addr.port()
                && !all.iter().any(|c| c.address == fwd.public_addr)
            {
                all.push(protocol::WireCandidate {
                    address: fwd.public_addr.clone(),
                    candidate_type: 4,
                    relay_id: None,
                });
            }
        }

        // Add relay candidate if registration succeeded.
        if let (Some(ref addr), Some(ref rid)) = (relay_addr_str, relay_registered_id) {
            all.push(protocol::WireCandidate {
                address: addr.clone(),
                candidate_type: 3,
                relay_id: Some(rid.clone()),
            });
            tracing::debug!(relay_addr = %addr, relay_id = %rid, "relay candidate added to invite");
        }

        all
    };
    identity::create_invite(
        kp,
        &actual_address,
        validity_secs,
        one_time,
        invite_candidates,
        Some(&crate::crypto::PrekeyBundle {
            identity_key: x25519_kp.public_key_bytes(),
            signed_prekey: spk_pub,
            signed_prekey_sig: spk_sig,
            one_time_prekey: None,
        }),
    )
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

    let is_x3dh = frame.packet_type == protocol::PacketType::X3DHHandshakeInit;
    if !is_x3dh && frame.packet_type != protocol::PacketType::HandshakeInit {
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
        let ipv6_candidates = candidate::gather_ipv6_candidates();
        let reflexive_candidates = stun_result
            .as_ref()
            .map(candidate::gather_reflexive_candidates)
            .unwrap_or_default();

        let mut all = host_candidates;
        all.extend(ipv6_candidates);
        all.extend(reflexive_candidates);
        all.sort_by(|a, b| b.priority.cmp(&a.priority));
        let wire_candidates: Vec<WireCandidate> = all.iter().map(|c| WireCandidate {
            address: c.address.clone(),
            candidate_type: c.candidate_type as u8,
            relay_id: None,
        }).collect();

        // Update state with gathered candidates
        {
            let mut cand_state = state.candidates.write().await;
            *cand_state = all;
        }

        if is_x3dh {
            // X3DH handshake path
            let x25519 = state.x25519_identity.read().await;
            let x25519_kp = match x25519.as_ref() {
                Some(kp) => kp,
                None => {
                    tracing::error!("no X25519 identity for X3DH handshake");
                    return;
                }
            };
            let spk_lock = state.active_signed_prekey.read().await;
            let spk = match spk_lock.as_ref() {
                Some(spk) => spk,
                None => {
                    tracing::error!("no signed prekey for X3DH handshake");
                    return;
                }
            };
            if let Err(e) = session.handshake_as_responder_x3dh(
                &mut stream, kp, x25519_kp, spk, &frame, wire_candidates,
            ).await {
                tracing::warn!(error = %e, "X3DH handshake failed for incoming connection");
                let _ = network::send_error(&mut stream, protocol::ErrorCode::HandshakeFailed, "x3dh handshake failed").await;
                return;
            }
        } else {
            // Legacy handshake path (use X25519 pub key for the `x25519_identity_pub` field)
            let x25519_pub = state.x25519_identity.read().await
                .as_ref().map(|k| k.public_key_bytes()).unwrap_or([0u8; 32]);
            if let Err(e) = session.handshake_as_responder(&mut stream, kp, &frame, wire_candidates, x25519_pub).await {
                tracing::warn!(error = %e, "handshake failed for incoming connection");
                let _ = network::send_error(
                    &mut stream,
                    protocol::ErrorCode::HandshakeFailed,
                    "handshake failed",
                )
                .await;
                return;
            }
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
        strategy_name: "incoming".to_string(),
    };

    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.clone(), Arc::new(Mutex::new(conn)));
    drop(conns);

    // Notify frontend
    let _ = app_handle.emit("m2m://connection", ConnectionEvent {
        peer_key_hex: peer_key_hex.clone(),
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint.clone()),
        peer_verified: false, // Incoming connections start unverified
    });

    tracing::info!(peer = %peer_key_hex, "peer connected and authenticated");

    // Upsert peer in key store (skip if peer key hex is malformed)
    if let Some(peer_key_bytes) = util::decode_peer_key_logged(&peer_key_hex) {
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
    spawn_receive_loop(app_handle, state, read_half, peer_key_hex, None);
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
    let hole_punch::StrategyResult {
        mut stream,
        role,
        remote_addr,
        strategy_name,
        latency,
    } = hole_punch::ConnectionManager::connect(&peer_addrs, listen_addr)
        .await
        .map_err(|e| format!("connection failed (tried {} candidates): {e}", peer_addrs.len()))?;

    tracing::info!(
        strategy = strategy_name,
        latency = ?latency,
        peer = %remote_addr,
        "connection established via connection manager"
    );

    let identity = state.identity.read().await;
    let kp = identity
        .as_ref()
        .ok_or("identity not initialized")?;

    // Gather our local candidates to share with the peer during handshake.
    let config = state.stun_config.read().await;
    let stun_result = stun::discover_public_addrs(&config).await.ok();
    drop(config);

    let host_candidates = candidate::gather_host_candidates();
    let ipv6_candidates = candidate::gather_ipv6_candidates();
    let reflexive_candidates = stun_result
        .as_ref()
        .map(candidate::gather_reflexive_candidates)
        .unwrap_or_default();

    let mut all = host_candidates;
    all.extend(ipv6_candidates);
    all.extend(reflexive_candidates);
    all.sort_by(|a, b| b.priority.cmp(&a.priority));
    let our_candidates: Vec<WireCandidate> = all.iter().map(|c| WireCandidate {
        address: c.address.clone(),
        candidate_type: c.candidate_type as u8,
        relay_id: None,
    }).collect();

    // Update state with gathered candidates
    {
        let mut cand_state = state.candidates.write().await;
        *cand_state = all;
    }

    let expected_peer_pub = signed.payload.identity_pub;
    let mut session = Session::new();

    // Check if the invite contains an X3DH prekey bundle
    let has_x3dh = signed.payload.x25519_identity_pub != [0u8; 32]
        && signed.payload.signed_prekey != [0u8; 32]
        && !signed.payload.signed_prekey_sig.is_empty();

    if has_x3dh {
        // Verify the signed prekey's Ed25519 signature
        crate::crypto::verify_signature(
            &expected_peer_pub,
            &signed.payload.signed_prekey,
            &signed.payload.signed_prekey_sig,
        ).map_err(|_| "invalid signed prekey signature in invite".to_string())?;
    }

    let x25519 = state.x25519_identity.read().await;
    let x25519_kp = x25519.as_ref();

    match role {
        hole_punch::Role::Initiator => {
            tracing::debug!("hole-punch role: Initiator (outgoing connect won)");
            if has_x3dh {
                let xkp = x25519_kp.ok_or("X25519 key not initialized for X3DH")?;
                let bundle = crate::crypto::PrekeyBundle {
                    identity_key: signed.payload.x25519_identity_pub,
                    signed_prekey: signed.payload.signed_prekey,
                    signed_prekey_sig: signed.payload.signed_prekey_sig.clone(),
                    one_time_prekey: signed.payload.one_time_prekey,
                };
                session
                    .handshake_as_initiator_x3dh(
                        &mut stream, kp, xkp, &expected_peer_pub, &bundle, our_candidates,
                    )
                    .await
                    .map_err(|e| format!("X3DH initiator handshake failed: {e}"))?;
            } else {
                let x25519_pub = x25519_kp.map(|k| k.public_key_bytes()).unwrap_or([0u8; 32]);
                session
                    .handshake_as_initiator(&mut stream, kp, &expected_peer_pub, our_candidates, x25519_pub)
                    .await
                    .map_err(|e| format!("initiator handshake failed: {e}"))?;
            }
        }
        hole_punch::Role::Responder => {
            tracing::debug!("hole-punch role: Responder (incoming accept won)");
            let frame = network::read_frame(&mut stream)
                .await
                .map_err(|e| format!("failed to read initial frame: {e}"))?;

            if frame.packet_type == protocol::PacketType::X3DHHandshakeInit {
                let xkp = x25519_kp.ok_or("X25519 key not initialized for X3DH")?;
                let spk_lock = state.active_signed_prekey.read().await;
                let spk = spk_lock.as_ref()
                    .ok_or("no signed prekey available for X3DH responder handshake")?;
                session
                    .handshake_as_responder_x3dh(
                        &mut stream, kp, xkp, spk, &frame, our_candidates,
                    )
                    .await
                    .map_err(|e| format!("X3DH responder handshake failed: {e}"))?;
            } else if frame.packet_type == protocol::PacketType::HandshakeInit {
                let x25519_pub = x25519_kp.map(|k| k.public_key_bytes()).unwrap_or([0u8; 32]);
                session
                    .handshake_as_responder(&mut stream, kp, &frame, our_candidates, x25519_pub)
                    .await
                    .map_err(|e| format!("responder handshake failed: {e}"))?;
            } else {
                return Err(format!("expected HandshakeInit or X3DHHandshakeInit, got {:?}", frame.packet_type));
            }

            if session.peer_identity_pub != expected_peer_pub {
                return Err("peer identity does not match invite".to_string());
            }
        }
    }

    let peer_fingerprint = session.peer_fingerprint();
    let peer_key_hex = hex::encode(session.peer_identity_pub);

    // Build reconnect info for possible future reconnection
    let reconnect_info = Some(crate::reconnect::ReconnectInfo {
        peer_key_hex: peer_key_hex.clone(),
        peer_fingerprint: peer_fingerprint.clone(),
        strategy_name: strategy_name.to_string(),
        peer_address_hint: remote_addr.to_string(),
        peer_verified: session.peer_verified,
        ratchet_interval: session.ratchet_interval,
    });

    // Split the stream
    let (read_half, write_half) = stream.into_split();

    let conn = PeerConnection {
        write_half,
        session,
        remote_addr,
        strategy_name: strategy_name.to_string(),
    };

    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.clone(), Arc::new(Mutex::new(conn)));
    drop(conns);

    // Start the receive loop for this peer
    spawn_receive_loop(app_handle, state.inner().clone(), read_half, peer_key_hex.clone(), reconnect_info);

    Ok(ConnectionInfo {
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint),
        peer_verified: false,
        peer_key_hex: Some(peer_key_hex),
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

/// Get the actual listening address (after binding to port 0).
#[tauri::command]
pub async fn get_listen_address(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let addr = state.listen_addr.read().await;
    addr.map(|a| a.to_string()).ok_or("not listening".to_string())
}

// ─── Message Receive Loop ───

/// Spawn an async task that reads incoming frames from a peer
/// and emits Tauri events for the React frontend.
pub fn spawn_receive_loop(
    app_handle: AppHandle,
    state: Arc<AppState>,
    mut read_half: tokio::net::tcp::OwnedReadHalf,
    peer_key_hex: String,
    reconnect_info: Option<crate::reconnect::ReconnectInfo>,
) {
    let hb_peer = peer_key_hex.clone();
    let hb_state = state.clone();
    // Spawn a heartbeat worker: sends a Heartbeat every HEARTBEAT_INTERVAL_SECS
    // and expects an ack within HEARTBEAT_TIMEOUT_SECS.
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(crate::protocol::HEARTBEAT_INTERVAL_SECS),
        );
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let conns = hb_state.connections.read().await;
            if let Some(conn_arc) = conns.get(&hb_peer) {
                let mut conn = conn_arc.lock().await;
                match crate::network::send_heartbeat(&mut conn.write_half).await {
                    Ok(_) => tracing::trace!(peer = %hb_peer, "heartbeat sent"),
                    Err(e) => {
                        tracing::info!(peer = %hb_peer, error = %e, "heartbeat failed — disconnecting");
                        break;
                    }
                }
            } else {
                // Connection removed — stop heartbeat
                break;
            }
        }
    });

    tokio::spawn(async move {
        loop {
            // Read a frame from the peer's read half
            let frame = match network::read_frame_from_read_half(&mut read_half).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::info!(peer = %peer_key_hex, error = %e, "peer connection closed");
                    // Store reconnect info for the frontend (if available)
                    if let Some(ri) = reconnect_info.clone() {
                        let mut pr = state.pending_reconnects.write().await;
                        pr.insert(peer_key_hex.clone(), ri);
                    }
                    // Notify frontend about disconnection
                    let was_verified = reconnect_info.as_ref()
                        .map(|ri| ri.peer_verified).unwrap_or(false);
                    let _ = app_handle.emit("m2m://connection", ConnectionEvent {
                        peer_key_hex: peer_key_hex.clone(),
                        state: "disconnected".to_string(),
                        peer_fingerprint: None,
                        peer_verified: was_verified,
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
                                MessageBody::Text { id, content, timestamp, .. } => {
                                    // Use sender's timestamp for consistent ordering.
                                    // Fall back to receiver's clock if timestamp is 0
                                    // (backward compat with older clients that don't send it).
                                    let now = if *timestamp > 0 {
                                        *timestamp
                                    } else {
                                        std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs()
                                    };

                                    // Persist received message
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
                                                            id, &peer_key_hex, "received",
                                                            &encrypted, &nonce, now as i64, true,
                                                        );
                                                    }
                                                    // Drop store lock before PRAGMA optimize to avoid
                                                    // holding RefCell-backed connection across .await
                                                    drop(ms);
                                                    drop(sk);
                                                    // Periodic PRAGMA optimize (at most once per minute)
                                                    let now_ts = Utc::now().timestamp();
                                                    let mut last_opt = state.last_optimize_at.write().await;
                                                    if now_ts - *last_opt > 60 {
                                                        // Re-acquire store lock just for the optimize call
                                                        let ms2 = state.message_store.lock().await;
                                                        if let Some(store2) = ms2.as_ref() {
                                                            let _ = store2.optimize();
                                                        }
                                                        *last_opt = now_ts;
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
                                            read_at: None,
                                            edited_at: None,
                                            deleted: false,
                                            expires_at: None,
                                            reactions: std::collections::HashMap::new(),
                                            sender_peer_key_hex: String::new(),
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
                                            let (temp_file, temp_path) = match util::create_temp_file(total_size) {
                                                Ok((f, p)) => (Some(f), Some(p)),
                                                Err(e) => {
                                                    tracing::warn!(error = %e, "failed to create temp file for transfer");
                                                    (None, None)
                                                }
                                            };

                                            IncomingFileTransfer {
                                                transfer_id: transfer_id.clone(),
                                                peer_key_hex: peer_key_hex.clone(),
                                                filename: safe_name,
                                                total_size,
                                                total_chunks,
                                                file_hash,
                                                chunk_hashes: req.chunk_hashes.clone(),
                                                peer_protocol_version: req.file_transfer_version,
                                                save_path: std::path::PathBuf::new(),
                                                temp_file,
                                                temp_path,
                                                chunks_received: 0,
                                                bytes_received: 0,
                                                chunks_bitmask: vec![false; total_chunks as usize],
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
                                        // Check if we have an outgoing transfer with filepath
                                        let filepath = {
                                            let transfers = state.outgoing_transfers.read().await;
                                            transfers.get(tid).map(|t| t.file_path.to_string_lossy().to_string())
                                        };
                                        if filepath.is_some() {
                                            let tid = tid.to_string();
                                            let state_c = state.clone();
                                            let app_c = app_handle.clone();
                                            let peer_c = peer_key_hex.clone();
                                            drop(conn);
                                            drop(conns);
                                            // Start via queue-aware transfer lifecycle
                                            super::files::try_start_outgoing_transfer(
                                                app_c, state_c, peer_c, tid,
                                            );
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
                PacketType::FileTransferChunkAck => {
                    // Sender side: peer confirmed a chunk was received and verified.
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
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
                                        tracing::trace!(
                                            transfer_id = %ack.transfer_id,
                                            chunk = ack.chunk_index,
                                            acked = t.chunks_acked,
                                            total = t.total_chunks,
                                            "chunk ack received"
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt chunk ack");
                            }
                        }
                    }
                }
                PacketType::FileTransferCancel => {
                    // Either side: peer cancelled an in-progress transfer.
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(cancel) = protocol::deserialize::<protocol::FileTransferCancelData>(&plaintext) {
                                    let tid = cancel.transfer_id;

                                    // Clean up outgoing transfer if this side was sending
                                    {
                                        let mut outgoing = state.outgoing_transfers.write().await;
                                        if let Some(t) = outgoing.get_mut(&tid) {
                                            t.state = crate::state::TransferState::Cancelled;
                                        }
                                        outgoing.remove(&tid);
                                    }

                                    // Clean up incoming transfer if this side was receiving
                                    {
                                        let mut incoming = state.incoming_transfers.write().await;
                                        if let Some(t) = incoming.remove(&tid) {
                                            drop(t.temp_file);
                                            if let Some(ref path) = t.temp_path {
                                                let _ = std::fs::remove_file(path);
                                            }
                                        }
                                    }

                                    // Remove from queue
                                    {
                                        let mut queue = state.transfer_queue.write().await;
                                        queue.queue.retain(|id| id != &tid);
                                        queue.active.remove(&tid);
                                    }

                                    let _ = app_handle.emit("m2m://transfer-cancelled", serde_json::json!({
                                        "transfer_id": tid,
                                    }));

                                    tracing::info!(transfer_id = %tid, "file transfer cancelled by peer");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt file cancel");
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
                    let was_verified = reconnect_info.as_ref()
                        .map(|ri| ri.peer_verified).unwrap_or(false);
                    let _ = app_handle.emit("m2m://connection", ConnectionEvent {
                        peer_key_hex: peer_key_hex.clone(),
                        state: "disconnected".to_string(),
                        peer_fingerprint: None,
                        peer_verified: was_verified,
                    });
                    let mut conns = state.connections.write().await;
                    conns.remove(&peer_key_hex);
                    break;
                }
                PacketType::MessageReaction => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(rxn) = crate::protocol::deserialize::<crate::protocol::MessageReactionData>(&plaintext) {
                                    // Store locally
                                    let ms = state.message_store.lock().await;
                                    if let Some(ref store) = *ms {
                                        let _ = store.upsert_reaction(
                                            &rxn.message_id, &rxn.reaction,
                                            &peer_key_hex, rxn.remove,
                                        );
                                    }
                                    drop(ms);

                                    // Notify frontend
                                    let _ = app_handle.emit("m2m://reaction", serde_json::json!({
                                        "message_id": rxn.message_id,
                                        "reaction": rxn.reaction,
                                        "peer_key_hex": peer_key_hex,
                                        "remove": rxn.remove,
                                    }));
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt message reaction");
                            }
                        }
                    }
                }
                PacketType::MessageEdit => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(edit) = crate::protocol::deserialize::<crate::protocol::MessageEditData>(&plaintext) {
                                    // Encrypt new content first (no lock held yet)
                                    let encrypted_result = {
                                        let sk = state.storage_key.read().await;
                                        sk.as_ref().and_then(|key| {
                                            util::crypto_encrypt_storage(
                                                edit.new_content.as_bytes(), key, util::AAD_MSG_STORE,
                                            ).ok()
                                        })
                                    };
                                    // Then update storage (separate lock scope)
                                    if let Some((nonce, encrypted)) = encrypted_result {
                                        let ms = state.message_store.lock().await;
                                        if let Some(ref store) = *ms {
                                            let _ = store.edit_message(&edit.message_id, &encrypted, &nonce);
                                        }
                                    }

                                    // Notify frontend
                                    let _ = app_handle.emit("m2m://edit", serde_json::json!({
                                        "message_id": edit.message_id,
                                        "new_content": edit.new_content,
                                        "edited_at": edit.edited_at,
                                        "peer_key_hex": peer_key_hex,
                                    }));
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt message edit");
                            }
                        }
                    }
                }
                PacketType::MessageDelete => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(del) = crate::protocol::deserialize::<crate::protocol::MessageDeleteData>(&plaintext) {
                                    // Soft-delete locally
                                    let ms = state.message_store.lock().await;
                                    if let Some(ref store) = *ms {
                                        let _ = store.delete_message(&del.message_id);
                                    }
                                    drop(ms);

                                    // Notify frontend
                                    let _ = app_handle.emit("m2m://delete", serde_json::json!({
                                        "message_id": del.message_id,
                                        "peer_key_hex": peer_key_hex,
                                    }));
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt message delete");
                            }
                        }
                    }
                }
                PacketType::SyncRequest => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(sync) = crate::protocol::deserialize::<crate::protocol::SyncRequestData>(&plaintext) {
                                    // Load sent messages for this peer since the given timestamp,
                                    // decrypt them from storage, and re-send over the session.
                                    let missed: Vec<(String, Option<i64>)> = {
                                        let ms = state.message_store.lock().await;
                                        let sk = state.storage_key.read().await;
                                        if let (Some(ref store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                            if let Ok(stored) = store.load_sent_messages_since(&peer_key_hex, sync.since_timestamp as i64) {
                                                stored.iter().filter_map(|msg| {
                                                    util::crypto_decrypt_storage(
                                                        &msg.content_encrypted, &msg.content_nonce,
                                                        key, util::AAD_MSG_STORE,
                                                    ).ok().and_then(|d| String::from_utf8(d).ok())
                                                     .map(|text| (text, msg.expires_at))
                                                }).collect()
                                            } else {
                                                Vec::new()
                                            }
                                        } else {
                                            Vec::new()
                                        }
                                    };

                                    // Re-send each missed message using the destructure pattern
                                    for (text, expires_at) in &missed {
                                        let PeerConnection { session, write_half, .. } = &mut *conn;
                                        let result = if let Some(secs) = expires_at {
                                            let remaining = *secs - chrono::Utc::now().timestamp();
                                            if remaining > 0 {
                                                session.send_text_with_timer(write_half, text, Some(remaining as u64)).await
                                            } else {
                                                continue;
                                            }
                                        } else {
                                            session.send_text(write_half, text).await
                                        };
                                        if let Err(e) = result {
                                            tracing::warn!(error = %e, "sync: failed to re-send missed message");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt sync request");
                            }
                        }
                    }
                }
                PacketType::SyncDeviceInfo => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(info) = crate::protocol::deserialize::<crate::protocol::SyncDeviceInfo>(&plaintext) {
                                    // Drop conn lock before calling sync handler which may re-acquire it
                                    drop(conn);
                                    let _ = conn_arc;
                                    drop(conns);
                                    let _ = crate::sync::handle_sync_device_info(
                                        &app_handle,
                                        &state,
                                        &peer_key_hex,
                                        &info,
                                    ).await;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt sync device info");
                            }
                        }
                    }
                }
                PacketType::SyncPayload => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(payload) = crate::protocol::deserialize::<crate::protocol::SyncPayload>(&plaintext) {
                                    drop(conn);
                                    let _ = conn_arc;
                                    drop(conns);
                                    crate::sync::handle_sync_payload(
                                        &state,
                                        &peer_key_hex,
                                        &payload,
                                    ).await;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt sync payload");
                            }
                        }
                    }
                }
                PacketType::Error => {
                    tracing::warn!(peer = %peer_key_hex, "peer sent error packet");
                }
                // ─── Group Chat (Phase 3) ───
                PacketType::GroupCreate => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(create) = protocol::deserialize::<protocol::GroupCreateData>(&plaintext) {
                                    tracing::info!(group = %create.group_id, "received group create");
                                    let gid = create.group_id.clone();
                                    drop(conn);
                                    drop(conn_arc);
                                    drop(conns);

                                    state.ensure_message_store(&state.data_dir).await.ok();
                                    let ms = state.message_store.lock().await;
                                    if let Some(store) = ms.as_ref() {
                                        let _ = store.upsert_group(&gid, &create.group_name, create.created_at as i64, "member");
                                        let _ = store.add_group_member(&gid, &create.creator_peer_key_hex, None, "admin", create.created_at as i64);
                                        for key in &create.initial_members {
                                            let _ = store.add_group_member(&gid, key, None, "member", create.created_at as i64);
                                        }
                                    }
                                    drop(ms);

                                    let _ = app_handle.emit("m2m://group-event", GroupEvent {
                                        group_id: gid,
                                        event_type: "created".to_string(),
                                        peer_key_hex: Some(create.creator_peer_key_hex),
                                    });
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt group create"),
                        }
                    }
                }
                PacketType::GroupInvite => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(invite) = protocol::deserialize::<protocol::GroupInviteData>(&plaintext) {
                                    tracing::info!(group = %invite.group_id, "received group invite");
                                    let _ = app_handle.emit("m2m://group-event", GroupEvent {
                                        group_id: invite.group_id,
                                        event_type: "invited".to_string(),
                                        peer_key_hex: Some(invite.inviter_peer_key_hex),
                                    });
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt group invite"),
                        }
                    }
                }
                PacketType::GroupSenderKey => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(sk_data) = protocol::deserialize::<protocol::GroupSenderKeyData>(&plaintext) {
                                    drop(conn);
                                    drop(conn_arc);
                                    drop(conns);
                                    let mut gm = state.group_manager.write().await;
                                    if let Err(e) = gm.handle_sender_key(&sk_data) {
                                        tracing::warn!(error = %e, "failed to handle sender key");
                                    }
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt sender key"),
                        }
                    }
                }
                PacketType::GroupEncryptedMessage => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(group_msg) = protocol::deserialize::<protocol::GroupEncryptedMessageData>(&plaintext) {
                                    let gid = group_msg.group_id.clone();
                                    let sender = group_msg.sender_peer_key_hex.clone();
                                    drop(conn);
                                    drop(conn_arc);
                                    drop(conns);

                                    // Decrypt inner group message
                                    let mut gm = state.group_manager.write().await;
                                    let decrypted = if let Some(group) = gm.get_group_mut(&gid) {
                                        group.decrypt_message(&group_msg).ok()
                                    } else {
                                        None
                                    };
                                    drop(gm);

                                    if let Some(decrypted_content) = decrypted {
                                        let content_str = String::from_utf8_lossy(&decrypted_content).to_string();
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs();
                                        let msg_id = uuid::Uuid::new_v4().to_string();

                                        state.ensure_message_store(&state.data_dir).await.ok();
                                        let sk = state.storage_key.read().await;
                                        let ms = state.message_store.lock().await;
                                        if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                            match super::util::crypto_encrypt_storage(content_str.as_bytes(), key, super::util::AAD_MSG_STORE) {
                                                Ok((nonce, encrypted)) => {
                                                    let _ = store.store_group_message(&msg_id, &gid, &sender, &encrypted, &nonce, now as i64, true);
                                                    let preview = if content_str.len() > 80 { format!("{}...", &content_str[..80]) } else { content_str.clone() };
                                                    let _ = store.update_group_last_message(&gid, now as i64, &preview);
                                                }
                                                Err(e) => tracing::warn!(error = %e, "failed to encrypt group message for storage"),
                                            }
                                        }
                                        drop(ms);
                                        drop(sk);

                                        let _ = app_handle.emit("m2m://group-message", GroupMessageEvent {
                                            group_id: gid,
                                            message: ChatMessage {
                                                id: msg_id,
                                                content: content_str,
                                                direction: "received".to_string(),
                                                timestamp: now,
                                                read_at: None,
                                                edited_at: None,
                                                deleted: false,
                                                expires_at: None,
                                                reactions: std::collections::HashMap::new(),
                                                sender_peer_key_hex: sender,
                                            },
                                        });
                                    }
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt group message"),
                        }
                    }
                }
                PacketType::GroupInfo => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(info) = protocol::deserialize::<protocol::GroupInfoData>(&plaintext) {
                                    let new_name = info.new_name.clone();
                                    let gid = info.group_id.clone();
                                    drop(conn);
                                    drop(conn_arc);
                                    drop(conns);

                                    if let Some(ref name) = new_name {
                                        let mut gm = state.group_manager.write().await;
                                        let _ = gm.update_group_name(&gid, name);
                                        state.ensure_message_store(&state.data_dir).await.ok();
                                        let ms = state.message_store.lock().await;
                                        if let Some(store) = ms.as_ref() {
                                            let _ = store.update_group_name(&gid, name);
                                        }
                                    }

                                    let _ = app_handle.emit("m2m://group-event", GroupEvent {
                                        group_id: gid,
                                        event_type: "name_changed".to_string(),
                                        peer_key_hex: Some(info.changed_by_peer_key_hex),
                                    });
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt group info"),
                        }
                    }
                }
                PacketType::GroupRemove => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(remove) = protocol::deserialize::<protocol::GroupRemoveData>(&plaintext) {
                                    let removed = remove.removed_peer_key_hex.clone();
                                    let gid = remove.group_id.clone();
                                    let is_us = removed == peer_key_hex;
                                    drop(conn);
                                    drop(conn_arc);
                                    drop(conns);

                                    if is_us {
                                        let mut gm = state.group_manager.write().await;
                                        gm.remove_group(&gid);
                                        state.ensure_message_store(&state.data_dir).await.ok();
                                        let ms = state.message_store.lock().await;
                                        if let Some(store) = ms.as_ref() {
                                            let _ = store.remove_group(&gid);
                                        }
                                    } else {
                                        let mut gm = state.group_manager.write().await;
                                        let _ = gm.leave_group(&gid, &removed);
                                        state.ensure_message_store(&state.data_dir).await.ok();
                                        let ms = state.message_store.lock().await;
                                        if let Some(store) = ms.as_ref() {
                                            let _ = store.remove_group_member(&gid, &removed);
                                        }
                                        if let Some(sk_data) = remove.new_sender_key {
                                            let _ = gm.handle_sender_key(&sk_data);
                                        }
                                    }

                                    let _ = app_handle.emit("m2m://group-event", GroupEvent {
                                        group_id: gid,
                                        event_type: "member_removed".to_string(),
                                        peer_key_hex: Some(removed),
                                    });
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt group remove"),
                        }
                    }
                }
                PacketType::GroupLeave => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(&frame) {
                            Ok(plaintext) => {
                                if let Ok(leave) = protocol::deserialize::<protocol::GroupLeaveData>(&plaintext) {
                                    let leaving = leave.leaving_peer_key_hex.clone();
                                    let gid = leave.group_id.clone();
                                    drop(conn);
                                    drop(conn_arc);
                                    drop(conns);

                                    let mut gm = state.group_manager.write().await;
                                    let _ = gm.leave_group(&gid, &leaving);
                                    state.ensure_message_store(&state.data_dir).await.ok();
                                    let ms = state.message_store.lock().await;
                                    if let Some(store) = ms.as_ref() {
                                        let _ = store.remove_group_member(&gid, &leaving);
                                    }

                                    let _ = app_handle.emit("m2m://group-event", GroupEvent {
                                        group_id: gid,
                                        event_type: "member_left".to_string(),
                                        peer_key_hex: Some(leaving),
                                    });
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "failed to decrypt group leave"),
                        }
                    }
                }
                _ => {
                    tracing::warn!(peer = %peer_key_hex, "received unexpected packet type in receive loop");
                }
            }
        }
    });
}
