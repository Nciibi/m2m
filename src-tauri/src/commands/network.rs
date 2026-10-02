//! Network connection commands.
//!
//! Handles invite creation/validation, TCP listening, peer connection
//! (via hole-punch race), connection state management, and the async
//! receive loop that dispatches all inbound packet types.

use crate::error::AppError;
use std::net::SocketAddr;
use std::sync::Arc;

use sha2::Digest;

use chrono::Utc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use crate::candidate;
use crate::crypto;
use crate::crypto::IdentityKeypair;
use crate::hole_punch;
use crate::identity;
use crate::network;
use crate::protocol::{
    self, ConversationMetaData, FileTransferRequestData, MessageBody, PacketType, WireCandidate,
};
use crate::relay;
use crate::session::Session;
use crate::state::{AppState, IncomingFileTransfer, PeerConnection};
use crate::stun;

use super::util;
use super::{
    ChatMessage, ConnectionEvent, ConnectionInfo, FileRequestEvent, GroupEvent, GroupMessageEvent,
    InviteInfo, MessageEvent,
};

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

/// Evaluate the contact allowlist gate for a freshly-handshaked peer.
///
/// Called by **every** inbound connection path — direct TCP *and* the relay —
/// after the handshake has authenticated `peer_identity_pub` and before any
/// persistence or dispatch. It lives here, next to the direct-TCP caller,
/// because the relay path previously omitted it entirely: a user who enabled
/// the documented `require_known_contact` hardening could still be connected
/// to, messaged, and have the stranger written into their key store simply by
/// routing through a relay.
///
/// Returns `Ok(())` when the connection may proceed, or the user-facing
/// rejection reason.
pub(crate) async fn check_contact_gate(
    state: &AppState,
    peer_key_hex: &str,
) -> Result<(), &'static str> {
    let require_known = state.security_config.read().await.require_known_contact;
    if !require_known {
        return Ok(());
    }

    let peer_key_bytes = match util::decode_peer_key(peer_key_hex) {
        Ok(k) => k,
        // A malformed key can never be a known contact; fail closed rather
        // than skipping the check the way the original code did.
        Err(_) => return Err("connection rejected"),
    };

    // Key-store lock scoped narrowly; no .await while held.
    let (is_family, is_known) = {
        let ks = state.key_store.lock().await;
        match ks.as_ref() {
            Some(store) => (
                store.is_family_member(&peer_key_bytes).unwrap_or(false),
                store.is_known_peer(&peer_key_bytes).unwrap_or(false),
            ),
            // No key store means no known contacts. Fail closed when the
            // allowlist is on, or the setting would be silently inert.
            None => (false, false),
        }
    };

    if contact_gate_allows(require_known, is_family, is_known) {
        Ok(())
    } else {
        Err("unknown contact — connection rejected")
    }
}

/// Advance the sender's confirmed-chunk watermark from one `ChunkAck`.
///
/// Returns the new `(last_acked_index, chunks_acked)`, or `None` when the ACK
/// should be ignored.
///
/// The sender transmits chunks strictly in order and waits for each ACK before
/// sending the next, so a correct peer's ACKs are contiguous by construction.
/// Only a contiguous next-chunk ACK is honoured.
///
/// The previous code accepted any `chunk_index >= last_acked_index` and added
/// the whole span, assuming no gaps. That had two consequences:
///
/// * A gap over-counted: ACK 0 then ACK 5 set `chunks_acked = 6` when chunks
///   1-4 were never acknowledged.
/// * Worse, one frame was enough to finish the transfer. `wait_for_ack`
///   treats a confirmed watermark past `chunk_index` as "this chunk is
///   delivered", so a single authenticated peer — the very party being asked
///   to confirm delivery — could send `chunk_index = total_chunks - 1` once and
///   have every subsequent chunk treated as delivered, with the file declared
///   sent without a byte being written.
///
/// Extracted as a pure function so the rule is unit-testable; the previous form
/// was inline in a `connections`-lock-held block, which is not.
fn advance_ack_watermark(last_acked_index: u32, acked_index: u32) -> Option<u32> {
    if acked_index == last_acked_index + 1 {
        Some(acked_index)
    } else {
        None
    }
}

/// Remove `peer_key_hex` from the connection map **only if it is still ours**.
///
/// `connections` is keyed by peer key, not by connection identity, so a bare
/// `remove(&peer_key_hex)` from an old task's teardown path cannot tell its own
/// dead entry from a live replacement that has since taken the same slot. All
/// four *outbound* connect paths (`connect_to_peer`, `attempt_reconnect`,
/// `connect_discovered_peer`, `connect_family_member`) `insert` over an existing
/// entry; the shared inbound path refuses duplicates with a comment explaining
/// exactly this, and the outbound forks never got the same guard.
///
/// The resulting sequence, entirely reachable by ordinary use: Alice is connected
/// to Bob (inbound). She clicks Connect on the chat, or Bob is re-dialled via
/// discovery or family. `connect_to_peer` overwrites the map entry; the *old*
/// receive loop's read then fails, and its teardown removes the NEW connection.
/// Her UI shows `established` and then, within one heartbeat interval, shows
/// `disconnected` with the new session gone.
///
/// `Arc::ptr_eq` is the identity test: only the task that owns the exact
/// `Arc<Mutex<PeerConnection>>` it was spawned for may remove it.
async fn remove_own_connection(
    state: &Arc<AppState>,
    peer_key_hex: &str,
    mine: &Arc<tokio::sync::Mutex<PeerConnection>>,
) -> bool {
    let mut conns = state.connections.write().await;
    match conns.get(peer_key_hex) {
        Some(current) if Arc::ptr_eq(current, mine) => {
            conns.remove(peer_key_hex);
            true
        }
        // Someone else owns the slot now. Leaving it alone is the whole point.
        _ => false,
    }
}

/// Tell the frontend that an incoming transfer failed, so the download row does
/// not sit at its last progress value forever.
///
/// The sender side has always emitted `m2m://transfer-error` (`finish_and_chain`
/// in `files.rs`); the four *receiving* failure branches emitted nothing at all.
/// Mirrors that payload exactly, including the optional machine-readable code.
fn emit_transfer_error(app_handle: &tauri::AppHandle, transfer_id: &str, error: &str) {
    let _ = tauri::Emitter::emit(
        app_handle,
        "m2m://transfer-error",
        serde_json::json!({
            "transfer_id": transfer_id,
            "error": error,
        }),
    );
}

/// Did the initiator's handshake frame claim to have used a one-time prekey?
///
/// Inspects the `used_opk` field of the X3DH `HandshakeInit`. A malformed
/// frame is treated as "not presented": the handshake itself will reject it,
/// and defaulting to the conservative answer means we never burn a prekey on
/// the strength of an unparseable frame.
fn frame_presented_one_time_prekey(frame: &network::RawFrame) -> bool {
    match protocol::deserialize::<protocol::HandshakeInit>(&frame.body) {
        Ok(init) => init.used_opk.is_some(),
        Err(e) => {
            tracing::debug!(error = %e, "could not parse handshake init for OPK check");
            false
        }
    }
}

/// Run an X3DH responder handshake, consuming the one-time prekey if used.
///
/// Both inbound paths (direct TCP accept and the hole-punch responder leg)
/// funnel through here so consume-on-use semantics cannot be applied on one
/// path and forgotten on the other.
///
/// ## Why the prekey is burned
///
/// The one-time prekey was previously a single slot, written once per invite
/// and never rotated on use, so it served *every* inbound session until the
/// user made another invite. That turns it into a long-lived prekey and
/// substantially defeats the forward secrecy X3DH exists to provide: one later
/// compromise of the prekey (or the prekey store) recovers the DH4 input for
/// all sessions established with that invite.
///
/// ## Why a spent prekey is not a hard failure
///
/// The invite embeds the prekey's public key. Once burned, a second peer
/// holding the same invite cannot complete DH4. X3DH is defined to work
/// without an OPK, so that session proceeds with reduced forward secrecy —
/// the correct price for a prekey that was already spent. It is logged rather
/// than accepted silently, because it is a real (if modest) downgrade.
///
/// ## Ordering
///
/// The prekey is retired only after the handshake *succeeds*, so a stream of
/// malformed or failing attempts cannot be used to grief the user by forcing
/// prekey rotation and permanently downgrading every future session.
pub async fn x3dh_responder_handshake_consume_opk<S>(
    state: &AppState,
    session: &mut crate::session::Session,
    stream: &mut S,
    identity: &crate::crypto::IdentityKeypair,
    x25519_identity: &crate::crypto::X25519IdentityKeypair,
    init_frame: &network::RawFrame,
    local_candidates: Vec<protocol::WireCandidate>,
) -> Result<(), AppError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let spk_lock = state.active_signed_prekey.read().await;
    let spk = spk_lock
        .as_ref()
        .ok_or_else(|| "no signed prekey for X3DH handshake".to_string())?;

    let opk_consumed = frame_presented_one_time_prekey(init_frame);

    // ── Reserve the one-time prekey atomically ──
    //
    // The prekey slot is taken under a *write* guard held for the whole
    // handshake, not read-then-later-write. Reading availability under a read
    // guard and retiring under a write guard afterwards left a TOCTOU window:
    // two concurrent inbound handshakes could both observe the prekey as
    // present, both apply DH4 with the same secret, and only then serialise on
    // the write lock — where the loser's `take()` returns `None` and the
    // damage is already done. That defeats the entire purpose of a one-time
    // prekey, which is to guarantee at most one session derives DH4 from it.
    //
    // Serialising inbound X3DH handshakes is the correct trade: they are rare
    // (one per new connection) and already bounded by the per-IP and global
    // connection limiters.
    let mut opk_lock = state.active_one_time_prekey.write().await;
    let opk_available = opk_lock.is_some();
    let use_opk = opk_available && opk_consumed;

    // No warning here for `opk_consumed && !opk_available`: the responder now
    // distinguishes a replayed one-time invite (refuse) from a reusable invite
    // whose prekey was spent by an earlier recipient (proceed without DH4) and
    // logs the appropriate message. Duplicating it here could only be wrong.

    let opk_for_handshake = if use_opk { opk_lock.as_ref() } else { None };
    let result = session
        .handshake_as_responder_x3dh(
            stream,
            identity,
            x25519_identity,
            spk,
            opk_for_handshake,
            init_frame,
            local_candidates,
        )
        .await;

    // Still under the write guard: retire only on success, so a stream of
    // failing attempts cannot be used to grief the user by burning prekeys.
    if result.is_ok() && use_opk {
        // `take()` + drop: EphemeralKeypair's Drop zeroizes the secret.
        drop(opk_lock.take());
        tracing::info!("one-time prekey consumed and retired");
    }
    drop(opk_lock);
    drop(spk_lock);

    result?;
    Ok(())
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
) -> Result<String, AppError> {
    // Air-gap mode: invite creation performs STUN/UPnP/relay registration —
    // all internet-facing. LAN invites are still possible via manual
    // address exchange, so this is a hard block rather than silent degrade.
    state.ensure_not_air_gapped().await?;

    // ─── Snapshot the identity, then release the lock ───
    //
    // `state.identity` is a write-preferring `RwLock`, and everything below
    // wants `identity.write()`: `lock_vault`, `unlock_vault`,
    // `create_vault_account`, `import_identity`, `execute_duress_wipe` and
    // `panic_wipe`. `kp` was last used at the very end of this function, so
    // NLL kept the read guard alive for the whole body — across
    // `add_port_mapping` (PCP 3s → NAT-PMP 3s → SSDP 4s → HTTP 5s → SOAP 5s →
    // GetExternalIPAddress 5s) and `relay::register` (8s connect + 5s frame).
    //
    // Worst case ~30s of sequential timeouts, during which one queued writer
    // also blocks every subsequent reader. A duress wipe or a panic-hotkey wipe
    // is the one operation that must never be delayed, and this is the UI path
    // that can delay it. The same snapshot idiom is used at
    // `complete_inbound_connection`, with the same reasoning.
    let kp = {
        let identity = state.identity.read().await;
        let kp = identity.as_ref().ok_or("identity not initialized")?;
        crate::crypto::IdentityKeypair::from_bytes(&kp.public_key_bytes(), &kp.secret_key_bytes())
            .map_err(|e| AppError::invalid(format!("identity unusable: {e}")))?
    };

    // ─── X3DH Prekey Bundle ───
    let x25519_kp = {
        let x25519 = state.x25519_identity.read().await;
        let kp = x25519.as_ref().ok_or("X25519 identity not initialized")?;
        crate::crypto::X25519IdentityKeypair::from_bytes(
            &kp.public_key_bytes(),
            &kp.secret_key_bytes(),
        )
        .map_err(|e| AppError::invalid(format!("X25519 identity unusable: {e}")))?
    };
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
        .map_err(|e| AppError::invalid(format!("invalid address: {e}")))?;

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
        return Err(AppError::blocked(
            "Tor is enabled for outbound connections but Private Mode is off. \
             This invite would contain your real IP address, and inbound connections \
             would bypass Tor entirely. Enable Private Mode in Settings to generate \
             invites that exclude your public IP.",
        ));
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

        // ═══ NEVER PUBLISH AN UNFILTERED CANDIDATE LIST ═══
        //
        // The `HandshakeInit`/`HandshakeResponse` frames are written before any
        // key exists, so every byte of them is readable by the peer, the Tor
        // exit and every AS in between. An invite is strictly worse: it is a
        // shareable link handed to a third party, and `state.candidates` holds
        // the LAN address, the global IPv6 and the STUN server-reflexive public
        // IP.
        //
        // Every other publication site (`network.rs:690` responder, `:952`
        // `connect_to_peer`, `discovery.rs:290`, `vault.rs:735`) already routed
        // through this filter and this one did not. The `relay_id: None` on
        // every entry above is what makes the point: `filter_advertised_candidates`
        // keeps only relay entries under Tor, so it would have dropped all of
        // them. That is the correct outcome.
        //
        // Note the Tor guard above only refuses when `!private_mode`, so Tor +
        // Private Mode — the explicitly-permitted combination — is exactly the
        // one where this leak reaches the user, while `port_mapping` and relay
        // registration are correctly skipped.
        // The filter keeps only relay entries under Tor, and is a no-op with Tor
        // off. Private Mode additionally withholds the server-reflexive entry
        // even with Tor off: it is the user's public IP, and Private Mode is
        // documented as "the public IP is NOT included".
        //
        // No `return` here: this block is a plain expression, and an early
        // `return` would skip the rest of `create_invite` entirely (including
        // the invite serialisation) and yield the wrong `Result`.
        let filtered = crate::dial::filter_advertised_candidates(all);
        if private_mode {
            filtered
                .into_iter()
                .filter(|c| {
                    c.candidate_type != crate::candidate::CandidateType::ServerReflexive as u8
                })
                .collect()
        } else {
            filtered
        }
    };
    identity::create_invite(
        &kp,
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
    .map_err(|e| AppError::invalid(format!("invite creation failed: {e}")))
}

