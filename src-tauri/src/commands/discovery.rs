//! M2M — Peer Discovery Commands
//!
//! Controls DHT and LAN peer discovery. Both are **OFF by default**
//! and must be explicitly enabled by the user in Settings.
//!
//! ## Privacy
//!
//! - LAN discovery broadcasts an ephemeral announcement over WiFi every 30s.
//! - DHT discovery publishes your ephemeral ID to bootstrap nodes.
//! - Both use ephemeral IDs that rotate periodically (NOT your permanent
//!   Ed25519 identity key), but your IP is still visible to observers.
//! - Enabling discovery while Private Mode is ON does **not** anonymize
//!   discovery traffic — your IP is exposed to the discovery channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

use crate::dht;
use crate::ephemeral_id;
use crate::lan_discovery;
use crate::state::{AppState, DiscoveryConfig};
use crate::tor;

use super::util;
use super::{ConnectionEvent, ConnectionInfo};

/// A peer discovered via LAN or DHT, exposed to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredPeer {
    /// Hex of the ephemeral session token (for display).
    pub id_hex: String,
    /// TCP address to connect to.
    pub address: String,
    /// How this peer was discovered ("lan" or "dht").
    pub method: String,
    /// Timestamp of last sighting (unix seconds).
    pub last_seen: u64,
}

/// Get the current discovery configuration.
#[tauri::command]
pub async fn get_discovery_config(
    state: State<'_, Arc<AppState>>,
) -> Result<DiscoveryConfig, String> {
    let config = state.discovery_config.read().await;
    Ok(config.clone())
}

/// Update discovery settings — starts or stops LAN/DHT services.
///
/// Both are **OFF by default** and must be explicitly enabled.
/// Enabling a method that's already running is a no-op.
/// Disabling a method that's not running is a no-op.
#[tauri::command]
pub async fn set_discovery_config(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    config: DiscoveryConfig,
) -> Result<DiscoveryConfig, String> {
    // ── LAN Discovery ──
    if config.lan_enabled && !state.lan_cancel.read().await.is_some() {
        // Start LAN discovery
        let lan_state = Arc::new(RwLock::new(lan_discovery::LanDiscoveryState::default()));
        let lan_cancel = Arc::new(AtomicBool::new(false));

        let listen_addr = match *state.listen_addr.read().await {
        let lan_state_clone = lan_state.clone();
        let eid = Arc::new(RwLock::new(ephemeral_id::EphemeralPeerId::generate()));
        let cancel_clone = lan_cancel.clone();

        tokio::spawn(async move {
            if let Err(e) = lan_discovery::start(listen_addr, lan_state_clone, eid, cancel_clone).await {
                tracing::warn!(error = %e, "LAN discovery failed to start");
            }
        });

        {
            let mut ls = state.lan_state.write().await;
            *ls = Some(lan_state);
        }
        {
            let mut lc = state.lan_cancel.write().await;
            *lc = Some(lan_cancel);
        }

        tracing::info!("LAN discovery ENABLED");
    } else if !config.lan_enabled {
        // Stop LAN discovery
        if let Some(ref cancel) = *state.lan_cancel.read().await {
            cancel.store(true, Ordering::SeqCst);
        }
        {
            let mut ls = state.lan_state.write().await;
            *ls = None;
        }
        {
            let mut lc = state.lan_cancel.write().await;
            *lc = None;
        }
        tracing::info!("LAN discovery DISABLED");
    }

    // ── DHT Discovery ──
    if config.dht_enabled && !state.dht_cancel.read().await.is_some() {
        // Start DHT discovery
        let dht_state = Arc::new(RwLock::new(dht::DhtState::new(dht::DhtConfig::default())));
        let dht_cancel = Arc::new(AtomicBool::new(false));

        let listen_addr = match *state.listen_addr.read().await {
        let dht_state_clone = dht_state.clone();
        let eid = Arc::new(RwLock::new(ephemeral_id::EphemeralPeerId::generate()));
        let network_monitor = Arc::new(RwLock::new(ephemeral_id::NetworkMonitor::new()));
        let cancel_clone = dht_cancel.clone();

        tokio::spawn(async move {
            dht::announce_loop(dht_state_clone, eid, network_monitor, listen_addr, cancel_clone).await;
        });

        {
            let mut ds = state.dht_state.write().await;
            *ds = Some(dht_state);
        }
        {
            let mut dc = state.dht_cancel.write().await;
            *dc = Some(dht_cancel);
        }

        tracing::info!("DHT discovery ENABLED");
    } else if !config.dht_enabled {
        // Stop DHT discovery
        if let Some(ref cancel) = *state.dht_cancel.read().await {
            cancel.store(true, Ordering::SeqCst);
        }
        {
            let mut ds = state.dht_state.write().await;
            *ds = None;
        }
        {
            let mut dc = state.dht_cancel.write().await;
            *dc = None;
        }
        tracing::info!("DHT discovery DISABLED");
    }

    // Persist config
    {
        let mut dc = state.discovery_config.write().await;
        *dc = config.clone();
    }

    Ok(config)
}

/// Get the list of currently-discovered peers (LAN + DHT).
#[tauri::command]
pub async fn get_discovered_peers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<DiscoveredPeer>, String> {
    let mut peers = Vec::new();

    // LAN peers
    if let Some(ref lan_state_arc) = *state.lan_state.read().await {
        let lan = lan_state_arc.read().await;
        for (_, peer) in lan.peers.iter() {
            peers.push(DiscoveredPeer {
                id_hex: peer.token_hex.clone(),
                address: peer.connect_addr.to_string(),
                method: "lan".to_string(),
                last_seen: peer.last_seen,
            });
        }
    }

    // DHT peers
    if let Some(ref dht_state_arc) = *state.dht_state.read().await {
        let dht = dht_state_arc.read().await;
        for (_, peer) in dht.peers.iter() {
            let addr = peer
                .connect_addr
                .map(|a| a.to_string())
                .unwrap_or_default();
            peers.push(DiscoveredPeer {
                id_hex: hex::encode(peer.peer_id),
                address: addr,
                method: "dht".to_string(),
                last_seen: peer.last_seen,
            });
        }
    }

    // Sort by last_seen descending (most recent first)
    peers.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));

    Ok(peers)
}

