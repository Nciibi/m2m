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

/// Contact allowlist decision for incoming connections (H5).
///
/// Pure function so the policy is unit-testable. `require_known_contact`
/// comes from the user's security config; when set, only peers that are
/// already known (previously connected → `peers` table) or family members
/// may establish a session. When unset, everyone passes (first-time
/// invite connections must work out of the box).
fn contact_gate_allows(require_known_contact: bool, is_family: bool, is_known_peer: bool) -> bool {
    !require_known_contact || is_family || is_known_peer
}

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
    // Air-gap mode: invite creation performs STUN/UPnP/relay registration —
    // all internet-facing. LAN invites are still possible via manual
    // address exchange, so this is a hard block rather than silent degrade.
    state.ensure_not_air_gapped().await?;
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

    // Generate a one-time prekey for this invite (H6). Its public key goes
    // into the invite's prekey bundle so initiators include DH4 = DH(EK_A,
    // OPK_B) in the X3DH shared secret; the secret key is kept here for the
    // responder. Rotated together with the signed prekey on every new
    // invite — replacing the slot drops (zeroizes) the previous key pair.
    let opk = crate::crypto::EphemeralKeypair::generate();
    let opk_pub = opk.public_key_bytes();
    {
        let mut active_opk = state.active_one_time_prekey.write().await;
        *active_opk = Some(opk);
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
            one_time_prekey: Some(opk_pub),
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

        // Use CACHED candidates for the handshake response. Running a full
        // STUN discovery here would let any unauthenticated host force us
        // into expensive outbound work just by opening a connection (DoS
        // amplification). The cache is populated at listener startup /
        // settings refresh; if it's empty we schedule an authenticated
        // post-handshake refresh below.
        let wire_candidates: Vec<WireCandidate> = {
            let cached = state.candidates.read().await;
            cached.iter().map(|c| WireCandidate {
                address: c.address.clone(),
                candidate_type: c.candidate_type as u8,
                relay_id: None,
            }).collect()
        };

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
            let opk_lock = state.active_one_time_prekey.read().await;
            if let Err(e) = session.handshake_as_responder_x3dh(
                &mut stream, kp, x25519_kp, spk, opk_lock.as_ref(), &frame, wire_candidates,
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

    // ── Contact allowlist gate (H5) ──
    // Runs AFTER the handshake (peer_identity_pub is now signature-authenticated)
    // and BEFORE any persistence: a stranger must not be upserted into the
    // key store merely by connecting, and must not reach the message
    // dispatcher when the allowlist is enabled.
    {
        let require_known = state.security_config.read().await.require_known_contact;
        if require_known {
            let peer_key_bytes = match util::decode_peer_key(&peer_key_hex) {
                Ok(k) => k,
                Err(_) => {
                    tracing::warn!(peer = %peer_key_hex, "gate check skipped: malformed peer key");
                    let _ = network::send_error(
                        &mut stream,
                        protocol::ErrorCode::HandshakeFailed,
                        "connection rejected",
                    ).await;
                    return;
                }
            };
            // Key-store lock scoped narrowly; no .await while held.
            let (is_family, is_known) = {
                let ks = state.key_store.lock().await;
                match ks.as_ref() {
                    Some(store) => (
                        store.is_family_member(&peer_key_bytes).unwrap_or(false),
                        store.is_known_peer(&peer_key_bytes).unwrap_or(false),
                    ),
                    None => (false, false),
                }
            };
            if !contact_gate_allows(require_known, is_family, is_known) {
                tracing::warn!(
                    peer = %peer_key_hex,
                    fingerprint = %peer_fingerprint,
                    "incoming connection rejected: unknown contact (allowlist enabled)"
                );
                let _ = network::send_error(
                    &mut stream,
                    protocol::ErrorCode::HandshakeFailed,
                    "unknown contact — connection rejected",
                ).await;
                return;
            }
        }
    }

    // Split the stream for the receive loop
    let (read_half, write_half) = stream.into_split();

    let conn = PeerConnection {
        write_half,
        session,
        remote_addr: peer_addr,
        strategy_name: "incoming".to_string(),
        last_hb_sent: None,
        last_hb_ack: None,
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

    // Post-authentication candidate refresh: only when the cached set is
    // empty (see pre-handshake comment) AND air-gap mode allows STUN.
    if state.candidates.read().await.is_empty()
        && !state.security_config.read().await.air_gap_mode
    {
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = st.refresh_stun().await {
                tracing::debug!(error = %e, "post-handshake STUN refresh failed");
            }
        });
    }

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

    // Relay auth token (if a relay with authentication is configured) —
    // required by hardened relay servers on CONNECT.
    let relay_auth_token = state
        .relay_config
        .read()
        .await
        .as_ref()
        .map(|c| c.auth_token.clone())
        .unwrap_or_default();

    // ── TCP Hole Punch: race accept vs connect simultaneously ──
    // Both peers race listener.accept() against connect(peer_candidates).
    // Whichever succeeds first determines our handshake role.
    let hole_punch::StrategyResult {
        mut stream,
        role,
        remote_addr,
        strategy_name,
        latency,
    } = hole_punch::ConnectionManager::connect(&peer_addrs, listen_addr, &relay_auth_token)
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
                let opk_lock = state.active_one_time_prekey.read().await;
                session
                    .handshake_as_responder_x3dh(
                        &mut stream, kp, xkp, spk, opk_lock.as_ref(), &frame, our_candidates,
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
        last_hb_sent: None,
        last_hb_ack: None,
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
/// Rotate OUR OWN sending chain for `group_id` (forward secrecy after a
/// member was removed or left) and announce the new signed bundle to all
/// remaining members with active connections.
pub(crate) async fn rotate_and_announce(
    state: Arc<AppState>,
    group_id: &str,
    our_peer_key_hex: &str,
) -> Result<(), String> {
    {
        let mut gm = state.group_manager.write().await;
        let group = gm.get_group_mut(group_id).ok_or("group not found")?;
        group.rotate_own_sender_key()?;
    }

    let roster: Vec<String> = {
        let gm = state.group_manager.read().await;
        let group = gm.get_group(group_id).ok_or("group not found")?;
        group.members.iter()
            .map(|m| m.peer_key_hex.clone())
            .collect()
    };

    fan_out_own_bundle(state, group_id, &roster, our_peer_key_hex).await
}

/// Send our own signed sender-key bundle for `group_id` to every roster
/// member with an active connection, excluding ourselves (H2 fan-out).
pub(crate) async fn fan_out_own_bundle(
    state: Arc<AppState>,
    group_id: &str,
    roster: &[String],
    our_peer_key_hex: &str,
) -> Result<(), String> {
    for peer in roster {
        if peer == our_peer_key_hex {
            continue;
        }
        if let Err(e) = send_own_bundle(state.clone(), group_id, peer, our_peer_key_hex).await {
            tracing::debug!(peer = %peer, error = %e, "sender key fan-out skipped (peer offline?)");
        }
    }
    Ok(())
}

/// Build, sign, and send OUR OWN sender-key bundle for `group_id` to
/// `target_peer` over its pairwise session (H2 trust model v2).
pub(crate) async fn send_own_bundle(    state: Arc<AppState>,
    group_id: &str,
    target_peer: &str,
    our_peer_key_hex: &str,
) -> Result<(), String> {
    let mut bundle = {
        let gm = state.group_manager.read().await;
        let group = gm.get_group(group_id).ok_or("group not found")?;
        group.own_sender_bundle()?
    };
    {
        let id = state.identity.read().await;
        let identity = id.as_ref().ok_or("identity not initialized")?;
        super::groups::finalize_bundle(identity, our_peer_key_hex, &mut bundle);
    }

    let serialized = protocol::serialize(&bundle)
        .map_err(|e| format!("serialize sender key: {e}"))?;

    let conns = state.connections.read().await;
    let conn_arc = conns.get(target_peer)
        .ok_or_else(|| "no connection to peer".to_string())?
        .clone();
    drop(conns);
    let mut conn = conn_arc.lock().await;
    let crate::state::PeerConnection { session, write_half, .. } = &mut *conn;
    session.send_encrypted_typed(write_half, PacketType::GroupSenderKey, &serialized)
        .await
        .map_err(|e| format!("send sender key failed: {e}"))
}

/// Packet handler extracted from spawn_receive_loop (receive-loop split).
#[allow(clippy::single_match)] // uniform handler signature across packet domains
async fn handle_incoming_text(
    state: &Arc<AppState>,
    app_handle: &AppHandle,
    peer_key_hex: &str,
    frame: &crate::network::RawFrame,
) {
    // Owned copy: handlers were extracted verbatim and rely on String semantics.
    let peer_key_hex = peer_key_hex.to_string();
    match frame.packet_type {
                PacketType::EncryptedMessage => {
                    // Decrypt under the per-peer connection lock ONLY; both
                    // guards are released before the SQLite writes below so
                    // slow disk I/O cannot head-of-line block concurrent
                    // sends to this peer (audit secondary fix).
                    let decrypted = {
                        let conns = state.connections.read().await;
                        match conns.get(&peer_key_hex) {
                            Some(conn_arc) => {
                                let mut conn = conn_arc.lock().await;
                                Some(conn.session.decrypt_message(frame))
                            }
                            None => None,
                        }
                    };
                    match decrypted {
                        Some(Ok(body)) => match &body {
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
                                    // Ephemeral mode: nothing touches SQLite.
                                    let history = *state.history_enabled.read().await
                                        && !state.security_config.read().await.ephemeral_mode;
                                    if history {
                                        let sk = state.storage_key.read().await;
                                        let ms = state.message_store.lock().await;
                                        if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                            if let Some(peer_bytes) = util::decode_peer_key_logged(&peer_key_hex) {
                                                let _ = store.ensure_conversation(&peer_key_hex, &peer_bytes);
                                                if let Err(e) = store.store_message_secure(
                                                    id, &peer_key_hex, "received",
                                                    content.as_bytes(), now as i64, None, true, key,
                                                ) {
                                                    tracing::error!(error = %e, "failed to persist received message");
                                                }
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
                                    }

                                    let _ = app_handle.emit("m2m://message", MessageEvent {
                                        peer_key_hex: peer_key_hex.clone(),
                                        message: ChatMessage::new(
                                            id.clone(), content.clone(),
                                            "received".to_string(), now,
                                        ),
                                    });
                                }
                                MessageBody::Ack { id } => {
                                    tracing::debug!(msg_id = %id, "received ack");
                                }
                            },
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "failed to decrypt message");
                        }
                        None => {}
                    }
                }
        _ => {}
    }
}