/// Validate a received invite link.
#[tauri::command]
pub async fn validate_invite(invite_str: String) -> Result<InviteInfo, AppError> {
    let signed = identity::validate_invite(&invite_str)
        .map_err(|e| AppError::invalid(format!("invite validation failed: {e}")))?;

    let fingerprint = crypto::fingerprint_from_public_key(&signed.payload.identity_pub);

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
) -> Result<String, AppError> {
    let addr: SocketAddr = address
        .parse()
        .map_err(|e| AppError::invalid(format!("invalid address: {e}")))?;

    // Use std TcpListener first to set a custom backlog (128 for DoS resilience),
    // then convert to tokio for async usage.
    let std_listener = std::net::TcpListener::bind(addr)
        .map_err(|e| AppError::invalid(format!("failed to bind listener: {e}")))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| AppError::invalid(format!("failed to set non-blocking: {e}")))?;

    let listener = tokio::net::TcpListener::from_std(std_listener)
        .map_err(|e| AppError::invalid(format!("failed to create async listener: {e}")))?;

    let bound_addr = listener
        .local_addr()
        .map_err(|e| AppError::invalid(format!("failed to get local address: {e}")))?;

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

            // Reap expired per-IP windows before checking. Without this the
            // limiter's map grows one entry per source IP forever, since
            // `check()` inserts an entry even for addresses it then rejects.
            // Only re-accepts pay the (tiny) O(entries) cost.
            let reaped = state_clone.connection_limiter.reap();
            if reaped > 0 {
                tracing::debug!(reaped, "reaped expired per-IP rate limit entries");
            }

            let allowed = state_clone.connection_limiter.check(ip);

            if allowed {
                let state_inner = state_clone.clone();
                let app_inner = app_clone.clone();
                tokio::spawn(async move {
                    state_inner.connection_limiter.increment();
                    handle_incoming_connection(app_inner, state_inner.clone(), stream, peer_addr)
                        .await;
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
                )
                .await;
                drop(stream);
            }
        }
    });

    tracing::info!(address = %bound_addr, "started listening");
    Ok(format!("listening on {bound_addr}"))
}

/// Hard ceiling on the number of entries in `state.connections`.
///
/// Every inbound transport runs its connections through
/// `state.connection_limiter` first, and that limiter is keyed on *attempts*:
/// its active counter is incremented when a handshake starts and decremented
/// when the handshake routine returns, not when the socket closes. A peer that
/// keeps completing handshakes therefore walks the counter back down to zero
/// while its established sessions stay in the map — and each entry pins a
/// socket, a `Session` and its ratchet state for as long as the peer keeps the
/// socket open. Nothing bounded the map itself, which is why the cap has to be
/// enforced here, on the size of the thing that is actually being bounded — and
/// being here it also covers the outbound paths, which insert into the same map
/// and were never counted by the limiter at all.
///
/// Mirrors `MAX_TOTAL_CONNECTIONS` in `network.rs`, which is private to that
/// module. If one is changed the other must be: the accept-path limiter refuses
/// once 50 handshakes are in flight, this refuses once 50 sessions are
/// established, and a lower value here is the stricter of the two.
const MAX_ESTABLISHED_CONNECTIONS: usize = 50;

/// Is there room in `state.connections` for one more established session?
///
/// Pure predicate so the cap is unit-testable at its boundary — the rule is
/// "`MAX_ESTABLISHED_CONNECTIONS` entries is full", and off-by-one there is the
/// difference between a bound and no bound at all.
fn connection_map_has_room(current_len: usize) -> bool {
    current_len < MAX_ESTABLISHED_CONNECTIONS
}

