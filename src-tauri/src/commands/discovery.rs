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

use crate::error::AppError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;

use crate::dht;
use crate::ephemeral_id;
use crate::lan_discovery;
use crate::state::{AppState, DiscoveryConfig};

use super::util;
use super::{ConnectionEvent, ConnectionInfo};

/// How long the disable path waits for a discovery task to observe its cancel
/// flag and actually exit.
///
/// The two announcers poll at most every `CANCEL_POLL_INTERVAL` / on the tick
/// boundary, so this is generous. It is a bound rather than a wait-forever
/// because a UI command must not hang: on expiry the handles are cleared and
/// the situation is logged, and the re-checks inside both loops mean the worst
/// case is "no further announcement", not "announcement after teardown".
const STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

/// Poll interval used while waiting for a discovery task to stop.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(25);

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
) -> Result<DiscoveryConfig, AppError> {
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
    _app_handle: AppHandle,
    state: State<'_, Arc<AppState>>,
    config: DiscoveryConfig,
) -> Result<DiscoveryConfig, AppError> {
    // Air-gap mode: both LAN multicast and DHT announce leak presence.
    if (config.lan_enabled || config.dht_enabled) && state.security_config.read().await.air_gap_mode
    {
        return Err(AppError::blocked(
            "air-gap mode is enabled — peer discovery is blocked",
        ));
    }

    // Tor routing: LAN multicast does not leave the L2 domain, so it is not an
    // internet-IP leak — but it is still presence disclosure. The announcer
    // broadcasts our listening port and a rotating token to every host on the
    // local network every 30 seconds, indefinitely, and Tor gives no
    // protection against a local observer. The two settings are separate
    // toggles, so a user can plausibly have Tor on and LAN discovery left
    // enabled from before.
    //
    // This is refused rather than warned about: the user asked for anonymity
    // and this contradicts it, and the recovery is one toggle in the UI.
    if config.lan_enabled && crate::tor::is_enabled() {
        return Err(AppError::blocked(
            "LAN discovery is disabled while Tor routing is enabled — it broadcasts your \
             listening port and a rotating token to every host on this network, which Tor \
             cannot protect against",
        ));
    }

    // ── LAN Discovery ──
    if config.lan_enabled && !state.lan_cancel.read().await.is_some() {
        // Start LAN discovery.
        let lan_state = Arc::new(RwLock::new(lan_discovery::LanDiscoveryState::new()));
        let lan_cancel = Arc::new(AtomicBool::new(false));

        let listen_addr = {
            let val = state.listen_addr.read().await;
            Arc::new(RwLock::new(*val))
        };
        let eid = Arc::new(RwLock::new(ephemeral_id::EphemeralPeerId::generate()));

        // Awaited inline rather than wrapped in `tokio::spawn`. `start` does
        // nothing but bind the socket, set four options, join the multicast
        // group and spawn two tasks, so there is nothing to keep off this
        // thread — and awaiting it is what makes a failure *visible*. It used
        // to be spawned into the void, where a `LanDiscoveryError` became one
        // `tracing::warn!` line: the state handles were installed regardless, so
        // `lan_cancel` was `Some`, the UI showed LAN discovery as enabled, and
        // the toggle could never be re-enabled because the guard on this branch
        // is "not already running". A failed start looked exactly like a
        // working one.
        if let Err(e) =
            lan_discovery::start(listen_addr, lan_state.clone(), eid, lan_cancel.clone()).await
        {
            return Err(match e {
                lan_discovery::LanDiscoveryError::TorEnabled => AppError::blocked(
                    "LAN discovery is disabled while Tor routing is enabled — it broadcasts \
                     your listening port and a rotating token to every host on this network, \
                     which Tor cannot protect against",
                ),
                other => AppError::new(
                    "discovery.unavailable",
                    format!("LAN discovery could not start: {other}"),
                ),
            });
        }

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

        // Wait for the announcer and the listener to actually exit before
        // clearing anything. Previously the flag was set, `lan_state` was
        // immediately set to `None` and "DISABLED" was logged — while the
        // announcer was still asleep and would wake up and multicast this
        // node's real listening port one more time, after the UI had already
        // reported discovery as off. `get_discovered_peers` reported an empty
        // list for a service that was still running.
        await_lan_stop(state.inner()).await;

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

        // Share the live LAN state so the DHT can seed from peers LAN
        // discovery finds. Previously the two discovery mechanisms were
        // completely unbridged, which is part of why the DHT never
        // bootstrapped.
        // Clone the inner `Arc`, not the guard: `RwLock` is not `Clone`, and
        // holding the guard here would keep it alive across the whole DHT task.
        // Falls back to an empty state so the DHT simply has no LAN seeds yet.
        let lan_state_shared = state
            .lan_state
            .read()
            .await
            .clone()
            .unwrap_or_else(|| Arc::new(RwLock::new(lan_discovery::LanDiscoveryState::new())));
        let dht_state_clone = dht_state.clone();
        let app_for_loop = state.inner().clone();
        let eid = Arc::new(RwLock::new(ephemeral_id::EphemeralPeerId::generate()));
        let network_monitor = Arc::new(RwLock::new(ephemeral_id::NetworkMonitor::new()));
        let cancel_clone = dht_cancel.clone();

        // Publish the handles *before* spawning. `announce_loop` reads its
        // cancel flag before its first sleep and returns immediately if it is
        // already set, so installing first closes the window in which a user
        // toggled DHT discovery on and off again quickly: the old order (spawn,
        // then install) meant the task could be running — with the real listen
        // address in hand — before the flag that would stop it existed.
        {
            let mut ds = state.dht_state.write().await;
            *ds = Some(dht_state);
        }
        {
            let mut dc = state.dht_cancel.write().await;
            *dc = Some(dht_cancel);
        }

        tokio::spawn(async move {
            dht::announce_loop(
                dht_state_clone,
                lan_state_shared,
                eid,
                network_monitor,
                app_for_loop,
                cancel_clone,
            )
            .await;
        });

        tracing::info!("DHT discovery ENABLED");
    } else if !config.dht_enabled {
        // Stop DHT discovery
        if let Some(ref cancel) = *state.dht_cancel.read().await {
            cancel.store(true, Ordering::SeqCst);
        }

        // Same reason as the LAN case: an announce tick already past its sleep
        // had this node's real listen address and ephemeral id queued for
        // every seed. `DhtState::running` is the completion signal.
        await_dht_stop(state.inner()).await;

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

/// Wait for the LAN discovery tasks to observe cancellation and exit.
///
/// `lan_discovery::start` publishes `LanDiscoveryState::enabled == false` once
/// its last task has returned, which is the only completion signal available
/// here — the stop path holds the cancel flag, not the tasks' `JoinHandle`s.
/// The `Arc` is snapshotted before the inner lock is taken so no guard is held
/// across the await.
async fn await_lan_stop(state: &Arc<AppState>) {
    let deadline = tokio::time::Instant::now() + STOP_WAIT_TIMEOUT;
    loop {
        let lan_arc = state.lan_state.read().await.clone();
        let still_running = match lan_arc {
            Some(arc) => arc.read().await.enabled,
            None => false,
        };
        if !still_running {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!(
                timeout_secs = STOP_WAIT_TIMEOUT.as_secs(),
                "LAN discovery task did not exit after being cancelled — clearing state anyway"
            );
            return;
        }
        tokio::time::sleep(STOP_POLL_INTERVAL).await;
    }
}

/// Wait for the DHT announce loop to observe cancellation and exit.
///
/// `DhtState::running` is the mirror of `LanDiscoveryState::enabled`, for the
/// same reason: `dht::announce_loop` runs forever and its `JoinHandle` is
/// discarded at spawn, so completion has to be observable in the state object.
async fn await_dht_stop(state: &Arc<AppState>) {
    let deadline = tokio::time::Instant::now() + STOP_WAIT_TIMEOUT;
    loop {
        let dht_arc = state.dht_state.read().await.clone();
        let still_running = match dht_arc {
            Some(arc) => arc.read().await.running,
            None => false,
        };
        if !still_running {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!(
                timeout_secs = STOP_WAIT_TIMEOUT.as_secs(),
                "DHT announce loop did not exit after being cancelled — clearing state anyway"
            );
            return;
        }
        tokio::time::sleep(STOP_POLL_INTERVAL).await;
    }
}

/// Get the list of currently-discovered peers (LAN + DHT).
#[tauri::command]
pub async fn get_discovered_peers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<DiscoveredPeer>, AppError> {
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
            let addr = peer.connect_addr.map(|a| a.to_string()).unwrap_or_default();
            peers.push(DiscoveredPeer {
                id_hex: hex::encode(peer.peer_id),
                address: addr,
                method: "dht".to_string(),
                last_seen: peer.last_seen,
            });
        }
    }

    // Sort by last_seen descending (most recent first)
    peers.sort_by_key(|p| std::cmp::Reverse(p.last_seen));

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
) -> Result<ConnectionInfo, AppError> {
    let peer_addr: std::net::SocketAddr = address
        .parse()
        .map_err(|e| AppError::invalid(format!("invalid address: {e}")))?;

    let identity = state.identity.read().await;
    let kp = identity.as_ref().ok_or("identity not initialized")?;

    // Connect via the Tor-aware chokepoint: a DHT-discovered peer is
    // third-party, so this path must not reveal the real IP under Tor.
    let mut stream = crate::dial::dial(peer_addr)
        .await
        .map_err(|e| AppError::invalid(format!("connection failed: {e}")))?;

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
    all.sort_by_key(|c| std::cmp::Reverse(c.priority));
    let our_candidates = crate::dial::filter_advertised_candidates(
        all.iter()
            .map(|c| crate::protocol::WireCandidate {
                address: c.address.clone(),
                candidate_type: c.candidate_type as u8,
                relay_id: None,
            })
            .collect(),
    );

    let x25519 = state.x25519_identity.read().await;
    let x25519_pub = x25519
        .as_ref()
        .map(|k| k.public_key_bytes())
        .unwrap_or([0u8; 32]);

    // Use [0u8; 32] as expected_peer_pub to skip the pre-identity check.
    // After the handshake we extract the actual peer identity from the session.
    let expected_peer_pub = [0u8; 32];

    session
        .handshake_as_initiator(
            &mut stream,
            kp,
            &expected_peer_pub,
            our_candidates,
            x25519_pub,
        )
        .await?;

    let peer_key_hex = hex::encode(session.peer_identity_pub);
    let peer_fingerprint = session.peer_fingerprint();

    // Check if this peer is already known (previously verified)
    let message_store = state.message_store.lock().await;
    let is_known = message_store
        .as_ref()
        .map(|ms| ms.get_conversation(&peer_key_hex).ok().flatten().is_some())
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
        last_hb_sent: None,
        last_hb_ack: None,
    };

    // Bound to a name so the receive loop's teardown paths can identify THIS
    // session by `Arc::ptr_eq`. `connections` is keyed by peer key alone, so a
    // bare `remove(&peer_key_hex)` from a stale task deletes whatever now holds
    // that slot — including a live replacement. See
    // `commands::network::remove_own_connection`.
    let my_conn = Arc::new(tokio::sync::Mutex::new(conn));
    {
        let mut conns = state.connections.write().await;
        conns.insert(peer_key_hex.clone(), my_conn.clone());
    }

    // Emit connection event to frontend
    let _ = app_handle.emit(
        "m2m://connection",
        ConnectionEvent {
            peer_key_hex: peer_key_hex.clone(),
            state: "established".to_string(),
            peer_fingerprint: Some(peer_fingerprint.clone()),
            peer_verified: false,
        },
    );

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
        my_conn,
        None,
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
) -> Result<Vec<DiscoveredPeer>, AppError> {
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