/// Connect to a discovered peer (no invite needed).
///
/// Performs a standard encrypted handshake. If the peer's identity is
/// already known (from a previous connection), the session is auto-trusted.
/// Otherwise the session is marked as "unverified" — the user must verify
/// the fingerprint out-of-band before sending messages.
#[tauri::command]
pub async fn connect_discovered_peer(
    app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    address: String,
) -> Result<ConnectionInfo, String> {
    let peer_addr: std::net::SocketAddr = address
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let identity = state.identity.read().await;
    let kp = identity
        .as_ref()
        .ok_or("identity not initialized")?;

    // Connect via TCP (respects Tor setting)
    let stream = tor::connect(peer_addr)
        .await
        .map_err(|e| format!("connection failed: {e}"))?;

    let mut session = crate::session::Session::new();

    // Gather our local candidates
    let config = state.stun_config.read().await;
    let stun_result = crate::stun::discover_public_addrs(&config).await.ok();
    drop(config);

    let host_candidates = crate::candidate::gather_host_candidates();
    let ipv6_candidates = crate::candidate::gather_ipv6_candidates();
    let reflexive_candidates = stun_result
        .as_ref()
        .map(crate::candidate::gather_reflexive_candidates)
        .unwrap_or_default();

    let mut all = host_candidates;
    all.extend(ipv6_candidates);
    all.extend(reflexive_candidates);
    all.sort_by(|a, b| b.priority.cmp(&a.priority));
    let our_candidates: Vec<crate::protocol::WireCandidate> = all
        .iter()
        .map(|c| crate::protocol::WireCandidate {
            address: c.address.clone(),
            candidate_type: c.candidate_type as u8,
            relay_id: None,
        })
        .collect();

    let x25519 = state.x25519_identity.read().await;
    let x25519_pub = x25519
        .as_ref()
        .map(|k| k.public_key_bytes())
        .unwrap_or([0u8; 32]);

    // Use [0u8; 32] as expected_peer_pub to skip the pre-identity check.
    // After the handshake we extract the actual peer identity from the session.
    let expected_peer_pub = [0u8; 32];

    session
        .handshake_as_initiator(&mut stream, kp, &expected_peer_pub, our_candidates, x25519_pub)
        .await
        .map_err(|e| format!("handshake failed: {e}"))?;

    let peer_key_hex = hex::encode(session.peer_identity_pub);
    let peer_fingerprint = session.peer_fingerprint();

    // Check if this peer is already known (previously verified)
    let message_store = state.message_store.lock().await;
    let is_known = message_store
        .as_ref()
        .map(|ms| {
            ms.get_conversation(&peer_key_hex)
                .ok()
                .flatten()
                .is_some()
        })
        .unwrap_or(false);
    drop(message_store);

    // If the peer is known, trust them. Otherwise leave as unverified.
    if is_known {
        session.mark_peer_verified();
    }

    // Split the stream
    let (read_half, write_half) = stream.into_split();

    let conn = crate::state::PeerConnection {
        write_half,
        session,
        remote_addr: peer_addr,
        strategy_name: format!("discovery-{}", "tcp"),
    };

    {
        let mut conns = state.connections.write().await;
        conns.insert(peer_key_hex.clone(), Arc::new(tokio::sync::Mutex::new(conn)));
    }

    // Emit connection event to frontend
    let _ = app_handle.emit("m2m://connection", ConnectionEvent {
        peer_key_hex: peer_key_hex.clone(),
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint.clone()),
    });

    // Upsert peer in key store
    if let Some(peer_key_bytes) = util::decode_peer_key_logged(&peer_key_hex) {
        let ks = state.key_store.lock().await;
        if let Some(ref store) = *ks {
            let _ = store.upsert_peer(&peer_key_bytes, &peer_fingerprint, None);
        }
    }

    // Start the receive loop
    crate::commands::network::spawn_receive_loop(
        app_handle,
        state.inner().clone(),
        read_half,
        peer_key_hex.clone(),
    );

    Ok(ConnectionInfo {
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint),
        peer_verified: is_known,
        peer_key_hex: Some(peer_key_hex),
    })
}

/// Refresh discovery state (force re-scan of LAN/DHT networks).
#[tauri::command]
pub async fn refresh_discovery(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<DiscoveredPeer>, String> {
    // LAN: expire stale peers explicitly
    if let Some(ref lan_state_arc) = *state.lan_state.read().await {
        let mut lan = lan_state_arc.write().await;
        lan.expire_stale_peers();
    }

    // DHT: expire stale peers explicitly
    if let Some(ref dht_state_arc) = *state.dht_state.read().await {
        let mut dht = dht_state_arc.write().await;
        dht.expire_stale_peers();
    }

    // Return the updated list
    get_discovered_peers(state).await
}