/// Packet handler extracted from spawn_receive_loop (receive-loop split).
async fn handle_file_transfer_packet(
    state: &Arc<AppState>,
    app_handle: &AppHandle,
    peer_key_hex: &str,
    frame: &crate::network::RawFrame,
) {
    // Owned copy: handlers were extracted verbatim and rely on String semantics.
    let peer_key_hex = peer_key_hex.to_string();
    match frame.packet_type {
                PacketType::FileTransferRequest => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(req) = protocol::deserialize::<FileTransferRequestData>(&plaintext) {
                                    let total_chunks = req.total_chunks;
                                    let total_size = req.total_size;
                                    let transfer_id = req.transfer_id.clone();
                                    let filename = req.filename.clone();
                                    let file_hash = req.file_hash.clone();

                                    // Validate peer-declared parameters BEFORE any allocation.
                                    // total_size/total_chunks are attacker-controlled; without
                                    // this check a single 64 KiB frame could force a ~4 GiB
                                    // bitmask allocation and an unbounded sparse temp file.
                                    match protocol::validate_transfer_request(total_size, total_chunks) {
                                        Err(reason) => {
                                            tracing::warn!(
                                                transfer_id = %transfer_id,
                                                peer = %peer_key_hex,
                                                total_size,
                                                total_chunks,
                                                reason,
                                                "rejected file transfer request"
                                            );
                                        }
                                        Ok(chunk_stride) => {

                                    // Sanitize the filename from the peer (path traversal protection).
                                    let safe_name = network::sanitize_filename(&filename)
                                        .unwrap_or_else(|| format!("file_{}", transfer_id));

                                    let accepted;
                                    {
                                        const MAX_PENDING_INCOMING_TRANSFERS: usize = 20;
                                        const STALE_TRANSFER_SECS: u64 = 60 * 60;

                                        let mut transfers = state.incoming_transfers.write().await;
                                        // Bound concurrent pending transfers: first prune
                                        // stale entries, then reject when still at capacity.
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs();
                                        if transfers.len() >= MAX_PENDING_INCOMING_TRANSFERS {
                                            transfers.retain(|_, t| now.saturating_sub(t.created_at) < STALE_TRANSFER_SECS);
                                        }
                                        accepted = transfers.len() < MAX_PENDING_INCOMING_TRANSFERS;
                                        if accepted {
                                        transfers.entry(transfer_id.clone()).or_insert_with(|| {
                                            let (temp_file, temp_path) = match util::create_temp_file() {
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
                                                chunk_stride,
                                                chunks_bitmask: vec![false; total_chunks as usize],
                                                state: crate::state::TransferState::Pending,
                                                created_at: std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_secs(),
                                                error: None,
                                            }
                                        });
                                        } else {
                                            tracing::warn!(
                                                peer = %peer_key_hex,
                                                transfer_id = %transfer_id,
                                                "too many concurrent incoming transfers — rejecting"
                                            );
                                        }
                                    }
                                    if accepted {
                                        let _ = app_handle.emit("m2m://file-request", FileRequestEvent {
                                            peer_key_hex: peer_key_hex.clone(),
                                            transfer_id,
                                            filename,
                                            total_size,
                                        });
                                    }
                                        }
                                    }
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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(chunk) = protocol::deserialize::<protocol::FileTransferChunkData>(&plaintext) {
                                    let mut transfers = state.incoming_transfers.write().await;
                                    if let Some(transfer) = transfers.get_mut(&chunk.transfer_id) {
                                        // Transfer must belong to THIS peer and be actively
                                        // accepted (Transferring) — prevents cross-peer chunk
                                        // injection into another conversation's transfer and
                                        // writing into paused/unaccepted/cancelled temp files.
                                        if transfer.peer_key_hex != peer_key_hex {
                                            tracing::warn!(
                                                                transfer_id = %chunk.transfer_id,
                                                                "file chunk from wrong peer — ignoring"
                                                            );
                                        } else if transfer.state != crate::state::TransferState::Transferring {
                                            tracing::debug!(
                                                                transfer_id = %chunk.transfer_id,
                                                                state = ?transfer.state,
                                                                "file chunk for non-active transfer — ignoring"
                                                            );
                                        }
                                        // Bounds-check the peer-controlled index and payload size
                                        // against validated transfer parameters BEFORE any seek or
                                        // write: prevents writes past declared EOF (H3).
                                        else {
                                        let idx = chunk.chunk_index as usize;
                                        if idx >= transfer.total_chunks as usize {
                                            tracing::warn!(
                                                chunk = chunk.chunk_index,
                                                total = transfer.total_chunks,
                                                "file chunk index out of range — skipping"
                                            );
                                        } else if (chunk.data.len() as u64) > transfer.chunk_stride {
                                            tracing::warn!(
                                                chunk = chunk.chunk_index,
                                                len = chunk.data.len(),
                                                stride = transfer.chunk_stride,
                                                "file chunk larger than declared stride — skipping"
                                            );
                                        } else if transfer.chunks_bitmask[idx] {
                                            tracing::trace!(chunk = chunk.chunk_index, "duplicate file chunk — ignoring");
                                        } else {
                                        // Verify chunk hash before writing to disk
                                        let hash: [u8; 32] = {
                                                        use sha2::Digest;
                                                        sha2::Sha256::digest(&chunk.data).into()
                                                    };
                                        let hash_valid = hash.to_vec() == chunk.chunk_hash;

                                        if !hash_valid {
                                            tracing::warn!(chunk = chunk.chunk_index, "file chunk hash mismatch — skipping");
                                        } else if let Some(ref mut file) = transfer.temp_file {
                                            use std::io::{Seek, Write};
                                            let offset = (idx as u64) * transfer.chunk_stride;
                                            match file.seek(std::io::SeekFrom::Start(offset)) {
                                                Ok(_) => {
                                                    match file.write_all(&chunk.data) {
                                                        Ok(_) => {
                                                            transfer.chunks_received += 1;
                                                            transfer.bytes_received += chunk.data.len() as u64;
                                                            transfer.chunks_bitmask[idx] = true;
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
                        match conn.session.decrypt_typed_frame(frame) {
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
                                                // Stream-hash the temp file in fixed-size chunks —
                                                // never buffer the whole file in RAM (peer could
                                                // have declared up to MAX_FILE_SIZE).
                                                let mut hasher = sha2::Sha256::new();
                                                let mut buf = vec![0u8; crate::protocol::MAX_FILE_CHUNK_SIZE];
                                                let mut hashed_len: u64 = 0;
                                                let mut read_ok = true;
                                                if file.seek(std::io::SeekFrom::Start(0)).is_err() {
                                                    read_ok = false;
                                                }
                                                while read_ok {
                                                    match file.read(&mut buf) {
                                                        Ok(0) => break,
                                                        Ok(n) => {
                                                            hasher.update(&buf[..n]);
                                                            hashed_len += n as u64;
                                                        }
                                                        Err(e) => {
                                                            tracing::warn!(error = %e, "failed to read temp file for hash verification");
                                                            read_ok = false;
                                                        }
                                                    }
                                                }
                                                read_ok
                                                    && hashed_len == transfer.total_size
                                                    && let digest: [u8; 32] = hasher.finalize().into();
                                                            digest.to_vec() == transfer.file_hash
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
                        match conn.session.decrypt_typed_frame(frame) {
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
                        if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
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
                        match conn.session.decrypt_typed_frame(frame) {
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
                        match conn.session.decrypt_typed_frame(frame) {
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
        _ => {}
    }
}

/// Packet handler extracted from spawn_receive_loop (receive-loop split).
async fn handle_heartbeat_frame(
    state: &Arc<AppState>,
    _app_handle: &AppHandle,
    peer_key_hex: &str,
    frame: &crate::network::RawFrame,
) {
    // Owned copy: handlers were extracted verbatim and rely on String semantics.
    let peer_key_hex = peer_key_hex.to_string();
    match frame.packet_type {
                PacketType::Heartbeat => {
                    // Encrypted heartbeat: decrypt first (forged/garbage
                    // frames are dropped, never acked), then answer with an
                    // encrypted ack while still holding the connection lock.
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(_) => {
                                let crate::state::PeerConnection { session, write_half, .. } = &mut *conn;
                                if let Err(e) = session.send_heartbeat_ack(write_half).await {
                                    tracing::warn!(peer = %peer_key_hex, error = %e, "failed to send heartbeat ack");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "failed to decrypt heartbeat");
                            }
                        }
                    }
                }
                PacketType::HeartbeatAck => {
                    // Peer answered our probe — but only a DECRYPTABLE ack
                    // counts as liveness (plaintext/injected acks must not
                    // defeat the timeout).
                    let decrypted = {
                        let conns = state.connections.read().await;
                        match conns.get(&peer_key_hex) {
                            Some(conn_arc) => {
                                let mut conn = conn_arc.lock().await;
                                Some(conn.session.decrypt_typed_frame(frame))
                            }
                            None => None,
                        }
                    };
                    match decrypted {
                        Some(Ok(_)) => {
                            let conns = state.connections.read().await;
                            if let Some(conn_arc) = conns.get(&peer_key_hex) {
                                let mut conn = conn_arc.lock().await;
                                conn.last_hb_ack = Some(std::time::Instant::now());
                            }
                        }
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "failed to decrypt heartbeat ack");
                        }
                        None => {}
                    }
                }
        _ => {}
    }
}

/// Packet handler extracted from spawn_receive_loop (receive-loop split).
#[allow(clippy::single_match)] // uniform handler signature across packet domains
async fn handle_conversation_meta(
    state: &Arc<AppState>,
    app_handle: &AppHandle,
    peer_key_hex: &str,
    frame: &crate::network::RawFrame,
) {
    // Owned copy: handlers were extracted verbatim and rely on String semantics.
    let peer_key_hex = peer_key_hex.to_string();
    match frame.packet_type {
                PacketType::ConversationMeta => {
                    // Decrypt under the per-peer lock only; SQLite writes run
                    // after both guards are released (head-of-line blocking fix).
                    let decrypted = {
                        let conns = state.connections.read().await;
                        match conns.get(&peer_key_hex) {
                            Some(conn_arc) => {
                                let mut conn = conn_arc.lock().await;
                                Some(conn.session.decrypt_typed_frame(frame))
                            }
                            None => None,
                        }
                    };
                    match decrypted {
                        Some(Ok(plaintext)) => {
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
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "failed to decrypt conversation meta");
                        }
                        None => {}
                    }
                }
        _ => {}
    }
}

/// Packet handler extracted from spawn_receive_loop (receive-loop split).
async fn handle_message_update_frame(
    state: &Arc<AppState>,
    app_handle: &AppHandle,
    peer_key_hex: &str,
    frame: &crate::network::RawFrame,
) {
    // Owned copy: handlers were extracted verbatim and rely on String semantics.
    let peer_key_hex = peer_key_hex.to_string();
    match frame.packet_type {
                PacketType::MessageReaction => {
                    // Decrypt under the per-peer lock only (head-of-line fix).
                    let decrypted = {
                        let conns = state.connections.read().await;
                        match conns.get(&peer_key_hex) {
                            Some(conn_arc) => {
                                let mut conn = conn_arc.lock().await;
                                Some(conn.session.decrypt_typed_frame(frame))
                            }
                            None => None,
                        }
                    };
                    match decrypted {
                        Some(Ok(plaintext)) => {
                            if let Ok(rxn) = crate::protocol::deserialize::<crate::protocol::MessageReactionData>(&plaintext) {
                                // Mirror the send-side cap on receive (H4).
                                if rxn.reaction.chars().count() > 10 {
                                    tracing::warn!(peer = %peer_key_hex, "rejected oversized reaction");
                                    return;
                                }
                                // Store locally, scoped to the sender's conversation (H4):
                                // reactions to messages in other conversations are dropped.
                                // Ephemeral mode: accept for the live UI without SQLite.
                                let ephemeral = state.security_config.read().await.ephemeral_mode;
                                let mut accepted = ephemeral;
                                if !ephemeral {
                                    let sk = state.storage_key.read().await;
                                    let ms = state.message_store.lock().await;
                                    if let Some(ref store) = *ms {
                                        match store.upsert_reaction(
                                            &rxn.message_id, &rxn.reaction,
                                            &peer_key_hex, rxn.remove, &peer_key_hex,
                                            sk.as_ref(),
                                        ) {
                                            Ok(true) => accepted = true,
                                            Ok(false) => {
                                                tracing::warn!(
                                                    peer = %peer_key_hex,
                                                    "reaction for message outside sender conversation — rejected"
                                                );
                                            }
                                            Err(e) => {
                                                tracing::warn!(error = %e, "failed to store reaction");
                                            }
                                        }
                                    }
                                }
                                if !accepted {
                                    return;
                                }

                                // Notify frontend
                                let _ = app_handle.emit("m2m://reaction", serde_json::json!({
                                    "message_id": rxn.message_id,
                                    "reaction": rxn.reaction,
                                    "peer_key_hex": peer_key_hex,
                                    "remove": rxn.remove,
                                }));
                            }
                        }
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "failed to decrypt message reaction");
                        }
                        None => {}
                    }
                }
                PacketType::MessageEdit => {
                    // Decrypt under the per-peer lock only (head-of-line fix).
                    let decrypted = {
                        let conns = state.connections.read().await;
                        match conns.get(&peer_key_hex) {
                            Some(conn_arc) => {
                                let mut conn = conn_arc.lock().await;
                                Some(conn.session.decrypt_typed_frame(frame))
                            }
                            None => None,
                        }
                    };
                    match decrypted {
                        Some(Ok(plaintext)) => {
                            if let Ok(edit) = crate::protocol::deserialize::<crate::protocol::MessageEditData>(&plaintext) {
                                // Mirror the send-side size cap on receive (H4).
                                if edit.new_content.len() > protocol::MAX_TEXT_MESSAGE_SIZE {
                                    tracing::warn!(peer = %peer_key_hex, "rejected oversized message edit");
                                    return;
                                }
                                // Validate + update storage with a fresh per-message
                                // content key (crypto-shredding, H7), scoped to the
                                // sender's conversation and 'received' messages only
                                // (H4) — a peer can never rewrite our own sent messages
                                // or rows in unrelated conversations.
                                // Ephemeral mode: accept the edit for the live UI
                                // without touching SQLite.
                                let mut accepted = false;
                                let ephemeral = state.security_config.read().await.ephemeral_mode;
                                if !ephemeral {
                                    let sk = state.storage_key.read().await;
                                    if let Some(key) = sk.as_ref() {
                                        let ms = state.message_store.lock().await;
                                        if let Some(ref store) = *ms {
                                            match store.edit_message_secure(&edit.message_id, &peer_key_hex, "received", edit.new_content.as_bytes(), key) {
                                                Ok(true) => accepted = true,
                                                Ok(false) => {
                                                    tracing::warn!(
                                                        peer = %peer_key_hex,
                                                        "edit for message outside sender conversation — rejected"
                                                    );
                                                }
                                                Err(e) => {
                                                    tracing::warn!(error = %e, "failed to persist edit");
                                                }
                                            }
                                        }
                                    }
                                }
                                // Ephemeral mode: accept edits without persistence.
                                if !accepted && !ephemeral {
                                    return;
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
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "failed to decrypt message edit");
                        }
                        None => {}
                    }
                }
                PacketType::MessageDelete => {
                    // Decrypt under the per-peer lock only (head-of-line fix).
                    let decrypted = {
                        let conns = state.connections.read().await;
                        match conns.get(&peer_key_hex) {
                            Some(conn_arc) => {
                                let mut conn = conn_arc.lock().await;
                                Some(conn.session.decrypt_typed_frame(frame))
                            }
                            None => None,
                        }
                    };
                    match decrypted {
                        Some(Ok(plaintext)) => {
                            if let Ok(del) = crate::protocol::deserialize::<crate::protocol::MessageDeleteData>(&plaintext) {
                                // Soft-delete locally, scoped to the sender's conversation
                                // and 'received' messages only (H4).
                                // Ephemeral mode: accept for the live UI without SQLite.
                                let ephemeral = state.security_config.read().await.ephemeral_mode;
                                let mut accepted = ephemeral;
                                if !ephemeral {
                                    let ms = state.message_store.lock().await;
                                    if let Some(ref store) = *ms {
                                        match store.delete_message(&del.message_id, &peer_key_hex, "received") {
                                            Ok(true) => accepted = true,
                                            Ok(false) => {
                                                tracing::warn!(
                                                    peer = %peer_key_hex,
                                                    "delete for message outside sender conversation — rejected"
                                                );
                                            }
                                            Err(e) => {
                                                tracing::warn!(error = %e, "failed to persist delete");
                                            }
                                        }
                                    }
                                }
                                if !accepted {
                                    return;
                                }

                                // Notify frontend
                                let _ = app_handle.emit("m2m://delete", serde_json::json!({
                                    "message_id": del.message_id,
                                    "peer_key_hex": peer_key_hex,
                                }));
                            }
                        }
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "failed to decrypt message delete");
                        }
                        None => {}
                    }
                }
        _ => {}
    }
}

/// Packet handler extracted from spawn_receive_loop (receive-loop split).
async fn handle_sync_frame(
    state: &Arc<AppState>,
    app_handle: &AppHandle,
    peer_key_hex: &str,
    frame: &crate::network::RawFrame,
) {
    // Owned copy: handlers were extracted verbatim and rely on String semantics.
    let peer_key_hex = peer_key_hex.to_string();
    match frame.packet_type {
                PacketType::SyncRequest => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(sync) = crate::protocol::deserialize::<crate::protocol::SyncRequestData>(&plaintext) {
                                    // Load sent messages for this peer since the given timestamp,
                                    // decrypt them from storage, and re-send over the session.
                                    let missed: Vec<(String, Option<i64>)> = {
                                        let ms = state.message_store.lock().await;
                                        let sk = state.storage_key.read().await;
                                        if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                            if let Ok(stored) = store.load_sent_messages_since(&peer_key_hex, sync.since_timestamp as i64) {
                                                stored.iter().filter_map(|msg| {
                                                    crate::storage::MessageStore::decrypt_stored_content(
                                                        &msg.content_encrypted, &msg.content_nonce,
                                                        msg.content_key_wrapped.as_deref(),
                                                        key,
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
                                                return;
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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(info) = crate::protocol::deserialize::<crate::protocol::SyncDeviceInfo>(&plaintext) {
                                    // Drop conn lock before calling sync handler which may re-acquire it
                                    drop(conn);
                                    let _ = conn_arc;
                                    drop(conns);
                                    let _ = crate::sync::handle_sync_device_info(
                                        app_handle,
                                        state,
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
                        match conn.session.decrypt_typed_frame(frame) {
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
        _ => {}
    }
}

/// Packet handler extracted from spawn_receive_loop (receive-loop split).
async fn handle_group_frame(
    state: &Arc<AppState>,
    app_handle: &AppHandle,
    peer_key_hex: &str,
    frame: &crate::network::RawFrame,
) {
    // Owned copy: handlers were extracted verbatim and rely on String semantics.
    let peer_key_hex = peer_key_hex.to_string();
    match frame.packet_type {
                // ─── Group Chat (Phase 3) ───
                PacketType::GroupCreate => {
                    let conns = state.connections.read().await;
                    if let Some(conn_arc) = conns.get(&peer_key_hex) {
                        let mut conn = conn_arc.lock().await;
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(create) = protocol::deserialize::<protocol::GroupCreateData>(&plaintext) {
                                    tracing::info!(group = %create.group_id, "received group create");
                                    let gid = create.group_id.clone();
                                    // Roster = creator + initial members (H2: we generate
                                    // our own keys; never trust key material shipped to us).
                                    let mut roster = vec![create.creator_peer_key_hex.clone()];
                                    for m in &create.initial_members {
                                        if !roster.contains(m) {
                                            roster.push(m.clone());
                                        }
                                    }
                                    drop(conn);
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

                                    // Join locally with OUR OWN keys, then announce them.
                                    let joined_bundle = {
                                        let our_peer_key_hex = {
                                            let id = state.identity.read().await;
                                            id.as_ref().map(|kp| hex::encode(kp.public_key_bytes()))
                                        };
                                        match our_peer_key_hex {
                                            Some(our) => {
                                                let mut gm = state.group_manager.write().await;
                                                gm.join_group(
                                                    gid.clone(),
                                                    create.group_name.clone(),
                                                    create.created_at,
                                                    our.clone(),
                                                    false,
                                                    &roster,
                                                ).ok()
                                            }
                                            None => None,
                                        }
                                    };

                                    if joined_bundle.is_some() {
                                        let our_peer_key_hex = {
                                            let id = state.identity.read().await;
                                            id.as_ref().map(|kp| hex::encode(kp.public_key_bytes()))
                                        };
                                        if let Some(our) = our_peer_key_hex {
                                            if let Err(e) = fan_out_own_bundle(state.clone(), &gid, &roster, &our).await {
                                                tracing::warn!(error = %e, group = %gid, "failed to announce own sender key after group create");
                                            }
                                        }
                                    }

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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(invite) = protocol::deserialize::<protocol::GroupInviteData>(&plaintext) {
                                    tracing::info!(group = %invite.group_id, "received group invite");
                                    let gid = invite.group_id.clone();

                                    // Verify the inviter's signature over the roster
                                    // (H2: invites are identity-signed by the inviter).
                                    let inviter_pub_ok = {
                                        let mut sign_data = Vec::new();
                                        sign_data.extend_from_slice(gid.as_bytes());
                                        sign_data.extend_from_slice(&invite.member_count.to_be_bytes());
                                        crate::crypto::verify_signature(
                                            &conn.session.peer_identity_pub,
                                            &sign_data,
                                            &invite.signature,
                                        ).is_ok()
                                    };
                                    drop(conn);
        drop(conns);

                                    if !inviter_pub_ok {
                                        tracing::warn!(group = %gid, peer = %peer_key_hex, "group invite signature invalid — ignoring");
                                        return;
                                    }

                                    // Join locally with OUR OWN keys and announce them to
                                    // the whole roster (mutual exchange with every member).
                                    let our_peer_key_hex = {
                                        let id = state.identity.read().await;
                                        id.as_ref().map(|kp| hex::encode(kp.public_key_bytes()))
                                    };
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                    if let Some(our) = our_peer_key_hex {
                                        let mut roster = invite.existing_members.clone();
                                        if !roster.contains(&invite.inviter_peer_key_hex) {
                                            roster.push(invite.inviter_peer_key_hex.clone());
                                        }
                                        let joined = {
                                            let mut gm = state.group_manager.write().await;
                                            gm.join_group(
                                                gid.clone(),
                                                invite.group_name.clone(),
                                                now,
                                                our.clone(),
                                                false,
                                                &roster,
                                            )
                                        };
                                        match joined {
                                            Ok(_) => {
                                                state.ensure_message_store(&state.data_dir).await.ok();
                                                let ms = state.message_store.lock().await;
                                                if let Some(store) = ms.as_ref() {
                                                    let _ = store.upsert_group(&gid, &invite.group_name, now as i64, "member");
                                                    for key in &roster {
                                                        let _ = store.add_group_member(&gid, key, None, "member", now as i64);
                                                    }
                                                }
                                                drop(ms);

                                                if let Err(e) = fan_out_own_bundle(state.clone(), &gid, &roster, &our).await {
                                                    tracing::warn!(error = %e, group = %gid, "failed to announce own sender key after invite");
                                                }
                                            }
                                            Err(e) => tracing::warn!(error = %e, group = %gid, "failed to join group from invite"),
                                        }
                                    }

                                    let _ = app_handle.emit("m2m://group-event", GroupEvent {
                                        group_id: gid,
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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(sk_data) = protocol::deserialize::<protocol::GroupSenderKeyData>(&plaintext) {
                                    // Capture the transport peer's long-term identity key
                                    // BEFORE releasing the connection: bundle signatures are
                                    // verified against it (H2 trust model v2).
                                    let peer_identity_pub = conn.session.peer_identity_pub;
                                    drop(conn);
        drop(conns);
                                    let our_peer_key_hex = {
                                        let id = state.identity.read().await;
                                        id.as_ref().map(|kp| hex::encode(kp.public_key_bytes()))
                                    };
                                    let receipt = {
                                        let mut gm = state.group_manager.write().await;
                                        our_peer_key_hex.as_ref().and_then(|our| {
                                            gm.handle_sender_key(&sk_data, our, &peer_identity_pub)
                                                .map_err(|e| {
                                                    tracing::warn!(error = %e, peer = %peer_key_hex, "rejected group sender key");
                                                    e
                                                })
                                                .ok()
                                        })
                                    };

                                    // Mutual key exchange: when a NEW member announces itself,
                                    // reply with our own signed bundle so they can decrypt our
                                    // traffic (also how late joiners get existing chain keys).
                                    if receipt == Some(crate::group::SenderKeyReceipt::NewMember) {
                                        if let Some(our) = &our_peer_key_hex {
                                            if let Err(e) = send_own_bundle(state.clone(), &sk_data.group_id, &peer_key_hex, our).await {
                                                tracing::warn!(error = %e, peer = %peer_key_hex, "failed to reply with own sender key");
                                            }
                                        }
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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(group_msg) = protocol::deserialize::<protocol::GroupEncryptedMessageData>(&plaintext) {
                                    let gid = group_msg.group_id.clone();
                                    let sender = group_msg.sender_peer_key_hex.clone();
                                    drop(conn);
drop(conns);

                                    // Decrypt inner group message
                                    let mut gm = state.group_manager.write().await;
                                    let decrypted = if let Some(group) = gm.get_group_mut(&gid) {
                                        match group.decrypt_message(&group_msg) {
                                            Ok(content) => Some(content),
                                            Err(e) => {
                                                tracing::warn!(
                                                    group = %gid,
                                                    sender = %sender,
                                                    error = %e,
                                                    "group message rejected"
                                                );
                                                None
                                            }
                                        }
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

                                        // Ephemeral mode: group content stays in RAM.
                                        if !state.security_config.read().await.ephemeral_mode {
                                            state.ensure_message_store(&state.data_dir).await.ok();
                                            let sk = state.storage_key.read().await;
                                            let ms = state.message_store.lock().await;
                                            if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                                match super::util::crypto_encrypt_storage(content_str.as_bytes(), key, super::util::AAD_MSG_STORE) {
                                                    Ok((nonce, encrypted)) => {
                                                        let _ = store.store_group_message(&msg_id, &gid, &sender, &encrypted, &nonce, now as i64, true);
                                                        let preview = super::util::truncate_utf8(&content_str, 80, "...");
                                                        let _ = store.update_group_last_message(&gid, now as i64, &preview);
                                                    }
                                                    Err(e) => tracing::warn!(error = %e, "failed to encrypt group message for storage"),
                                                }
                                            }
                                            drop(ms);
                                            drop(sk);
                                        }
                                        let _ = app_handle.emit("m2m://group-message", GroupMessageEvent {
                                            group_id: gid,
                                            message: ChatMessage::new(
                                                msg_id, content_str,
                                                "received".to_string(), now,
                                            ).with_sender(sender),
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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(info) = protocol::deserialize::<protocol::GroupInfoData>(&plaintext) {
                                    let new_name = info.new_name.clone();
                                    let gid = info.group_id.clone();
                                    let changer_is_admin;
                                    {
                                        let gm = state.group_manager.read().await;
                                        changer_is_admin = gm.get_group(&gid)
                                            .map(|g| g.is_member(&peer_key_hex) && g.is_admin(&info.changed_by_peer_key_hex)
                                                && info.changed_by_peer_key_hex == peer_key_hex)
                                            .unwrap_or(false);
                                    }
                                    drop(conn);
        drop(conns);

                                    // Authorization (H2): renames must come from the claimed
                                    // changer themselves, who must be an admin member of the
                                    // group. Anything else is dropped.
                                    if !changer_is_admin {
                                        tracing::warn!(
                                            peer = %peer_key_hex,
                                            group = %gid,
                                            "unauthorized group rename attempt — ignored"
                                        );
                                        return;
                                    }

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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(remove) = protocol::deserialize::<protocol::GroupRemoveData>(&plaintext) {
                                    let removed = remove.removed_peer_key_hex.clone();
                                    let gid = remove.group_id.clone();
                                    let is_us = removed == peer_key_hex;
                                    // Authorization (H2): the remover must be the transport
                                    // peer itself, a member of the group, and — when removing
                                    // someone else — an admin.
                                    let authorized = {
                                        let gm = state.group_manager.read().await;
                                        gm.get_group(&gid)
                                            .map(|g| {
                                                remove.removed_by_peer_key_hex == peer_key_hex
                                                    && g.is_member(&peer_key_hex)
                                                    && (is_us || g.is_admin(&peer_key_hex))
                                            })
                                            .unwrap_or(false)
                                    };
                                    let peer_identity_pub = conn.session.peer_identity_pub;
                                    drop(conn);
        drop(conns);

                                    if !authorized {
                                        tracing::warn!(
                                            peer = %peer_key_hex,
                                            group = %gid,
                                            "unauthorized group removal claim — ignored"
                                        );
                                        return;
                                    }

                                    if is_us {
                                        let mut gm = state.group_manager.write().await;
                                        gm.remove_group(&gid);
                                        state.ensure_message_store(&state.data_dir).await.ok();
                                        let ms = state.message_store.lock().await;
                                        if let Some(store) = ms.as_ref() {
                                            let _ = store.remove_group(&gid);
                                        }
                                    } else {
                                        let our_peer_key_hex = {
                                            let id = state.identity.read().await;
                                            id.as_ref().map(|kp| hex::encode(kp.public_key_bytes()))
                                        };
                                        let mut gm = state.group_manager.write().await;
                                        let _ = gm.leave_group(&gid, &removed);
                                        state.ensure_message_store(&state.data_dir).await.ok();
                                        let ms = state.message_store.lock().await;
                                        if let Some(store) = ms.as_ref() {
                                            let _ = store.remove_group_member(&gid, &removed);
                                        }

                                        // Install the remover's rotated key if present AND validly
                                        // signed by the remover's identity key (H2).
                                        if let (Some(sk_data), Some(our)) = (&remove.new_sender_key, &our_peer_key_hex) {
                                            match gm.handle_sender_key(sk_data, our, &peer_identity_pub) {
                                                Ok(_) => {}
                                                Err(e) => tracing::warn!(error = %e, "rejected rotated sender key"),
                                            }
                                        }
                                        drop(gm);

                                        // Forward secrecy: the removed member still knows our OLD
                                        // chain key, so rotate OUR OWN sending chain too and
                                        // announce the new one to remaining members.
                                        if let Some(our) = our_peer_key_hex {
                                            rotate_and_announce(state.clone(), &gid, &our).await.ok();
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
                        match conn.session.decrypt_typed_frame(frame) {
                            Ok(plaintext) => {
                                if let Ok(leave) = protocol::deserialize::<protocol::GroupLeaveData>(&plaintext) {
                                    let leaving = leave.leaving_peer_key_hex.clone();
                                    let gid = leave.group_id.clone();

                                    // Authorization (H2): a peer can only announce its OWN
                                    // departure — forged leave claims on behalf of others
                                    // are dropped.
                                    if leaving != peer_key_hex {
                                        tracing::warn!(
                                            peer = %peer_key_hex,
                                            group = %gid,
                                            "forged group leave claim — ignored"
                                        );
                                        return;
                                    }
                                    drop(conn);
        drop(conns);

                                    let our_peer_key_hex = {
                                        let id = state.identity.read().await;
                                        id.as_ref().map(|kp| hex::encode(kp.public_key_bytes()))
                                    };
                                    {
                                        let mut gm = state.group_manager.write().await;
                                        let _ = gm.leave_group(&gid, &leaving);
                                    }
                                    state.ensure_message_store(&state.data_dir).await.ok();
                                    let ms = state.message_store.lock().await;
                                    if let Some(store) = ms.as_ref() {
                                        let _ = store.remove_group_member(&gid, &leaving);
                                    }
                                    drop(ms);

                                    // Forward secrecy: the leaver knew our old chain key —
                                    // rotate our sending chain and announce the new one.
                                    if let Some(our) = our_peer_key_hex {
                                        rotate_and_announce(state.clone(), &gid, &our).await.ok();
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
        _ => {}
    }
}



pub fn spawn_receive_loop(
    app_handle: AppHandle,
    state: Arc<AppState>,
    mut read_half: tokio::net::tcp::OwnedReadHalf,
    peer_key_hex: String,
    reconnect_info: Option<crate::reconnect::ReconnectInfo>,
) {    let hb_peer = peer_key_hex.clone();
    let hb_state = state.clone();
    let hb_app = app_handle.clone();
    let hb_reconnect = reconnect_info.clone();
    // Spawn a heartbeat worker: probes the peer with a Heartbeat every
    // HEARTBEAT_INTERVAL_SECS and requires a HeartbeatAck within
    // HEARTBEAT_TIMEOUT_SECS of each probe; otherwise the connection is
    // torn down as dead (half-open TCP would otherwise linger forever).
    // The worker polls at half the timeout so a dead peer is detected
    // within one timeout window of its missed ack instead of waiting for
    // the next full probe interval.
    tokio::spawn(async move {
        let poll_secs = crate::protocol::HEARTBEAT_TIMEOUT_SECS
            .min(crate::protocol::HEARTBEAT_INTERVAL_SECS)
            / 2;
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(poll_secs.max(1)),
        );
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;

            let mut dead_reason: Option<String> = None;
            {
                let conns = hb_state.connections.read().await;
                let Some(conn_arc) = conns.get(&hb_peer) else {
                    // Connection removed — stop heartbeat
                    break;
                };
                let mut conn = conn_arc.lock().await;

                // Liveness check: was the most recent probe answered in time?
                if let Some(sent_at) = conn.last_hb_sent {
                    let acked =
                        matches!(conn.last_hb_ack, Some(ack_at) if ack_at >= sent_at);
                    if !acked
                        && sent_at.elapsed()
                            >= std::time::Duration::from_secs(
                                crate::protocol::HEARTBEAT_TIMEOUT_SECS,
                            )
                    {
                        dead_reason = Some("heartbeat ack timeout".to_string());
                    }
                }

                // Send the periodic probe (also acts as keep-alive traffic).
                if dead_reason.is_none() {
                    let probe_due = conn
                        .last_hb_sent
                        .is_none_or(|sent_at| {
                            sent_at.elapsed()
                                >= std::time::Duration::from_secs(
                                    crate::protocol::HEARTBEAT_INTERVAL_SECS,
                                )
                        });
                    if probe_due {
                        // Encrypted heartbeat — sent through the session's
                        // AEAD path (plaintext heartbeats were a liveness
                        // oracle and forgeable by any active attacker).
                        let crate::state::PeerConnection { session, write_half, .. } = &mut *conn;
                        match session.send_heartbeat(write_half).await {
                            Ok(_) => {
                                conn.last_hb_sent = Some(std::time::Instant::now());
                                tracing::trace!(peer = %hb_peer, "heartbeat sent");
                            }
                            Err(e) => {
                                dead_reason = Some(format!("heartbeat send failed: {e}"));
                            }
                        }
                    }
                }
            } // guards dropped before teardown writes

            if let Some(reason) = dead_reason {
                tracing::info!(peer = %hb_peer, reason = %reason, "connection dead — cleaning up");
                if let Some(ri) = hb_reconnect.clone() {
                    let mut pr = hb_state.pending_reconnects.write().await;
                    pr.insert(hb_peer.clone(), ri);
                }
                let was_verified = hb_reconnect.as_ref()
                    .map(|ri| ri.peer_verified).unwrap_or(false);
                let _ = hb_app.emit("m2m://connection", ConnectionEvent {
                    peer_key_hex: hb_peer.clone(),
                    state: "disconnected".to_string(),
                    peer_fingerprint: None,
                    peer_verified: was_verified,
                });
                hb_state.connections.write().await.remove(&hb_peer);
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

            // -- Domain dispatch: packet groups are handled in dedicated
            // functions below (receive-loop split). Each returns having
            // fully consumed the frame.
            if matches!(frame.packet_type, PacketType::EncryptedMessage) {
                handle_incoming_text(&state, &app_handle, &peer_key_hex, &frame).await;
                continue;
            }
            if matches!(
                frame.packet_type,
                PacketType::FileTransferRequest
                    | PacketType::FileTransferChunk
                    | PacketType::FileTransferComplete
                    | PacketType::FileTransferAccept
                    | PacketType::FileTransferReject
                    | PacketType::FileTransferChunkAck
                    | PacketType::FileTransferCancel
            ) {
                handle_file_transfer_packet(&state, &app_handle, &peer_key_hex, &frame).await;
                continue;
            }
            if matches!(frame.packet_type, PacketType::Heartbeat | PacketType::HeartbeatAck) {
                handle_heartbeat_frame(&state, &app_handle, &peer_key_hex, &frame).await;
                continue;
            }
            if matches!(frame.packet_type, PacketType::ConversationMeta) {
                handle_conversation_meta(&state, &app_handle, &peer_key_hex, &frame).await;
                continue;
            }
            if matches!(
                frame.packet_type,
                PacketType::MessageReaction | PacketType::MessageEdit | PacketType::MessageDelete
            ) {
                handle_message_update_frame(&state, &app_handle, &peer_key_hex, &frame).await;
                continue;
            }
            if matches!(
                frame.packet_type,
                PacketType::SyncRequest | PacketType::SyncDeviceInfo | PacketType::SyncPayload
            ) {
                handle_sync_frame(&state, &app_handle, &peer_key_hex, &frame).await;
                continue;
            }
            if matches!(
                frame.packet_type,
                PacketType::GroupCreate
                    | PacketType::GroupInvite
                    | PacketType::GroupSenderKey
                    | PacketType::GroupEncryptedMessage
                    | PacketType::GroupInfo
                    | PacketType::GroupRemove
                    | PacketType::GroupLeave
            ) {
                handle_group_frame(&state, &app_handle, &peer_key_hex, &frame).await;
                continue;
            }

            match frame.packet_type {
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
                PacketType::Error => {
                    tracing::warn!(peer = %peer_key_hex, "peer sent error packet");
                }
                PacketType::TypingIndicator => {
                    let _ = app_handle.emit("m2m://typing", serde_json::json!({
                        "peer_key_hex": peer_key_hex,
                        "typing": true,
                    }));
                }
                PacketType::TypingIndicatorClear => {
                    let _ = app_handle.emit("m2m://typing", serde_json::json!({
                        "peer_key_hex": peer_key_hex,
                        "typing": false,
                    }));
                }
                _ => {
                    tracing::warn!(peer = %peer_key_hex, "received unexpected packet type in receive loop");
                }
            }
        }
    });
}

#[cfg(test)]
mod contact_gate_tests {
    use super::contact_gate_allows;

    /// H5: gate disabled (default) — everyone passes, first-time invite
    /// connections keep working.
    #[test]
    fn test_gate_disabled_lets_everyone_through() {
        assert!(contact_gate_allows(false, false, false));
        assert!(contact_gate_allows(false, true, false));
        assert!(contact_gate_allows(false, false, true));
    }

    /// H5: gate enabled — a validly-signed STRANGER must be rejected.
    /// Pre-fix there was no gate at all: any signed identity could open a
    /// session, get persisted into the key store, and deliver messages.
    #[test]
    fn test_gate_enabled_rejects_stranger() {
        assert!(!contact_gate_allows(true, false, false));
    }

    /// H5: gate enabled — known peers and family pass.
    #[test]
    fn test_gate_enabled_accepts_known_and_family() {
        assert!(contact_gate_allows(true, true, false));
        assert!(contact_gate_allows(true, false, true));
        assert!(contact_gate_allows(true, true, true));
    }
}