/// Complete an inbound connection, given the stream and its already-read
/// handshake-init frame.
///
/// ## Why this exists
///
/// There were five hand-rolled copies of "gather candidates → build a
/// `PeerConnection` → insert into the map → emit → upsert → spawn the receive
/// loop", and they had already diverged in ways that mattered. The clearest
/// example is documented in the body of [`check_contact_gate`]: the allowlist
/// gate was added to the direct-TCP and relay paths *after the fact*, because
/// a security control was bypassable by choosing a different transport, and the
/// same gap reopened on the discovery and family-contact paths.
///
/// Forking a ~150-line connection routine is exactly the shape that produces
/// the next H5. There is now one implementation, and every inbound transport
/// (direct TCP, relay) routes through it, so a control added here cannot be
/// bypassed by adding a transport.
///
/// Both call sites differ only in where the stream came from and whether the
/// first frame has already been consumed — hence the `pre_read` frame.
pub(crate) async fn complete_inbound_connection(
    app_handle: &AppHandle,
    state: &Arc<AppState>,
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    pre_read: Option<network::RawFrame>,
) {
    let frame = match pre_read {
        Some(f) => f,
        None => match network::read_frame(&mut stream).await {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(error = %e, "failed to read initial frame from incoming connection");
                return;
            }
        },
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

    // ── Snapshot the identity, then release the lock ──
    //
    // The handshake is a blocking read of a peer-supplied frame, so holding
    // `state.identity` across it turns an unauthenticated socket into a lever
    // on the vault. `handshake_as_responder_x3dh` waits for a
    // `HandshakeComplete` frame bounded at 256 KiB, and `read_frame_impl` reads
    // three times per frame under a shared total deadline — so a slow peer still
    // costs real wall-clock time here. Everything downstream of this point needs
    // `identity.write()` (lock_vault, unlock_vault, create_vault_account,
    // import_identity), so holding it across the handshake let a remote
    // stranger delay the user locking their vault for as long as it liked.
    //
    // This routine returns `()`, so these cannot use `?`; each failure is
    // logged and drops the connection.
    let identity_kp = {
        let identity = state.identity.read().await;
        let kp = match identity.as_ref() {
            Some(kp) => kp,
            None => {
                tracing::error!("cannot handle connection: no identity");
                return;
            }
        };
        match IdentityKeypair::from_bytes(&kp.public_key_bytes(), &kp.secret_key_bytes()) {
            Ok(kp) => kp,
            Err(e) => {
                tracing::error!(error = %e, "cannot handle connection: unusable identity key");
                return;
            }
        }
    };
    let x25519_kp_owned = {
        let x = state.x25519_identity.read().await;
        match x.as_ref() {
            Some(kp) => {
                match crate::crypto::X25519IdentityKeypair::from_bytes(
                    &kp.public_key_bytes(),
                    &kp.secret_key_bytes(),
                ) {
                    Ok(kp) => Some(kp),
                    Err(e) => {
                        tracing::error!(error = %e, "cannot handle connection: unusable X25519 identity");
                        return;
                    }
                }
            }
            None => None,
        }
    };

    // Use CACHED candidates for the handshake response. Running a full STUN
    // discovery here would let any unauthenticated host force us into
    // expensive outbound work just by opening a connection (DoS
    // amplification) — and, under Tor, would emit a STUN query from the user's
    // real address. The cache is populated at listener startup / settings
    // refresh; if it is empty we schedule an authenticated post-handshake
    // refresh below.
    let wire_candidates: Vec<WireCandidate> = {
        let cached = state.candidates.read().await;
        // Filtered for the same reason as the initiator side: this frame is
        // plaintext, and under Tor these addresses would let the peer bypass
        // the proxy entirely.
        crate::dial::filter_advertised_candidates(
            cached
                .iter()
                .map(|c| WireCandidate {
                    address: c.address.clone(),
                    candidate_type: c.candidate_type as u8,
                    relay_id: None,
                })
                .collect(),
        )
    };

    if is_x3dh {
        let x25519_kp = match x25519_kp_owned.as_ref() {
            Some(kp) => kp,
            None => {
                tracing::error!("no X25519 identity for X3DH handshake");
                return;
            }
        };
        // Consume-on-use one-time prekey; see
        // `x3dh_responder_handshake_consume_opk` for the rationale.
        if let Err(e) = x3dh_responder_handshake_consume_opk(
            state,
            &mut session,
            &mut stream,
            &identity_kp,
            x25519_kp,
            &frame,
            wire_candidates,
        )
        .await
        {
            tracing::warn!(error = %e, "X3DH handshake failed for incoming connection");
            let _ = network::send_error(
                &mut stream,
                protocol::ErrorCode::HandshakeFailed,
                "x3dh handshake failed",
            )
            .await;
            return;
        }
    } else {
        let x25519_pub = x25519_kp_owned
            .as_ref()
            .map(|k| k.public_key_bytes())
            .unwrap_or([0u8; 32]);
        if let Err(e) = session
            .handshake_as_responder(
                &mut stream,
                &identity_kp,
                &frame,
                wire_candidates,
                x25519_pub,
            )
            .await
        {
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

    let peer_key_hex = hex::encode(session.peer_identity_pub);
    let peer_fingerprint = session.peer_fingerprint();

    // ── Contact allowlist gate (H5) ──
    // Runs AFTER the handshake (peer_identity_pub is now signature-authenticated)
    // and BEFORE any persistence: a stranger must not be upserted into the key
    // store merely by connecting, and must not reach the message dispatcher
    // when the allowlist is enabled.
    if let Err(reason) = check_contact_gate(state, &peer_key_hex).await {
        tracing::warn!(
            peer = %peer_key_hex,
            fingerprint = %peer_fingerprint,
            "incoming connection rejected: {reason} (allowlist enabled)"
        );
        let _ =
            network::send_error(&mut stream, protocol::ErrorCode::HandshakeFailed, reason).await;
        return;
    }

    let (read_half, write_half) = stream.into_split();

    let conn = PeerConnection {
        write_half,
        session,
        remote_addr: peer_addr,
        strategy_name: "incoming".to_string(),
        last_hb_sent: None,
        last_hb_ack: None,
    };

    // Do not silently displace an existing session. This map is keyed by the
    // peer's *self-declared* Ed25519 key, and a responder cannot pin an
    // identity without prior contact — so an attacker who announces a known
    // contact's key would otherwise overwrite the legitimate `PeerConnection`
    // and evict the real peer from the UI and dispatcher. Refusing is the safe
    // default.
    {
        let mut conns = state.connections.write().await;
        if conns.contains_key(&peer_key_hex) {
            tracing::warn!(
                peer = %peer_key_hex,
                "refusing incoming connection: a session with this peer key already exists"
            );
            return;
        }
        // Refuse before inserting, and log the refusal with the counts. Without
        // this the only ceiling on `connections` was the accept-path limiter's
        // count of in-flight handshakes, which a peer drives back to zero by
        // finishing each handshake: the relay (a full MITM by design) could
        // bridge unlimited peers, each leaving a permanent entry holding a
        // socket, a `Session` and ratchet state — and all of them arrive from the
        // relay's single address, so the per-IP rotation defence never sees them.
        // The check sits inside the same `write()` guard as the insert, so two
        // peers racing for the last slot cannot both win it.
        //
        // The peer gets a close rather than a `RateLimitExceeded` frame: the
        // stream is already split here, and this matches the duplicate-key
        // refusal above, which is also a silent drop by design.
        if !connection_map_has_room(conns.len()) {
            tracing::warn!(
                peer = %peer_key_hex,
                established = conns.len(),
                cap = MAX_ESTABLISHED_CONNECTIONS,
                "refusing incoming connection: connection map at capacity"
            );
            return;
        }
        conns.insert(peer_key_hex.clone(), Arc::new(Mutex::new(conn)));
    }
    // Re-read the `Arc` we just stored so the teardown paths in the receive loop
    // can identify *this* session. `complete_inbound_connection` refuses
    // duplicates, so this can only ever be its own entry — but the guard costs
    // nothing and keeps the invariant explicit.
    let my_conn = match state.peer_connection(&peer_key_hex).await {
        Some(arc) => arc,
        None => return,
    };

    let _ = app_handle.emit(
        "m2m://connection",
        ConnectionEvent {
            peer_key_hex: peer_key_hex.clone(),
            state: "established".to_string(),
            peer_fingerprint: Some(peer_fingerprint.clone()),
            peer_verified: false, // Incoming connections start unverified
        },
    );

    // Post-authentication candidate refresh.
    //
    // This used to fire whenever the cached candidate set was empty, which
    // made it remotely triggerable: any stranger who completed a handshake
    // (only a self-signed Ed25519 identity is needed, and
    // `require_known_contact` is off by default) could make the victim perform
    // STUN queries — and, before the fix in `query_single_server`, hostname
    // resolutions — from its real address.
    //
    // Two things gate it now. Tor makes it a hard error, because STUN cannot
    // be performed over Tor and asking anyway is the leak. And the cache
    // being empty is a normal state on a fresh install, so rather than firing
    // a refresh we record that candidates are unknown; the listener populates
    // them at startup and the Settings screen refreshes on demand. A remote
    // peer must never be able to cause the host to talk to a third party.
    if state.candidates.read().await.is_empty() && !state.security_config.read().await.air_gap_mode
    {
        if crate::tor::is_enabled() {
            tracing::debug!(
                "candidates unknown and Tor is enabled — skipping the post-handshake \
                 STUN refresh, which would disclose the real address"
            );
        } else {
            tracing::info!(
                "candidates unknown after an inbound handshake — the listener \
                 populates these at startup; use Settings → Run diagnostics to refresh"
            );
        }
    }

    tracing::info!(peer = %peer_key_hex, "peer connected and authenticated");

    // Upsert peer in key store (skip if peer key hex is malformed)
    if let Some(peer_key_bytes) = util::decode_peer_key_logged(&peer_key_hex) {
        let ks = state.key_store.lock().await;
        if let Some(ref store) = *ks {
            let _ = store.upsert_peer(&peer_key_bytes, &peer_fingerprint, None);
        }
    }

    spawn_receive_loop(
        app_handle.clone(),
        state.clone(),
        read_half,
        peer_key_hex,
        my_conn,
        None,
    );
}

/// Handle an incoming direct-TCP connection.
async fn handle_incoming_connection(
    app_handle: AppHandle,
    state: Arc<AppState>,
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
) {
    complete_inbound_connection(&app_handle, &state, stream, peer_addr, None).await;
}

/// Connect to a peer using an invite link.
#[tauri::command]
pub async fn connect_to_peer(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    invite_str: String,
) -> Result<ConnectionInfo, AppError> {
    let signed = identity::validate_invite(&invite_str)
        .map_err(|e| AppError::invalid(format!("invite invalid: {e}")))?;

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
    // `role` is intentionally ignored: the dialer is connect-only, so it can
    // only ever return `Initiator` — see `punch_connect_only`.
    let hole_punch::StrategyResult {
        mut stream,
        remote_addr,
        strategy_name,
        latency,
        role: _,
    } = hole_punch::ConnectionManager::connect(&peer_addrs, listen_addr, &relay_auth_token)
        .await
        .map_err(|e| {
            format!(
                "connection failed (tried {} candidates): {e}",
                peer_addrs.len()
            )
        })?;

    tracing::info!(
        strategy = strategy_name,
        latency = ?latency,
        peer = %remote_addr,
        "connection established via connection manager"
    );

    // Snapshot, then release — same reasoning as `create_invite`. `kp` is used
    // for the handshake at the end of this function, so without the copy NLL
    // keeps `state.identity` read-locked across STUN discovery and the whole
    // X3DH exchange, and `lock_vault` / the duress and panic wipes queue behind
    // a remote peer's handshake.
    let kp = {
        let identity = state.identity.read().await;
        let kp = identity.as_ref().ok_or("identity not initialized")?;
        crate::crypto::IdentityKeypair::from_bytes(&kp.public_key_bytes(), &kp.secret_key_bytes())
            .map_err(|e| format!("identity unusable: {e}"))?
    };

    // Gather our local candidates to share with the peer during handshake.
    let config = state.stun_config.read().await.clone();
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
    all.sort_by_key(|c| std::cmp::Reverse(c.priority));

    // Update state with the full gathered set. This is used for the settings
    // diagnostics display and as the source for the responder-side
    // advertisement below; it is NOT what gets published, because under Tor
    // the published set is filtered.
    {
        let mut cand_state = state.candidates.write().await;
        *cand_state = all.clone();
    }

    let our_candidates = crate::dial::filter_advertised_candidates(
        all.iter()
            .map(|c| WireCandidate {
                address: c.address.clone(),
                candidate_type: c.candidate_type as u8,
                relay_id: None,
            })
            .collect(),
    );

    let expected_peer_pub = signed.payload.identity_pub;
    let mut session = Session::new();

    // A 5.0.0 peer must always X3DH.
    //
    // This used to be a branch: `if has_x3dh { …x3dh… } else { …legacy… }`.
    // The condition reads the *invite*, which is plaintext on the wire before
    // any key exists, so a peer that omitted or zeroed the prekey bundle could
    // choose which handshake a 5.0.0 initiator performed — and the legacy arm
    // (`handshake_as_initiator`) has no one-time prekey and therefore no forward
    // secrecy. That is a downgrade an on-path peer could force, so the initiator
    // now demands X3DH and fails loudly instead.
    let has_x3dh = signed.payload.x25519_identity_pub != [0u8; 32]
        && signed.payload.signed_prekey != [0u8; 32]
        && !signed.payload.signed_prekey_sig.is_empty();

    if !has_x3dh {
        return Err(AppError::invalid(
            "peer invite carries no X3DH prekey bundle; refusing to downgrade to the \
             pre-5.0.0 handshake. Ask the sender to upgrade to M2M 5.0.0 or later.",
        ));
    }

    // Verify the signed prekey's Ed25519 signature.
    crate::crypto::verify_signature(
        &expected_peer_pub,
        &signed.payload.signed_prekey,
        &signed.payload.signed_prekey_sig,
    )
    .map_err(|_| "invalid signed prekey signature in invite".to_string())?;

    // Snapshot the X25519 identity keypair. `lock_vault` and `unlock_vault` both
    // *write* `x25519_identity`, and `tokio`'s `RwLock` is write-preferring, so
    // holding this read guard across the handshake below delays the vault lock
    // and the duress/panic wipes behind a remote peer — the same hole the
    // `identity` snapshot above closes, one lock over.
    //
    // The snapshot is the whole keypair, not just the public half, because X3DH
    // needs the secret for DH4. It was previously two snapshots (public for the
    // legacy handshake, whole keypair for X3DH) taken inside the two arms of an
    // `if has_x3dh { … } else { … }`; the legacy arm is gone, so only one is
    // needed and the public-half copy has no remaining reader.

    // The dialer only ever produces `Role::Initiator` (see below), so this is
    // not a match — there is exactly one handshake to perform here.
    {
        {
            tracing::debug!("hole-punch role: Initiator (outgoing connect won)");
            // Snapshot the whole keypair, not just its public half: the secret is
            // required for DH4 and a public-key-only copy cannot perform it.
            let xkp = {
                let x25519 = state.x25519_identity.read().await;
                let kp = x25519
                    .as_ref()
                    .ok_or("X25519 key not initialized for X3DH")?;
                crate::crypto::X25519IdentityKeypair::from_bytes(
                    &kp.public_key_bytes(),
                    &kp.secret_key_bytes(),
                )
                .map_err(|e| AppError::invalid(format!("X25519 identity unusable: {e}")))?
            };
            let bundle = crate::crypto::PrekeyBundle {
                identity_key: signed.payload.x25519_identity_pub,
                signed_prekey: signed.payload.signed_prekey,
                signed_prekey_sig: signed.payload.signed_prekey_sig.clone(),
                one_time_prekey: signed.payload.one_time_prekey,
            };
            session
                .handshake_as_initiator_x3dh(
                    &mut stream,
                    &kp,
                    &xkp,
                    &expected_peer_pub,
                    &bundle,
                    our_candidates,
                    identity::is_one_time(&signed),
                )
                .await?;
        }
        // `Role::Responder` is intentionally not matched here. The outbound
        // dialer is connect-only — see `punch_connect_only` for why the local
        // accept leg was removed — so it can only ever return `Initiator`. An
        // inbound arrival is served by the listener in `start_listening`,
        // which hands off to `complete_inbound_connection` and does the
        // responder handshake there.
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

    let conn_arc = Arc::new(Mutex::new(conn));
    {
        let mut conns = state.connections.write().await;
        // Deliberate replace, but now identity-tagged: the `Arc` is handed to
        // `spawn_receive_loop`, whose teardown paths remove by `Arc::ptr_eq`
        // rather than by peer key. See `remove_own_connection` for the race
        // this closes — without it, this `insert` drops the live session and the
        // OLD receive loop's teardown then deletes the NEW one.
        conns.insert(peer_key_hex.clone(), conn_arc.clone());
    }

    // Start the receive loop for this peer
    spawn_receive_loop(
        app_handle,
        state.inner().clone(),
        read_half,
        peer_key_hex.clone(),
        conn_arc,
        reconnect_info,
    );

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
) -> Result<ConnectionInfo, AppError> {
    let conn_state = state.connection_state(&peer_key_hex).await;

    let (fingerprint, verified) = match state.peer_connection(&peer_key_hex).await {
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
) -> Result<(), AppError> {
    let conn_arc = state
        .peer_connection(&peer_key_hex)
        .await
        .ok_or("no connection to this peer")?;
    let mut conn = conn_arc.lock().await;
    conn.session.mark_peer_verified();
    Ok(())
}

/// Disconnect from a peer gracefully.
#[tauri::command]
pub async fn disconnect_peer(
    state: State<'_, Arc<AppState>>,
    peer_key_hex: String,
) -> Result<(), AppError> {
    // Remove from the map first, then send — and never hold the map's *write*
    // guard across the socket write.
    //
    // This held `connections.write()` for the whole send, up to the 10 s
    // `NETWORK_TIMEOUT` if the peer was slow. That is the most exclusive lock in
    // the process: every one of the ~65 `peer_connection()` call sites, every
    // new-connection insert and every heartbeat teardown blocked behind it. So
    // one unresponsive peer turned "disconnect this one chat" into a
    // process-wide stall.
    let conn_arc = state.connections.write().await.remove(&peer_key_hex);
    if let Some(conn_arc) = conn_arc {
        let mut conn = conn_arc.lock().await;
        // Send the disconnect ENCRYPTED. A plaintext 0x30 frame is forgeable:
        // 14 bytes injected into an established TCP stream tear the session
        // down, and the relay is a full MITM for relayed connections. The
        // receive loop only honours a disconnect it can decrypt, so sending it
        // in the clear would also be silently ignored.
        let msg = protocol::DisconnectMessage {
            reason: protocol::DisconnectReason::UserInitiated,
        };
        match protocol::serialize(&msg) {
            Ok(body) => {
                // Destructure first so both borrows come from the
                // destructured fields rather than from `conn` itself — the
                // project-wide pattern for `send_encrypted_typed`.
                let PeerConnection {
                    session,
                    write_half,
                    ..
                } = &mut *conn;
                if let Err(e) = session
                    .send_encrypted_typed(write_half, PacketType::Disconnect, &body)
                    .await
                {
                    // A session that cannot encrypt is already broken; fall
                    // back to plaintext so the peer still learns why, and log
                    // it rather than failing the user-facing command.
                    tracing::debug!(error = %e, "encrypted disconnect failed; sending plaintext");
                    let _ = network::send_disconnect(
                        &mut conn.write_half,
                        protocol::DisconnectReason::UserInitiated,
                    )
                    .await;
                }
            }
            Err(e) => tracing::warn!(error = %e, "failed to serialize disconnect"),
        }
    }
    Ok(())
}

/// Get a list of all connected peers.
#[tauri::command]
pub async fn list_peers(state: State<'_, Arc<AppState>>) -> Result<Vec<ConnectionInfo>, AppError> {
    // Snapshot the handles, then release the map guard before locking each
    // peer. Iterating the map in place held the global `connections` read lock
    // across every `conn_arc.lock().await`, so one peer stalled mid-send (up
    // to the 10s NETWORK_TIMEOUT) blocked this listing AND every writer in the
    // process — disconnects, heartbeat teardown, new-connection insert.
    let handles: Vec<(String, Arc<Mutex<crate::state::PeerConnection>>)> = {
        let conns = state.connections.read().await;
        conns.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    };
    let mut peers = Vec::new();

    for (key, conn_arc) in handles {
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
pub async fn get_listen_address(state: State<'_, Arc<AppState>>) -> Result<String, AppError> {
    let addr = state.listen_addr.read().await;
    addr.map(|a| a.to_string())
        .ok_or_else(|| AppError::not_connected("not listening for incoming connections"))
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
) -> Result<(), AppError> {
    {
        let mut gm = state.group_manager.write().await;
        let group = gm.get_group_mut(group_id).ok_or("group not found")?;
        group.rotate_own_sender_key()?;
    }

    let roster: Vec<String> = {
        let gm = state.group_manager.read().await;
        let group = gm.get_group(group_id).ok_or("group not found")?;
        group
            .members
            .iter()
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
) -> Result<(), AppError> {
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
pub(crate) async fn send_own_bundle(
    state: Arc<AppState>,
    group_id: &str,
    target_peer: &str,
    our_peer_key_hex: &str,
) -> Result<(), AppError> {
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
        .map_err(|e| AppError::serialization(format!("serialize sender key: {e}")))?;

    let conn_arc = state
        .peer_connection(target_peer)
        .await
        .ok_or_else(|| "no connection to peer".to_string())?;
    let mut conn = conn_arc.lock().await;
    let crate::state::PeerConnection {
        session,
        write_half,
        ..
    } = &mut *conn;
    session
        .send_encrypted_typed(write_half, PacketType::GroupSenderKey, &serialized)
        .await
        .map_err(|e| AppError::invalid(format!("send sender key failed: {e}")))
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
                match state.peer_connection(&peer_key_hex).await {
                    Some(conn_arc) => {
                        let mut conn = conn_arc.lock().await;
                        Some(conn.session.decrypt_message(frame))
                    }
                    None => None,
                }
            };
            match decrypted {
                Some(Ok(body)) => match &body {
                    MessageBody::Text {
                        id,
                        content,
                        timestamp,
                        ..
                    } => {
                        // Enforce the text cap on the way IN, not just
                        // on send. It was checked in `chat.rs` and on
                        // edit, but an established peer could post
                        // an arbitrarily large body that went straight
                        // to SQLite — bounded only by the frame cap,
                        // which the receive-loop rate limiter then
                        // still allows at `MAX_INBOUND_FRAMES_PER_SEC`.
                        // (This comment used to cite 20, quoting the deleted
                        // `RATE_LIMIT_MSGS_PER_SEC`; the enforced value has been 30.)
                        // Without this, a
                        // peer could fill the victim's disk.
                        if content.len() > crate::protocol::MAX_TEXT_MESSAGE_SIZE {
                            tracing::warn!(
                                peer = %peer_key_hex,
                                len = content.len(),
                                max = crate::protocol::MAX_TEXT_MESSAGE_SIZE,
                                "rejecting oversized inbound text message"
                            );
                            return;
                        }

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
                        //
                        // The security config is read once, here, and released
                        // before any store lock is taken. Reading it again inside
                        // the `message_store` scope would nest `security_config`
                        // under `message_store` — a pair with no documented
                        // acquisition order, which is how the two deadlocks in
                        // this codebase were shaped.
                        let ephemeral_mode = state.security_config.read().await.ephemeral_mode;
                        let storage_cap =
                            state.security_config.read().await.effective_storage_cap();
                        let history = *state.history_enabled.read().await && !ephemeral_mode;
                        if history {
                            let sk = state.storage_key.read().await;
                            let ms = state.message_store.lock().await;
                            if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                // Enforce the storage cap before writing, so a
                                // peer already over the ceiling cannot push the
                                // store further past it.
                                crate::maintenance::enforce_cap(app_handle, store, storage_cap);

                                if let Some(peer_bytes) =
                                    util::decode_peer_key_logged(&peer_key_hex)
                                {
                                    let _ = store.ensure_conversation(&peer_key_hex, &peer_bytes);
                                    if let Err(e) = store.store_message_secure(
                                        id,
                                        &peer_key_hex,
                                        "received",
                                        content.as_bytes(),
                                        now as i64,
                                        None,
                                        true,
                                        key,
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

                        let _ = app_handle.emit(
                            "m2m://message",
                            MessageEvent {
                                peer_key_hex: peer_key_hex.clone(),
                                message: ChatMessage::new(
                                    id.clone(),
                                    content.clone(),
                                    "received".to_string(),
                                    now,
                                ),
                            },
                        );
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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(req) =
                            protocol::deserialize::<FileTransferRequestData>(&plaintext)
                        {
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

                                    let (accepted, inserted);
                                    // Collected here so the deletes can happen
                                    // *after* the map guard is released, on the
                                    // blocking pool. `remove_file` is a blocking
                                    // syscall: doing it inside `retain` would hold
                                    // the global `incoming_transfers` write lock
                                    // across every delete, and doing it inline on
                                    // the runtime would stall a tokio worker.
                                    let mut orphaned: Vec<std::path::PathBuf> = Vec::new();
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
                                            transfers.retain(|_, t| {
                                                let fresh = now.saturating_sub(t.created_at)
                                                    < STALE_TRANSFER_SECS;
                                                if !fresh {
                                                    if let Some(path) = &t.temp_path {
                                                        orphaned.push(path.clone());
                                                    }
                                                }
                                                fresh
                                            });
                                        }
                                        // `inserted` is whether *this* request created the entry.
                                        //
                                        // `accepted` is a capacity check, so a peer re-sending a request
                                        // for an id that already exists passed it without changing anything — and
                                        // the emit below fired anyway, re-prompting the user's modal up to 30×/s
                                        // and never repairing a transfer the user had already accepted (whose
                                        // placeholder has `total_chunks: 0`).
                                        accepted = transfers.len() < MAX_PENDING_INCOMING_TRANSFERS;
                                        inserted = false;
                                        if accepted {
                                            use std::collections::hash_map::Entry;
                                            match transfers.entry(transfer_id.clone()) {
                                                Entry::Occupied(o) => {
                                                    // A re-send for an id we already
                                                    // know. Refresh the state the
                                                    // placeholder path could not know
                                                    // (the accept handler inserts a
                                                    // stub with `total_chunks: 0`),
                                                    // so a request arriving after the
                                                    // user accepted is repaired rather
                                                    // than silently dropped.
                                                    let slot = o.into_mut();
                                                    if slot.total_chunks == 0 {
                                                        slot.total_size = total_size;
                                                        slot.total_chunks = total_chunks;
                                                        slot.file_hash = file_hash;
                                                        slot.chunk_hashes =
                                                            req.chunk_hashes.clone();
                                                        slot.chunk_stride = chunk_stride;
                                                        slot.chunks_bitmask =
                                                            vec![false; total_chunks as usize];
                                                        slot.chunks_received = 0;
                                                        slot.bytes_received = 0;
                                                    }
                                                }
                                                Entry::Vacant(v) => {
                                                    inserted = true;
                                                    let (temp_file, temp_path) =
                                                        match util::create_temp_file() {
                                                            Ok((f, p)) => (Some(f), Some(p)),
                                                            Err(e) => {
                                                                tracing::warn!(
                                                                    error = %e,
                                                                    "failed to create temp file for transfer"
                                                                );
                                                                (None, None)
                                                            }
                                                        };

                                                    v.insert(IncomingFileTransfer {
                                                        transfer_id: transfer_id.clone(),
                                                        peer_key_hex: peer_key_hex.clone(),
                                                        filename: safe_name,
                                                        total_size,
                                                        total_chunks,
                                                        file_hash,
                                                        chunk_hashes: req.chunk_hashes.clone(),
                                                        peer_protocol_version: req
                                                            .file_transfer_version,
                                                        save_path: std::path::PathBuf::new(),
                                                        temp_file,
                                                        temp_path,
                                                        chunks_received: 0,
                                                        bytes_received: 0,
                                                        chunk_stride,
                                                        chunks_bitmask: vec![
                                                            false;
                                                            total_chunks as usize
                                                        ],
                                                        state: crate::state::TransferState::Pending,
                                                        created_at: std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap_or_default()
                                                            .as_secs(),
                                                        error: None,
                                                    });
                                                }
                                            }
                                        } else {
                                            tracing::warn!(
                                                peer = %peer_key_hex,
                                                transfer_id = %transfer_id,
                                                "too many concurrent incoming transfers — rejecting"
                                            );
                                        }
                                    }
                                    // The map guard is released here; delete the
                                    // orphaned temp files off-lock and off the
                                    // runtime. Without this the prune was the only
                                    // teardown path that left its `m2m_<uuid>` file
                                    // on disk, and the orphan count grows without
                                    // bound (send N requests, wait an hour, send one
                                    // more, repeat).
                                    if !orphaned.is_empty() {
                                        let _ = tokio::task::spawn_blocking(move || {
                                            for path in &orphaned {
                                                let _ = std::fs::remove_file(path);
                                            }
                                        })
                                        .await;
                                    }
                                    if accepted && inserted {
                                        // Only prompt for a transfer we actually
                                        // created. Re-emitting on a duplicate
                                        // `transfer_id` re-triggered the user's
                                        // accept/reject modal repeatedly for one
                                        // file.
                                        let _ = app_handle.emit(
                                            "m2m://file-request",
                                            FileRequestEvent {
                                                peer_key_hex: peer_key_hex.clone(),
                                                transfer_id,
                                                filename,
                                                total_size,
                                            },
                                        );
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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(chunk) =
                            protocol::deserialize::<protocol::FileTransferChunkData>(&plaintext)
                        {
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
                                } else if transfer.state
                                    != crate::state::TransferState::Transferring
                                {
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
                                        tracing::trace!(
                                            chunk = chunk.chunk_index,
                                            "duplicate file chunk — ignoring"
                                        );
                                    } else {
                                        // Verify the chunk before writing it.
                                        //
                                        // Two checks, in this order:
                                        //
                                        // 1. Against `transfer.chunk_hashes` —
                                        //    the per-chunk hashes announced in the
                                        //    signed, session-authenticated transfer
                                        //    *request*. This is the only value the
                                        //    receiver holds that the peer did not
                                        //    just supply alongside the bytes.
                                        // 2. Against `chunk.chunk_hash`, which
                                        //    travels with the chunk and is
                                        //    therefore only self-consistency.
                                        //
                                        // The old code did (2) alone, and the
                                        // pre-announced hashes were transmitted
                                        // for every transfer and then dropped —
                                        // defence in depth that cost a round trip
                                        // and was thrown away. A v1 sender sends no
                                        // per-chunk hashes, so (1) is skipped and
                                        // (2) remains the only check available.
                                        let announced = transfer
                                            .chunk_hashes
                                            .get(chunk.chunk_index as usize)
                                            .filter(|h| h.len() == 32);
                                        let hash: [u8; 32] = {
                                            use sha2::Digest;
                                            sha2::Sha256::digest(&chunk.data).into()
                                        };
                                        let self_consistent = hash.to_vec() == chunk.chunk_hash;
                                        let matches_announcement = match announced {
                                            Some(h) => hash.as_slice() == h.as_slice(),
                                            None => true,
                                        };

                                        if !self_consistent || !matches_announcement {
                                            tracing::warn!(
                                                chunk = chunk.chunk_index,
                                                self_consistent,
                                                matches_announcement,
                                                had_announced_hash = announced.is_some(),
                                                "file chunk hash mismatch — skipping"
                                            );
                                        } else if let Some(ref mut file) = transfer.temp_file {
                                            use std::io::{Seek, Write};
                                            let offset = (idx as u64) * transfer.chunk_stride;
                                            match file.seek(std::io::SeekFrom::Start(offset)) {
                                                Ok(_) => match file.write_all(&chunk.data) {
                                                    Ok(_) => {
                                                        transfer.chunks_received += 1;
                                                        transfer.bytes_received +=
                                                            chunk.data.len() as u64;
                                                        transfer.chunks_bitmask[idx] = true;
                                                    }
                                                    Err(e) => {
                                                        tracing::warn!(error = %e, chunk = chunk.chunk_index, "failed to write chunk to temp file");
                                                    }
                                                },
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
            // Four branches below can destroy the transfer (incomplete, hash
            // mismatch, rename failure, cross-device copy failure) and only the
            // success branch emitted an event. The frontend's `m2m://transfer-error`
            // handler and its validator already exist, so this is exactly the
            // "the event and its listener both exist and nothing fires" gap:
            // the user watched a download stall at 40% forever with no error and
            // no way to tell a failure from a hang.
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                // Decrypt, then drop the connection lock immediately. The whole
                // completion path below (SHA-256 over up to `MAX_FILE_SIZE`, a
                // cross-device copy, a rename) used to run while holding this
                // per-peer mutex, so every other send to the same peer blocked
                // behind it — and the file's own doc comment for the *sender*
                // half explains why blocking work must not sit on the runtime.
                let decrypted = {
                    let mut conn = conn_arc.lock().await;
                    conn.session.decrypt_typed_frame(frame)
                };
                match decrypted {
                    Ok(plaintext) => {
                        if let Ok(complete) =
                            protocol::deserialize::<protocol::FileTransferCompleteData>(&plaintext)
                        {
                            // Take the transfer out of the map FIRST, then release the
                            // global `incoming_transfers` write lock. Holding it across
                            // the whole verification below meant that while ONE peer
                            // finished a 2 GiB download, every other peer's chunks
                            // and requests queued behind it, and the per-peer
                            // `conn.lock()` was held for the same span — so this was a
                            // process-wide stall, not a per-transfer one.
                            let finished = {
                                let mut transfers = state.incoming_transfers.write().await;
                                transfers.remove(&complete.transfer_id)
                            };
                            if let Some(mut transfer) = finished {
                                let transfer_id = complete.transfer_id.clone();
                                let all_received = transfer.chunks_received
                                    == transfer.total_chunks
                                    && transfer.chunks_bitmask.iter().all(|&b| b);

                                if !all_received {
                                    tracing::warn!(
                                        received = transfer.chunks_received,
                                        total = transfer.total_chunks,
                                        "file transfer incomplete - missing chunks"
                                    );
                                    // Every other teardown path emits an event;
                                    // this one did not, so the download row sat at
                                    // its last progress value forever with no
                                    // error — indistinguishable from a hang.
                                    emit_transfer_error(
                                        &app_handle,
                                        &complete.transfer_id,
                                        "the transfer ended before every chunk arrived",
                                    );
                                    drop(transfer.temp_file);
                                    if let Some(ref path) = transfer.temp_path {
                                        let _ = std::fs::remove_file(path);
                                    }
                                } else {
                                    // Stream-hash the temp file on the blocking
                                    // pool, taking the handle out of the
                                    // transfer so nothing else can touch it.
                                    //
                                    // This used to run inline in an `async fn`
                                    // while holding the peer's connection lock
                                    // and the global `incoming_transfers` write
                                    // lock. `read` on a slow or full volume
                                    // blocks the OS thread, so a tokio worker
                                    // stops polling every other socket and timer
                                    // on it — and *all* peers' chunk handlers were
                                    // queued behind the global lock anyway. The
                                    // sender half of this same feature already
                                    // documents why this matters.
                                    let handle = transfer.temp_file.take();
                                    let expected_size = transfer.total_size;
                                    let expected_hash = transfer.file_hash.clone();
                                    let (hashed_len, digest) = match handle {
                                        Some(mut file) => {
                                            match tokio::task::spawn_blocking(
                                                move || -> (u64, [u8; 32]) {
                                                    use std::io::{Read, Seek};
                                                    let mut hasher = sha2::Sha256::new();
                                                    let mut buf = vec![
                                                        0u8;
                                                        crate::protocol::MAX_FILE_CHUNK_SIZE
                                                    ];
                                                    let mut hashed_len: u64 = 0;
                                                    if file
                                                        .seek(std::io::SeekFrom::Start(0))
                                                        .is_err()
                                                    {
                                                        return (0, [0u8; 32]);
                                                    }
                                                    loop {
                                                        match file.read(&mut buf) {
                                                            Ok(0) => break,
                                                            Ok(n) => {
                                                                hasher.update(&buf[..n]);
                                                                hashed_len += n as u64;
                                                            }
                                                            Err(e) => {
                                                                tracing::warn!(
                                                                    error = %e,
                                                                    "failed to read temp file for hash verification"
                                                                );
                                                                // `u64::MAX` can never
                                                                // equal `total_size`,
                                                                // so a read error
                                                                // fails verification.
                                                                return (u64::MAX, [0u8; 32]);
                                                            }
                                                        }
                                                    }
                                                    (hashed_len, hasher.finalize().into())
                                                },
                                            )
                                            .await
                                            {
                                                Ok(v) => v,
                                                Err(e) => {
                                                    tracing::warn!(
                                                        error = %e,
                                                        "hash verification task failed"
                                                    );
                                                    (u64::MAX, [0u8; 32])
                                                }
                                            }
                                        }
                                        None => (u64::MAX, [0u8; 32]),
                                    };
                                    let hash_valid = hashed_len == expected_size
                                        && digest.to_vec() == expected_hash;

                                    if hash_valid {
                                        let safe_name =
                                            network::sanitize_filename(&transfer.filename)
                                                .unwrap_or_else(|| {
                                                    format!("download_{}", transfer_id)
                                                });
                                        let final_path =
                                            if transfer.save_path.as_os_str().is_empty() {
                                                std::path::PathBuf::from(&safe_name)
                                            } else if transfer.save_path.is_dir() {
                                                transfer.save_path.join(&safe_name)
                                            } else {
                                                transfer.save_path.clone()
                                            };

                                        // `transfer.temp_file` was already `.take()`n and moved into the
                                        // `spawn_blocking` hash closure above, which has returned by now —
                                        // so the handle is closed, which is what Windows needs before a
                                        // rename. The guard is on the *path* only.
                                        let rename_result = match transfer.temp_path.as_ref() {
                                            Some(temp_path) => {
                                                std::fs::rename(temp_path, &final_path)
                                            }
                                            None => Err(std::io::Error::new(
                                                std::io::ErrorKind::Other,
                                                "transfer was missing its temp path",
                                            )),
                                        };

                                        match rename_result {
                                            Ok(()) => {
                                                let _ = app_handle.emit(
                                                    "m2m://file-complete",
                                                    serde_json::json!({
                                                        "transfer_id": transfer_id,
                                                        "filename": safe_name,
                                                        "path": final_path.to_string_lossy(),
                                                    }),
                                                );
                                            }
                                            // `rename` is not a copy. Across
                                            // filesystems it fails with `EXDEV`
                                            // (Linux) / `ERROR_NOT_SAME_DEVICE`
                                            // (Windows), and `/tmp` is a separate
                                            // mount on most Linux systems while
                                            // `%LOCALAPPDATA%\Temp` is very often
                                            // a different volume from the user's
                                            // Downloads folder.
                                            //
                                            // The old code treated that as a hard
                                            // failure and DELETED the temp file —
                                            // so a fully received, per-chunk
                                            // hash-verified, whole-file-SHA-256
                                            // verified download was destroyed,
                                            // with no event on either the failure
                                            // or the loss. Fall back to a copy.
                                            Err(e)
                                                if e.kind()
                                                    == std::io::ErrorKind::CrossesDevices =>
                                            {
                                                // `spawn_blocking`: the fallback is a
                                                // full-file read/write/sync, and a
                                                // blocking syscall on a tokio worker
                                                // stalls every other socket and
                                                // timer on that thread. The peer lock
                                                // and the global transfer-map lock
                                                // are already released above, so this
                                                // is about the runtime rather than
                                                // about contention.
                                                let temp = transfer.temp_path.clone();
                                                let dest = final_path.clone();
                                                let copied = match temp {
                                                    Some(t) => {
                                                        match tokio::task::spawn_blocking(
                                                            move || {
                                                                util::move_across_filesystems(
                                                                    &t, &dest,
                                                                )
                                                            },
                                                        )
                                                        .await
                                                        {
                                                            Ok(r) => r,
                                                            Err(e) => Err(std::io::Error::new(
                                                                std::io::ErrorKind::Other,
                                                                format!("copy task failed: {e}"),
                                                            )),
                                                        }
                                                    }
                                                    None => Err(std::io::Error::new(
                                                        std::io::ErrorKind::Other,
                                                        "transfer was missing its temp path",
                                                    )),
                                                };
                                                match copied {
                                                    Ok(()) => {
                                                        let _ = app_handle.emit(
                                                            "m2m://file-complete",
                                                            serde_json::json!({
                                                                "transfer_id": transfer_id,
                                                                "filename": safe_name,
                                                                "path": final_path.to_string_lossy(),
                                                            }),
                                                        );
                                                    }
                                                    Err(ce) => {
                                                        tracing::warn!(
                                                            error = %ce,
                                                            "cross-device copy failed - cleaning up"
                                                        );
                                                        emit_transfer_error(
                                                            &app_handle,
                                                            &transfer_id,
                                                            &format!(
                                                                "could not save the file to {}: {ce}",
                                                                final_path.display()
                                                            ),
                                                        );
                                                        if let Some(ref path) = transfer.temp_path {
                                                            let _ = std::fs::remove_file(path);
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    error = %e,
                                                    "failed to rename temp file - cleaning up"
                                                );
                                                emit_transfer_error(
                                                    &app_handle,
                                                    &transfer_id,
                                                    "could not save the received file",
                                                );
                                                if let Some(ref path) = transfer.temp_path {
                                                    let _ = std::fs::remove_file(path);
                                                }
                                            }
                                        }
                                    } else {
                                        tracing::warn!("file hash verification failed - deleting corrupted temp file");
                                        emit_transfer_error(
                                            &app_handle,
                                            &transfer_id,
                                            "the received file failed its integrity check and was discarded",
                                        );
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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        // MUST be `protocol::deserialize` (MessagePack), not
                        // `serde_json`. `send_file_accept` writes via
                        // `protocol::serialize` = `rmp_serde::to_vec`, whose
                        // first byte is a map header, and `serde_json` rejects
                        // anything that is not `{`/`[`/a literal — so this
                        // branch could never succeed, no chunk was ever sent,
                        // and the sender's UI sat in `Pending` forever with no
                        // error. The sibling handlers 40 lines away used the
                        // right deserialiser, which is why two of four were
                        // converted and two were not.
                        match protocol::deserialize::<protocol::FileTransferAcceptData>(&plaintext)
                        {
                            Ok(accept) => {
                                let tid = accept.transfer_id;
                                // The map lookup below is keyed by an
                                // attacker-supplied string, so bound it before
                                // it reaches the map and `save_dir` handling.
                                if !tid.is_empty() && tid.len() <= 64 {
                                    // Check if we have an outgoing transfer with filepath
                                    let filepath = {
                                        let transfers = state.outgoing_transfers.read().await;
                                        transfers
                                            .get(&tid)
                                            .map(|t| t.file_path.to_string_lossy().to_string())
                                    };
                                    if filepath.is_some() {
                                        let state_c = state.clone();
                                        let app_c = app_handle.clone();
                                        let peer_c = peer_key_hex.clone();
                                        drop(conn);
                                        // Start via queue-aware transfer lifecycle
                                        super::files::try_start_outgoing_transfer(
                                            app_c, state_c, peer_c, tid,
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "malformed file-transfer accept");
                            }
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to decrypt file accept"),
                }
            }
        }
        PacketType::FileTransferReject => {
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                if let Ok(plaintext) = conn.session.decrypt_typed_frame(frame) {
                    // Same defect as `FileTransferAccept` above: the wire format
                    // is MessagePack. This branch silently never matched, so a
                    // rejected transfer was never removed from
                    // `outgoing_transfers` and permanently leaked its queue slot.
                    match protocol::deserialize::<protocol::FileTransferRejectData>(&plaintext) {
                        Ok(reject) => {
                            let tid = reject.transfer_id;
                            if !tid.is_empty() && tid.len() <= 64 {
                                state.outgoing_transfers.write().await.remove(&tid);
                                tracing::info!(transfer_id = %tid, "file transfer rejected by peer");
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "malformed file-transfer reject");
                        }
                    }
                }
            }
        }
        PacketType::FileTransferChunkAck => {
            // Sender side: peer confirmed a chunk was received and verified.
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(ack) =
                            protocol::deserialize::<protocol::FileTransferChunkAckData>(&plaintext)
                        {
                            let mut outgoing = state.outgoing_transfers.write().await;
                            if let Some(t) = outgoing.get_mut(&ack.transfer_id) {
                                // Advance only on a *contiguous* next-chunk ACK.
                                //
                                // The previous code accepted any `ack.chunk_index
                                // >= last_acked_index` and added the whole span,
                                // which assumed strictly increasing ACKs with no
                                // gaps. Two consequences:
                                //
                                // 1. A gap over-counted. ACK 0 then ACK 5 set
                                //    `chunks_acked = 6` when chunks 1-4 were
                                //    never acknowledged.
                                // 2. Worse, one frame was enough to finish the
                                //    transfer. `wait_for_ack` treats
                                //    `chunks_acked > chunk_index` as "this chunk
                                //    is confirmed", so a single authenticated
                                //    peer — the very party being asked to
                                //    confirm delivery — could send
                                //    `chunk_index = total_chunks - 1` once and
                                //    have every subsequent chunk treated as
                                //    delivered. The file would be declared sent
                                //    without a byte ever being written.
                                //
                                // The sender transmits chunks strictly in order
                                // and waits for each ACK before sending the next,
                                // so a well-behaved peer's ACKs are contiguous by
                                // construction and this accepts them unchanged. A
                                // gap or a duplicate is now ignored, which is
                                // also what the receiver's `chunks_bitmask`
                                // already did — the sender now holds a
                                // conservative mirror of it instead of a
                                // separately-invented count.
                                if let Some(next) =
                                    advance_ack_watermark(t.last_acked_index, ack.chunk_index)
                                {
                                    t.last_acked_index = next;
                                    t.chunks_acked = next + 1;
                                    t.last_activity_at = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs();
                                } else if ack.chunk_index > t.last_acked_index + 1 {
                                    tracing::warn!(
                                        transfer_id = %ack.transfer_id,
                                        expected = t.last_acked_index + 1,
                                        got = ack.chunk_index,
                                        "non-contiguous chunk ack ignored"
                                    );
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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(cancel) =
                            protocol::deserialize::<protocol::FileTransferCancelData>(&plaintext)
                        {
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

                            let _ = app_handle.emit(
                                "m2m://transfer-cancelled",
                                serde_json::json!({
                                    "transfer_id": tid,
                                }),
                            );

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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(_) => {
                        let crate::state::PeerConnection {
                            session,
                            write_half,
                            ..
                        } = &mut *conn;
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
                match state.peer_connection(&peer_key_hex).await {
                    Some(conn_arc) => {
                        let mut conn = conn_arc.lock().await;
                        Some(conn.session.decrypt_typed_frame(frame))
                    }
                    None => None,
                }
            };
            match decrypted {
                Some(Ok(_)) => {
                    if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
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
                match state.peer_connection(&peer_key_hex).await {
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
                            let _ =
                                store.set_peer_display_name(&peer_key_hex, &meta.my_display_name);
                            // If the peer suggested a name for our side, store it as display_name
                            // (only if we don't already have one)
                            if !meta.your_display_name.is_empty() {
                                if let Ok(Some(conv)) = store.get_conversation(&peer_key_hex) {
                                    if conv.display_name.is_none() {
                                        let _ = store.rename_conversation(
                                            &peer_key_hex,
                                            &meta.your_display_name,
                                        );
                                    }
                                }
                            }
                        }
                        // Notify frontend to refresh conversation list
                        let _ = app_handle.emit(
                            "m2m://conversation-meta",
                            serde_json::json!({
                                "peer_key_hex": peer_key_hex.clone(),
                                "peer_display_name": meta.my_display_name,
                                "suggested_name": meta.your_display_name,
                            }),
                        );
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
                match state.peer_connection(&peer_key_hex).await {
                    Some(conn_arc) => {
                        let mut conn = conn_arc.lock().await;
                        Some(conn.session.decrypt_typed_frame(frame))
                    }
                    None => None,
                }
            };
            match decrypted {
                Some(Ok(plaintext)) => {
                    if let Ok(rxn) = crate::protocol::deserialize::<
                        crate::protocol::MessageReactionData,
                    >(&plaintext)
                    {
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
                                    &rxn.message_id,
                                    &rxn.reaction,
                                    &peer_key_hex,
                                    rxn.remove,
                                    &peer_key_hex,
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
                                // Inbound reactions are a `messages.db` write
                                // path like the outbound ones, so the cap
                                // applies here too — a peer sending repeated
                                // Reaction frames must not be able to grow the
                                // store past its ceiling.
                                crate::maintenance::enforce_cap(
                                    app_handle,
                                    store,
                                    storage_cap,
                                );
                            }
                        }
                        if !accepted {
                            return;
                        }

                        // Notify frontend
                        let _ = app_handle.emit(
                            "m2m://reaction",
                            serde_json::json!({
                                "message_id": rxn.message_id,
                                "reaction": rxn.reaction,
                                "peer_key_hex": peer_key_hex,
                                "remove": rxn.remove,
                            }),
                        );
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
                match state.peer_connection(&peer_key_hex).await {
                    Some(conn_arc) => {
                        let mut conn = conn_arc.lock().await;
                        Some(conn.session.decrypt_typed_frame(frame))
                    }
                    None => None,
                }
            };
            match decrypted {
                Some(Ok(plaintext)) => {
                    if let Ok(edit) =
                        crate::protocol::deserialize::<crate::protocol::MessageEditData>(&plaintext)
                    {
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
                                    match store.edit_message_secure(
                                        &edit.message_id,
                                        &peer_key_hex,
                                        "received",
                                        edit.new_content.as_bytes(),
                                        key,
                                    ) {
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
                        let _ = app_handle.emit(
                            "m2m://edit",
                            serde_json::json!({
                                "message_id": edit.message_id,
                                "new_content": edit.new_content,
                                "edited_at": edit.edited_at,
                                "peer_key_hex": peer_key_hex,
                            }),
                        );
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
                match state.peer_connection(&peer_key_hex).await {
                    Some(conn_arc) => {
                        let mut conn = conn_arc.lock().await;
                        Some(conn.session.decrypt_typed_frame(frame))
                    }
                    None => None,
                }
            };
            match decrypted {
                Some(Ok(plaintext)) => {
                    if let Ok(del) = crate::protocol::deserialize::<
                        crate::protocol::MessageDeleteData,
                    >(&plaintext)
                    {
                        // Soft-delete locally, scoped to the sender's conversation
                        // and 'received' messages only (H4).
                        // Ephemeral mode: accept for the live UI without SQLite.
                        let ephemeral = state.security_config.read().await.ephemeral_mode;
                        let mut accepted = ephemeral;
                        if !ephemeral {
                            let ms = state.message_store.lock().await;
                            if let Some(ref store) = *ms {
                                match store.delete_message(
                                    &del.message_id,
                                    &peer_key_hex,
                                    "received",
                                ) {
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
                        let _ = app_handle.emit(
                            "m2m://delete",
                            serde_json::json!({
                                "message_id": del.message_id,
                                "peer_key_hex": peer_key_hex,
                            }),
                        );
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
/// Maximum age of the sync window a peer may request, in seconds (30 days).
///
/// Bounds how much history a single `SyncRequest` can ask the client to scan,
/// decrypt and re-send. Older than this is clamped rather than refused, so a
/// legitimate device that has been offline for a long time still catches up to
/// the retention window instead of failing outright.
const MAX_SYNC_LOOKBACK_SECS: i64 = 30 * 24 * 60 * 60;

/// Maximum number of messages re-sent in response to one `SyncRequest` (2000).
///
/// The lookback clamp bounds the *time* range; this bounds the *count*, which
/// is what actually bounds the work (a decrypt plus a frame write each).
///
/// A peer that asks for more is **refused**, not truncated. The previous
/// documentation here claimed "the oldest-N within the window rather than
/// nothing, and the truncation is logged", and the handler did exactly that —
/// after decrypting every row in the window, so the cap bounded only what was
/// kept. A control that is documented and not enforced is worse than no
/// control: the code below now refuses, and the doc matches it.
const MAX_SYNC_RESEND_MESSAGES: usize = 2000;

/// Outcome of gathering what one `SyncRequest` should be answered with.
enum SyncResend {
    /// Decrypted bodies, oldest first, ready to write.
    Ready(Vec<(String, Option<i64>)>),
    /// Nothing is being sent, and the `&'static str` is the reason reported to
    /// the peer.
    ///
    /// There is deliberately no "empty" variant: an empty response is
    /// indistinguishable from "you have everything", so it may only be sent
    /// when the query genuinely succeeded and matched no rows. Every other
    /// outcome is a refusal that the peer is told about.
    Refused(&'static str),
}

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
            // Clone the handle and release the connection-map guard BEFORE any
            // I/O. Holding it across this body meant a single authenticated
            // 20-byte `SyncRequest` from a peer that simply stops reading its
            // socket pinned the global `connections` read lock for up to
            // `MAX_SYNC_RESEND_MESSAGES` × `NETWORK_TIMEOUT` (~5.5 hours),
            // blocking every `disconnect_peer`, every heartbeat teardown and
            // every new-connection insert in the process. The count and
            // lookback caps bound the work but never released the lock.
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(sync) = crate::protocol::deserialize::<
                            crate::protocol::SyncRequestData,
                        >(&plaintext)
                        {
                            // ── Bound the requested window ──
                            // `since_timestamp` was fully peer-controlled
                            // with no lower bound (0 was accepted) and no cap
                            // on how many messages came back. A single
                            // 20-byte authenticated packet therefore
                            // triggered a full-table scan, a decrypt pass
                            // over the entire history, and N re-sends —
                            // repeatable at will, since (before the
                            // receive-loop rate limiter) there was no
                            // inbound throttle either.
                            //
                            // A request older than the retention window
                            // is clamped; a request that would return more
                            // than the cap is refused outright so the
                            // attacker cannot even get the partial work.
                            let now = chrono::Utc::now().timestamp();
                            let earliest = now - MAX_SYNC_LOOKBACK_SECS;
                            if (sync.since_timestamp as i64) < earliest {
                                tracing::warn!(
                                    peer = %peer_key_hex,
                                    requested = sync.since_timestamp,
                                    "sync request window too old — clamped"
                                );
                            }
                            let since = (sync.since_timestamp as i64).max(earliest);

                            // ── Bound the count BEFORE any decryption ──
                            //
                            // The cap has to be applied to the *rows*, not to
                            // the decrypted output. It used to be applied after
                            // the decrypt loop, so `load_sent_messages_since`
                            // returned every row in the window and every one of
                            // them was decrypted — a CEK unwrap plus an AEAD open
                            // of up to `MAX_TEXT_MESSAGE_SIZE` — with only the
                            // first 2000 kept. A peer holding enough sent history
                            // to fill the window therefore bought 2000
                            // decryptions of work per 20-byte request, and could
                            // ask again at the frame rate.
                            //
                            // `Refused` is sent rather than a truncated prefix
                            // because the comment above promises exactly that
                            // ("refused outright so the attacker cannot even get
                            // the partial work"). The rows are chronological, so a
                            // truncated prefix is indistinguishable at the
                            // receiving end from a complete catch-up of the oldest
                            // messages.
                            //
                            // Residual (needs `storage.rs`, out of scope here):
                            // the SQL itself is still unbounded, so the rows of an
                            // over-cap window are read before they are counted.
                            // The correct fix is `LIMIT ?3` with
                            // `MAX_SYNC_RESEND_MESSAGES + 1` bound as `?3` in
                            // `load_sent_messages_since`; then this branch stays
                            // exactly as it is and the over-cap case never
                            // allocates the rows.
                            let fetched: SyncResend = {
                                // Lock order: `storage_key` before `message_store`.
                                let sk = state.storage_key.read().await;
                                let ms = state.message_store.lock().await;
                                match (ms.as_ref(), sk.as_ref()) {
                                    (Some(store), Some(key)) => {
                                        match store
                                            .load_sent_messages_since(&peer_key_hex, since)
                                        {
                                            Ok(stored)
                                                if stored.len() > MAX_SYNC_RESEND_MESSAGES =>
                                            {
                                                tracing::warn!(
                                                    peer = %peer_key_hex,
                                                    total = stored.len(),
                                                    cap = MAX_SYNC_RESEND_MESSAGES,
                                                    "sync response refused: window exceeds the \
                                                     per-request cap — peer must resume from \
                                                     a later since_timestamp"
                                                );
                                                SyncResend::Refused(
                                                    "sync window exceeds the per-request cap",
                                                )
                                            }
                                            Ok(stored) => SyncResend::Ready(
                                                stored
                                                    .iter()
                                                    .filter_map(|msg| {
                                                        crate::storage::MessageStore::decrypt_stored_content(
                                                            &msg.content_encrypted,
                                                            &msg.content_nonce,
                                                            msg.content_key_wrapped.as_deref(),
                                                            key,
                                                        )
                                                        .ok()
                                                        .and_then(|d| String::from_utf8(d).ok())
                                                        .map(|text| (text, msg.expires_at))
                                                    })
                                                    .collect(),
                                            ),
                                            // An unreadable store must not be reported as
                                            // an empty window: the peer would take
                                            // "nothing to send" as authoritative and never
                                            // ask again.
                                            Err(e) => {
                                                tracing::warn!(
                                                    peer = %peer_key_hex,
                                                    error = %e,
                                                    "sync: failed to read missed messages — \
                                                     response refused"
                                                );
                                                SyncResend::Refused("message store unavailable")
                                            }
                                        }
                                    }
                                    // No store, or a locked vault with no storage key.
                                    // Nothing can be decrypted, and that is not an
                                    // authoritative empty answer.
                                    _ => SyncResend::Refused("message store unavailable"),
                                }
                            };

                            let missed = match fetched {
                                SyncResend::Ready(missed) => missed,
                                SyncResend::Refused(reason) => {
                                    // Say so. A refusal that looks exactly like
                                    // "nothing missed" is the worst of the two
                                    // options: the requester treats the conversation as
                                    // caught up, never advances its `since_timestamp`,
                                    // and asks the same over-cap question on every
                                    // reconnect — a permanent, invisible hole in the
                                    // history. The requesting side only logs incoming
                                    // `Error` frames today, so this cannot repair that
                                    // yet (it needs `commands/mod.rs` and the frontend),
                                    // but it puts the refusal in the peer's log instead
                                    // of in neither. Plaintext, like every other
                                    // `send_error`: it discloses nothing and does not
                                    // touch the send ratchet.
                                    let PeerConnection { write_half, .. } = &mut *conn;
                                    if let Err(e) = network::send_error(
                                        write_half,
                                        protocol::ErrorCode::RateLimitExceeded,
                                        reason,
                                    )
                                    .await
                                    {
                                        tracing::warn!(
                                            error = %e,
                                            "sync: failed to report refusal to peer"
                                        );
                                    }
                                    return;
                                }
                            };

                            // Re-send each missed message using the destructure pattern
                            for (text, expires_at) in &missed {
                                let PeerConnection {
                                    session,
                                    write_half,
                                    ..
                                } = &mut *conn;
                                let result = if let Some(secs) = expires_at {
                                    let remaining = *secs - chrono::Utc::now().timestamp();
                                    if remaining > 0 {
                                        session
                                            .send_text_with_timer(
                                                write_half,
                                                text,
                                                Some(remaining as u64),
                                            )
                                            .await
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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(info) = crate::protocol::deserialize::<
                            crate::protocol::SyncDeviceInfo,
                        >(&plaintext)
                        {
                            // Drop conn lock before calling sync handler which may re-acquire it
                            drop(conn);
                            let _ = conn_arc;
                            let _ = crate::sync::handle_sync_device_info(
                                app_handle,
                                state,
                                &peer_key_hex,
                                &info,
                            )
                            .await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to decrypt sync device info");
                    }
                }
            }
        }
        PacketType::SyncPayload => {
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(payload) =
                            crate::protocol::deserialize::<crate::protocol::SyncPayload>(&plaintext)
                        {
                            drop(conn);
                            let _ = conn_arc;
                            crate::sync::handle_sync_payload(state, &peer_key_hex, &payload).await;
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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(create) =
                            protocol::deserialize::<protocol::GroupCreateData>(&plaintext)
                        {
                            tracing::info!(group = %create.group_id, "received group create");

                            // ── Authorization ──
                            // `GroupInfo`, `GroupRemove` and `GroupLeave`
                            // all verify the sender's standing before
                            // acting. `GroupCreate` did not, and because
                            // `upsert_group` is an INSERT OR REPLACE, any
                            // peer that knew a `group_id` could overwrite
                            // that group's name, `created_at` and the
                            // victim's own `our_role`, force-join the
                            // victim, and have the victim's signed
                            // sender-key bundle fanned out to an
                            // attacker-chosen roster.
                            //
                            // Two conditions must hold:
                            //  1. The claim about who created the group
                            //     must match the authenticated peer.
                            //  2. The creator must not already exist —
                            //     re-using an id would clobber the
                            //     established roster and roles.
                            let peer_claims_creator = create.creator_peer_key_hex == peer_key_hex;

                            let group_already_exists = {
                                let ms = state.message_store.lock().await;
                                ms.as_ref()
                                    .and_then(|s| s.load_group(&create.group_id).ok())
                                    .is_some()
                            };

                            if !peer_claims_creator {
                                tracing::warn!(
                                    group = %create.group_id,
                                    peer = %peer_key_hex,
                                    claimed_creator = %create.creator_peer_key_hex,
                                    "group create rejected: creator claim does not \
                                     match the authenticated peer"
                                );
                                return;
                            }
                            if group_already_exists {
                                tracing::warn!(
                                    group = %create.group_id,
                                    peer = %peer_key_hex,
                                    "group create rejected: group already exists \
                                     (a re-create would clobber the roster and roles)"
                                );
                                return;
                            }

                            let gid = create.group_id.clone();
                            // Roster = creator + initial members (H2: we generate
                            // our own keys; never trust key material shipped to us).
                            //
                            // Capped before the loop, and the dedup uses a set:
                            // `Vec::contains` inside a `for` over a peer-controlled
                            // list is O(n²), and the list comes from a 512 KiB
                            // frame, so ~260k single-character entries meant ~3×10¹⁰
                            // string comparisons — minutes of CPU, on a runtime
                            // thread, with the peer's connection mutex still held
                            // (`drop(conn)` comes after).
                            let initial = &create.initial_members;
                            if initial.len() > crate::group::MAX_GROUP_MEMBERS - 1 {
                                tracing::warn!(
                                    peers = %peer_key_hex,
                                    members = initial.len(),
                                    "group create rejected: initial_members exceeds the group cap"
                                );
                                return;
                            }
                            let mut seen: std::collections::HashSet<&str> =
                                std::collections::HashSet::with_capacity(initial.len() + 1);
                            let mut roster = vec![create.creator_peer_key_hex.clone()];
                            seen.insert(create.creator_peer_key_hex.as_str());
                            for m in initial {
                                if seen.insert(m.as_str()) {
                                    roster.push(m.clone());
                                }
                            }
                            drop(conn);

                            state.ensure_message_store(&state.data_dir).await.ok();
                            let ms = state.message_store.lock().await;
                            if let Some(store) = ms.as_ref() {
                                let _ = store.upsert_group(
                                    &gid,
                                    &create.group_name,
                                    create.created_at as i64,
                                    "member",
                                );
                                let _ = store.add_group_member(
                                    &gid,
                                    &create.creator_peer_key_hex,
                                    None,
                                    "admin",
                                    create.created_at as i64,
                                );
                                for key in &create.initial_members {
                                    let _ = store.add_group_member(
                                        &gid,
                                        key,
                                        None,
                                        "member",
                                        create.created_at as i64,
                                    );
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
                                        )
                                        .ok()
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
                                    if let Err(e) =
                                        fan_out_own_bundle(state.clone(), &gid, &roster, &our).await
                                    {
                                        tracing::warn!(error = %e, group = %gid, "failed to announce own sender key after group create");
                                    }
                                }
                            }

                            let _ = app_handle.emit(
                                "m2m://group-event",
                                GroupEvent {
                                    group_id: gid,
                                    event_type: "created".to_string(),
                                    peer_key_hex: Some(create.creator_peer_key_hex),
                                },
                            );
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to decrypt group create"),
                }
            }
        }
        PacketType::GroupInvite => {
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(invite) =
                            protocol::deserialize::<protocol::GroupInviteData>(&plaintext)
                        {
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
                                )
                                .is_ok()
                            };
                            drop(conn);

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
                                            let _ = store.upsert_group(
                                                &gid,
                                                &invite.group_name,
                                                now as i64,
                                                "member",
                                            );
                                            for key in &roster {
                                                let _ = store.add_group_member(
                                                    &gid, key, None, "member", now as i64,
                                                );
                                            }
                                        }
                                        drop(ms);

                                        if let Err(e) =
                                            fan_out_own_bundle(state.clone(), &gid, &roster, &our)
                                                .await
                                        {
                                            tracing::warn!(error = %e, group = %gid, "failed to announce own sender key after invite");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, group = %gid, "failed to join group from invite")
                                    }
                                }
                            }

                            let _ = app_handle.emit(
                                "m2m://group-event",
                                GroupEvent {
                                    group_id: gid,
                                    event_type: "invited".to_string(),
                                    peer_key_hex: Some(invite.inviter_peer_key_hex),
                                },
                            );
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to decrypt group invite"),
                }
            }
        }
        PacketType::GroupSenderKey => {
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(sk_data) =
                            protocol::deserialize::<protocol::GroupSenderKeyData>(&plaintext)
                        {
                            // Capture the transport peer's long-term identity key
                            // BEFORE releasing the connection: bundle signatures are
                            // verified against it (H2 trust model v2).
                            let peer_identity_pub = conn.session.peer_identity_pub;
                            drop(conn);
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
                                    if let Err(e) = send_own_bundle(
                                        state.clone(),
                                        &sk_data.group_id,
                                        &peer_key_hex,
                                        our,
                                    )
                                    .await
                                    {
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
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(group_msg) =
                            protocol::deserialize::<protocol::GroupEncryptedMessageData>(&plaintext)
                        {
                            let gid = group_msg.group_id.clone();
                            let sender = group_msg.sender_peer_key_hex.clone();
                            drop(conn);

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
                                let content_str =
                                    String::from_utf8_lossy(&decrypted_content).to_string();
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let msg_id = uuid::Uuid::new_v4().to_string();

                                // Ephemeral mode: group content stays in RAM.
                                // Both values are snapshotted from
                                // `security_config` in one read here, before the
                                // store lock is taken — reading them inside the
                                // `message_store` scope would nest
                                // `security_config` under `message_store`.
                                let (ephemeral_mode, storage_cap) = {
                                    let cfg = state.security_config.read().await;
                                    (cfg.ephemeral_mode, cfg.effective_storage_cap())
                                };
                                if !ephemeral_mode {
                                    state.ensure_message_store(&state.data_dir).await.ok();
                                    let sk = state.storage_key.read().await;
                                    let ms = state.message_store.lock().await;
                                    if let (Some(store), Some(key)) = (ms.as_ref(), sk.as_ref()) {
                                        // `group_messages` counts toward the cap,
                                        // so an attacker who only ever sends
                                        // group traffic must not be a way
                                        // around it.
                                        crate::maintenance::enforce_cap(
                                            app_handle,
                                            store,
                                            storage_cap,
                                        );
                                        match super::util::crypto_encrypt_storage(
                                            content_str.as_bytes(),
                                            key,
                                            super::util::AAD_MSG_STORE,
                                        ) {
                                            Ok((nonce, encrypted)) => {
                                                let _ = store.store_group_message(
                                                    &msg_id, &gid, &sender, &encrypted, &nonce,
                                                    now as i64, true,
                                                );
                                                let preview = super::util::truncate_utf8(
                                                    &content_str,
                                                    80,
                                                    "...",
                                                );
                                                let _ = store.update_group_last_message(
                                                    &gid, now as i64, &preview,
                                                );
                                            }
                                            Err(e) => {
                                                tracing::warn!(error = %e, "failed to encrypt group message for storage")
                                            }
                                        }
                                    }
                                    drop(ms);
                                    drop(sk);
                                }
                                let _ = app_handle.emit(
                                    "m2m://group-message",
                                    GroupMessageEvent {
                                        group_id: gid,
                                        message: ChatMessage::new(
                                            msg_id,
                                            content_str,
                                            "received".to_string(),
                                            now,
                                        )
                                        .with_sender(sender),
                                    },
                                );
                            }
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to decrypt group message"),
                }
            }
        }
        PacketType::GroupInfo => {
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(info) =
                            protocol::deserialize::<protocol::GroupInfoData>(&plaintext)
                        {
                            let new_name = info.new_name.clone();
                            let gid = info.group_id.clone();
                            let changer_is_admin;
                            {
                                let gm = state.group_manager.read().await;
                                changer_is_admin = gm
                                    .get_group(&gid)
                                    .map(|g| {
                                        g.is_member(&peer_key_hex)
                                            && g.is_admin(&info.changed_by_peer_key_hex)
                                            && info.changed_by_peer_key_hex == peer_key_hex
                                    })
                                    .unwrap_or(false);
                            }
                            drop(conn);

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

                            let _ = app_handle.emit(
                                "m2m://group-event",
                                GroupEvent {
                                    group_id: gid,
                                    event_type: "name_changed".to_string(),
                                    peer_key_hex: Some(info.changed_by_peer_key_hex),
                                },
                            );
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to decrypt group info"),
                }
            }
        }
        PacketType::GroupRemove => {
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(remove) =
                            protocol::deserialize::<protocol::GroupRemoveData>(&plaintext)
                        {
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
                                if let (Some(sk_data), Some(our)) =
                                    (&remove.new_sender_key, &our_peer_key_hex)
                                {
                                    match gm.handle_sender_key(sk_data, our, &peer_identity_pub) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            tracing::warn!(error = %e, "rejected rotated sender key")
                                        }
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

                            let _ = app_handle.emit(
                                "m2m://group-event",
                                GroupEvent {
                                    group_id: gid,
                                    event_type: "member_removed".to_string(),
                                    peer_key_hex: Some(removed),
                                },
                            );
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to decrypt group remove"),
                }
            }
        }
        PacketType::GroupLeave => {
            if let Some(conn_arc) = state.peer_connection(&peer_key_hex).await {
                let mut conn = conn_arc.lock().await;
                match conn.session.decrypt_typed_frame(frame) {
                    Ok(plaintext) => {
                        if let Ok(leave) =
                            protocol::deserialize::<protocol::GroupLeaveData>(&plaintext)
                        {
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

                            let _ = app_handle.emit(
                                "m2m://group-event",
                                GroupEvent {
                                    group_id: gid,
                                    event_type: "member_left".to_string(),
                                    peer_key_hex: Some(leaving),
                                },
                            );
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "failed to decrypt group leave"),
                }
            }
        }
        _ => {}
    }
}

/// Maximum inbound file-transfer chunk frames per second, per connection.
///
/// `network::MAX_INBOUND_FRAMES_PER_SEC` (30/s) is sized for *control* traffic:
/// typing indicators, reactions, heartbeats — things a human produces. Charging
/// bulk data frames against it capped every legitimate transfer at 30 × 256 KiB
/// = 7.68 MB/s, and 30 × 128 KiB = 3.84 MB/s over a relay, because
/// `compute_chunk_size` returns `MAX_FILE_CHUNK_SIZE` (256 KiB) for every
/// non-relay strategy and `send_file_chunks_inner` paces nothing at all — it
/// reads, hashes and writes the next chunk as fast as the link allows. A 1 GiB
/// transfer therefore cleared the one-second burst allowance and then had
/// *every* frame rejected, so `rate_limit_strikes` never reset and the session
/// was dropped mid-file with a `disconnected` event. That is a false positive on
/// an abuse heuristic, triggered by the product's main feature working.
///
/// 1000/s is not a throttle on any transfer this build can produce: the byte
/// ceiling below binds first, at 64 MiB/s, which is 256 chunk-frames/s at
/// 256 KiB and 512/s at 128 KiB. The cap binds only for a peer sending chunks
/// *smaller* than 64 KiB, and its purpose is to bound per-frame dispatch cost
/// (read syscall, MessagePack parse, AEAD open, SHA-256) when a peer declares
/// tiny chunks — the case in which a byte-only budget would otherwise become a
/// 65536-frames-per-second CPU allowance. A peer cannot widen that allowance by
/// choosing a chunk size; it can only choose to accept a 1000/s ceiling.
const MAX_INBOUND_CHUNK_FRAMES_PER_SEC: u32 = 1000;

/// Maximum inbound bulk-transfer bytes per second, per connection.
///
/// This is the budget that actually bounds a file transfer, so it has to sit
/// above what a real link delivers — otherwise the limiter simply becomes the
/// reason transfers fail. The control-frame byte budget (16 MiB/s) is not safe
/// to reuse here: a 1 GiB transfer over a 1 Gbps LAN sustains ~119 MiB/s, so at
/// 16 MiB/s the bucket drains in well under a second and every later frame is
/// rejected, which is the same dropped-mid-file failure at twice the threshold.
/// 64 MiB/s is above every non-local path (1 Gbps peaks at 119 MiB/s) and below
/// the 250 MiB/s that the 1000-frames/s cap alone would permit at 256 KiB
/// chunks, so both budgets stay meaningful.
///
/// Cost of the change: a connection now has two byte budgets instead of one,
/// so the per-connection ceiling is 16 MiB/s of control frames plus 64 MiB/s of
/// transfer data. Both are hard token buckets; neither is unbounded.
const MAX_INBOUND_CHUNK_BYTES_PER_SEC: u32 = 64 * 1024 * 1024;

/// Is this frame part of the bulk file-transfer data path?
///
/// Both directions of the per-chunk protocol qualify: the chunk itself, and the
/// `FileTransferChunkAck` the receiver returns for each one. The ACK rate is
/// dictated by the data rate — at 64 MiB/s of 256 KiB chunks the *sender*
/// receives 256 ACKs/s — so leaving ACKs on the 30/s control budget would drop
/// the sender's own session mid-transfer for a transfer the byte budget had
/// already allowed.
fn is_bulk_transfer_frame(packet_type: PacketType) -> bool {
    matches!(
        packet_type,
        PacketType::FileTransferChunk | PacketType::FileTransferChunkAck
    )
}

/// Spawn the receive loop and its heartbeat worker for one established session.
///
/// `conn_arc` is the *identity* of this session. Both workers must be able to
/// tell "the connection I was spawned for died" apart from "the slot for this
/// peer now holds a different, live connection", and `connections` is keyed by
/// peer key alone — so a bare `remove(&peer_key_hex)` from either worker deletes
/// whatever is there now. See [`remove_own_connection`].
pub fn spawn_receive_loop(
    app_handle: AppHandle,
    state: Arc<AppState>,
    mut read_half: tokio::net::tcp::OwnedReadHalf,
    peer_key_hex: String,
    conn_arc: Arc<tokio::sync::Mutex<PeerConnection>>,
    reconnect_info: Option<crate::reconnect::ReconnectInfo>,
) {
    let hb_peer = peer_key_hex.clone();
    let hb_state = state.clone();
    let hb_app = app_handle.clone();
    let hb_reconnect = reconnect_info.clone();
    let hb_conn = conn_arc.clone();
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
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(poll_secs.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;

            // Lock *this* session, never whatever currently holds the map
            // slot. A re-dial for the same peer replaces the entry, and resolving
            // by peer key here would probe the replacement's socket while this
            // worker still believed it owned the old one — and, worse, clear the
            // replacement's liveness bookkeeping. `hb_conn` is the same identity
            // `remove_own_connection` below compares against.
            let mut dead_reason: Option<String> = None;
            {
                // Still the live session? `lock_vault` drains the map, and a
                // re-dial replaces the entry, so both retire this worker.
                let still_current = {
                    let map = hb_state.connections.read().await;
                    map.get(&hb_peer).is_some_and(|c| Arc::ptr_eq(c, &hb_conn))
                };
                if !still_current {
                    break;
                }
                let mut conn = hb_conn.lock().await;

                // Liveness check: was the most recent probe answered in time?
                if let Some(sent_at) = conn.last_hb_sent {
                    let acked = matches!(conn.last_hb_ack, Some(ack_at) if ack_at >= sent_at);
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
                    let probe_due = conn.last_hb_sent.is_none_or(|sent_at| {
                        sent_at.elapsed()
                            >= std::time::Duration::from_secs(
                                crate::protocol::HEARTBEAT_INTERVAL_SECS,
                            )
                    });
                    if probe_due {
                        // Encrypted heartbeat — sent through the session's
                        // AEAD path (plaintext heartbeats were a liveness
                        // oracle and forgeable by any active attacker).
                        let crate::state::PeerConnection {
                            session,
                            write_half,
                            ..
                        } = &mut *conn;
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
                let was_verified = hb_reconnect
                    .as_ref()
                    .map(|ri| ri.peer_verified)
                    .unwrap_or(false);
                let _ = hb_app.emit(
                    "m2m://connection",
                    ConnectionEvent {
                        peer_key_hex: hb_peer.clone(),
                        state: "disconnected".to_string(),
                        peer_fingerprint: None,
                        peer_verified: was_verified,
                    },
                );
                remove_own_connection(&hb_state, &hb_peer, &hb_conn).await;
                break;
            }
        }
    });

    tokio::spawn(async move {
        // Per-connection inbound budget. Created once per established session
        // and owned by this receive loop, so it resets when the peer
        // reconnects and cannot be shared or manipulated across peers.
        let frame_limiter = network::FrameRateLimiter::new();
        // A second, separate budget for bulk transfer frames. Both buckets are
        // token buckets and both are still enforced; only the *frame* ceiling
        // differs, which is the whole point — see `is_bulk_transfer_frame` for
        // why charging 256 KiB chunks against a 30-frames/s control budget
        // dropped every transfer faster than 7.68 MB/s.
        let bulk_frame_limiter = network::FrameRateLimiter::with_limits(
            MAX_INBOUND_CHUNK_FRAMES_PER_SEC,
            MAX_INBOUND_CHUNK_BYTES_PER_SEC,
        );
        // Consecutive over-budget frames; reset by any accepted frame.
        let mut rate_limit_strikes: u32 = 0;

        // Captured out of the outer scope so the teardown paths below can use
        // `Arc::ptr_eq` instead of a bare `remove(&peer_key_hex)`, which cannot
        // distinguish this dead session from a live replacement that has since
        // taken the same slot. See `remove_own_connection`.
        let my_conn = conn_arc.clone();

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
                    let was_verified = reconnect_info
                        .as_ref()
                        .map(|ri| ri.peer_verified)
                        .unwrap_or(false);
                    let _ = app_handle.emit(
                        "m2m://connection",
                        ConnectionEvent {
                            peer_key_hex: peer_key_hex.clone(),
                            state: "disconnected".to_string(),
                            peer_fingerprint: None,
                            peer_verified: was_verified,
                        },
                    );
                    // Remove connection
                    remove_own_connection(&state, &peer_key_hex, &my_conn).await;
                    break;
                }
            };

            // ── Inbound rate limit ──
            // Charged on the frame's declared wire size, before any
            // deserialization, database write, or event emission. The
            // limiter is a token bucket, so brief bursts pass while a
            // sustained flood does not.
            //
            // Bulk transfer frames are charged against `bulk_frame_limiter`
            // instead of `frame_limiter`. Both are token buckets over both a
            // frame count and a byte count, so a peer still cannot flood:
            // it simply cannot flood with 256 KiB chunks, because the frame
            // count it is allowed is derived from the transfer, not from the
            // typing-indicator budget. This is what fixes the dropped-mid-file
            // false positive documented on `MAX_INBOUND_CHUNK_FRAMES_PER_SEC`.
            //
            // A breach is treated as fatal for the connection: once a peer is
            // demonstrably over budget, continuing to serve it would just
            // mean the attacker picks which frames get processed.
            //
            // A breach is not immediately fatal. The token bucket already
            // tolerated a full second of burst, so one breach means a full
            // second over budget — strong evidence of a flood — but dropping an
            // established session on the first one makes a false positive
            // expensive. The frame is dropped either way; only a *sustained*
            // breach (MAX_INBOUND_RATE_LIMIT_STRIKES) ends the connection.
            let bulk = is_bulk_transfer_frame(frame.packet_type);
            let limiter = if bulk {
                &bulk_frame_limiter
            } else {
                &frame_limiter
            };
            match limiter.check(frame.body.len()) {
                network::RateLimitVerdict::Allowed => {
                    // A single accepted frame clears the strike counter, so
                    // the peer must be continuously over budget to be dropped.
                    rate_limit_strikes = 0;
                }
                verdict @ (network::RateLimitVerdict::TooManyFrames
                | network::RateLimitVerdict::TooManyBytes) => {
                    rate_limit_strikes += 1;
                    tracing::warn!(
                        peer = %peer_key_hex,
                        bytes = frame.body.len(),
                        ?verdict,
                        bulk,
                        strike = rate_limit_strikes,
                        of = network::MAX_INBOUND_RATE_LIMIT_STRIKES,
                        "inbound rate limit exceeded — frame dropped"
                    );
                    if rate_limit_strikes < network::MAX_INBOUND_RATE_LIMIT_STRIKES {
                        continue;
                    }
                    tracing::warn!(
                        peer = %peer_key_hex,
                        "sustained rate limiting — dropping connection"
                    );
                    let _ = app_handle.emit(
                        "m2m://connection",
                        ConnectionEvent {
                            peer_key_hex: peer_key_hex.clone(),
                            state: "disconnected".to_string(),
                            peer_fingerprint: None,
                            peer_verified: reconnect_info
                                .as_ref()
                                .map(|ri| ri.peer_verified)
                                .unwrap_or(false),
                        },
                    );
                    remove_own_connection(&state, &peer_key_hex, &my_conn).await;
                    break;
                }
            }

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
            if matches!(
                frame.packet_type,
                PacketType::Heartbeat | PacketType::HeartbeatAck
            ) {
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
                    // A Disconnect is only honoured if it DECRYPTS. It used to
                    // be sent and accepted in plaintext, which made a 14-byte
                    // injected frame a universal session-kill primitive for any
                    // on-path attacker — and trivially for the relay server,
                    // which is a full MITM for relayed connections.
                    //
                    // The only cost of requiring authentication is that a peer
                    // whose session is already broken cannot announce its
                    // departure; it is reaped by the heartbeat timeout anyway.
                    let authenticated = {
                        match state.peer_connection(&peer_key_hex).await {
                            Some(c) => {
                                let mut c = c.lock().await;
                                c.session.decrypt_typed_frame(&frame).is_ok()
                            }
                            None => false,
                        }
                    };

                    if !authenticated {
                        tracing::warn!(
                            peer = %peer_key_hex,
                            "ignoring UNAUTHENTICATED disconnect frame \
                             (possible injection attempt)"
                        );
                        continue;
                    }

                    tracing::info!(peer = %peer_key_hex, "peer sent authenticated disconnect");
                    let was_verified = reconnect_info
                        .as_ref()
                        .map(|ri| ri.peer_verified)
                        .unwrap_or(false);
                    let _ = app_handle.emit(
                        "m2m://connection",
                        ConnectionEvent {
                            peer_key_hex: peer_key_hex.clone(),
                            state: "disconnected".to_string(),
                            peer_fingerprint: None,
                            peer_verified: was_verified,
                        },
                    );
                    remove_own_connection(&state, &peer_key_hex, &my_conn).await;
                    break;
                }
                PacketType::Error => {
                    tracing::warn!(peer = %peer_key_hex, "peer sent error packet");
                }
                // Typing indicators are SENT encrypted (see commands/chat.rs) but
                // were RECEIVED without decryption — the handler ignored the
                // body entirely, so any peer could inject fake typing events
                // into the victim's UI. Authenticate them like every other
                // encrypted packet type, and only surface a decryptable one.
                //
                // Note: the plaintext body is empty, so a successful AEAD open
                // is the entire check.
                PacketType::TypingIndicator | PacketType::TypingIndicatorClear => {
                    let typing = frame.packet_type == PacketType::TypingIndicator;
                    let authenticated = {
                        match state.peer_connection(&peer_key_hex).await {
                            Some(c) => {
                                let mut c = c.lock().await;
                                c.session.decrypt_typed_frame(&frame).is_ok()
                            }
                            None => false,
                        }
                    };
                    if !authenticated {
                        tracing::warn!(
                            peer = %peer_key_hex,
                            "ignoring unauthenticated typing indicator"
                        );
                        continue;
                    }
                    let _ = app_handle.emit(
                        "m2m://typing",
                        serde_json::json!({
                            "peer_key_hex": peer_key_hex,
                            "typing": typing,
                        }),
                    );
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
    use super::*;

    /// HIGH-5: the frame budget must be sized for control traffic, not for bulk
    /// data. A 256 KiB chunk charged against `MAX_INBOUND_FRAMES_PER_SEC` is
    /// what capped every transfer at 7.68 MB/s and dropped it mid-file.
    #[test]
    fn test_bulk_transfer_frames_are_not_control_frames() {
        assert!(is_bulk_transfer_frame(PacketType::FileTransferChunk));
        // ACKs are driven by the data rate, so they belong to the same budget.
        assert!(is_bulk_transfer_frame(PacketType::FileTransferChunkAck));
        assert!(!is_bulk_transfer_frame(PacketType::EncryptedMessage));
        assert!(!is_bulk_transfer_frame(PacketType::FileTransferRequest));
        assert!(!is_bulk_transfer_frame(PacketType::Heartbeat));
        assert!(!is_bulk_transfer_frame(PacketType::SyncRequest));
    }

    /// HIGH-5 sizing, stated as an assertion so the two constants cannot be
    /// retuned into a transfer-breaking combination without a test failing: the
    /// byte ceiling must be reachable at both chunk sizes `compute_chunk_size`
    /// emits (256 KiB direct, 128 KiB relay) *without* exhausting the frame cap,
    /// because a transfer is bounded by whichever budget runs out first.
    #[test]
    fn test_bulk_frame_cap_admits_a_full_rate_chunk_stream() {
        let direct_chunk = crate::protocol::MAX_FILE_CHUNK_SIZE;
        let relay_chunk = 128 * 1024usize;
        let bytes = MAX_INBOUND_CHUNK_BYTES_PER_SEC as usize;
        let frames = MAX_INBOUND_CHUNK_FRAMES_PER_SEC as usize;
        assert!(
            bytes / direct_chunk <= frames,
            "the byte ceiling must admit a full-rate direct chunk stream"
        );
        assert!(
            bytes / relay_chunk <= frames,
            "the byte ceiling must admit a full-rate relayed chunk stream"
        );
    }

    /// MEDIUM-12: the connection map is full at the cap, not one past it.
    #[test]
    fn test_connection_map_cap_boundary() {
        assert!(connection_map_has_room(0));
        assert!(connection_map_has_room(MAX_ESTABLISHED_CONNECTIONS - 1));
        assert!(!connection_map_has_room(MAX_ESTABLISHED_CONNECTIONS));
        assert!(!connection_map_has_room(MAX_ESTABLISHED_CONNECTIONS + 1));
    }

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

    /// In-order ACKs advance the watermark one chunk at a time.
    #[test]
    fn test_ack_watermark_advances_contiguously() {
        let mut last = 0u32;
        for expected in 1..=5u32 {
            let next = advance_ack_watermark(last, expected)
                .unwrap_or_else(|| panic!("chunk {expected} should advance from {last}"));
            last = next;
            // `chunks_acked` mirrors the contiguous prefix.
            assert_eq!(last + 1, expected + 1);
        }
        assert_eq!(last, 5);
    }

    /// A gap must not be honoured: ACK 0 then ACK 5 skips chunks 1-4.
    #[test]
    fn test_ack_watermark_rejects_gap() {
        let last = 0u32;
        assert_eq!(advance_ack_watermark(last, 0), None, "duplicate first ack");
        assert_eq!(
            advance_ack_watermark(last, 5),
            None,
            "a gap must not advance the confirmed prefix"
        );
    }

    /// Regression: one ACK for the final chunk must not confirm the whole file.
    ///
    /// This is the delivery-confirmation forgery. `wait_for_ack` treats
    /// `last_acked_index >= chunk_index` as delivered, so under the previous
    /// span-inflating arithmetic a single `chunk_index = total_chunks - 1`
    /// satisfied it for every remaining chunk.
    #[test]
    fn test_single_final_ack_does_not_confirm_whole_file() {
        let total_chunks = 8u32;
        let last = advance_ack_watermark(0, total_chunks - 1);
        assert_eq!(
            last, None,
            "a lone ack for the last chunk must not confirm unacked chunks"
        );
        // The honest sequence still works.
        let mut last = 0u32;
        for i in 1..total_chunks {
            last = advance_ack_watermark(last, i).expect("in-order ack");
        }
        assert_eq!(last, total_chunks - 1);
    }
}
