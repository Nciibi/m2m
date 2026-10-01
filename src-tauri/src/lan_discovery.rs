/// M2M — LAN Discovery
///
/// ⚠️ **PRIVACY WARNING** ⚠️
///
/// This module is **OFF by default**. When enabled, your app broadcasts
/// a presence announcement over WiFi every 30 seconds. Anyone on the
/// same network can see your presence. Use only on trusted networks.
///
/// When on, the announcement uses an **ephemeral session token** —
/// NOT your permanent identity key. The token changes every hour, so
/// observers cannot track you across sessions. But your IP address is
/// still visible to anyone on the same WiFi.
///
/// ## When to use
///
/// - **Safe**: Home WiFi, friends nearby, want zero-config setup
/// - **Unsafe**: Coffee shops, airports, conferences, any public WiFi
///
/// ## Protocol
///
/// UDP multicast to 239.255.27.3:38553.
///
/// Packet format:
///   [version: u8] [listen_port: u16 BE] [ephemeral_token: 32B]
///   [timestamp: u64 BE]
///
/// Note: No permanent identity key, no signature — the token is ephemeral
/// and carries no linkable information.
///
/// Total: 1 + 2 + 32 + 8 = 43 bytes
///
/// ## What that means for trust
///
/// The announcement is *not* authenticated in any way: the token is invented
/// by the sender for that one packet and proves nothing. The only field that
/// can be checked is the source address, and [`is_acceptable_lan_source`]
/// checks it — a peer must present a private, link-local or loopback IPv4
/// address. An entry that reaches `LanDiscoveryState::peers` is therefore
/// "some host on a local network said this", nothing more: it is a candidate
/// address the UI may offer to dial, not an identity. Nothing downstream may
/// treat it as authenticated — in particular `dht::lan_dht_seeds` only
/// promotes a peer to a DHT seed once a handshake has actually completed with
/// the address involved.
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

// Protocol module imported for potential future packet types

/// Multicast group address and port for LAN discovery.
/// Using 239.255.27.3:38553 — a non-standard multicast address in the
/// administratively-scoped range (239.255.0.0/16) to avoid conflicts
/// with other LAN services.
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 27, 3);

const MULTICAST_PORT: u16 = 38553;

/// Interval between successive LAN announcements (30 seconds).
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(30);

/// Time after which a peer is considered offline if no announcement
/// is received (90 seconds = 3 missed announcements).
const PEER_EXPIRY_SECS: u64 = 90;

/// Current LAN discovery protocol version.
const LAN_DISCOVERY_VERSION: u8 = 0x01;

/// Upper bound on the LAN peer table.
///
/// Every entry is keyed on a 32-byte token chosen by whoever sent the
/// datagram, and the datagram is not authenticated at all. Without a bound,
/// one host on the LAN can mint unlimited fresh tokens — growing this map,
/// and the list of diallable addresses the UI offers, without limit — well
/// inside the 90-second peer expiry window. See
/// [`LanDiscoveryState::insert_peer`].
const MAX_LAN_PEERS: usize = 64;

/// How often the listener re-reads the cancel flag while waiting on a
/// datagram.
///
/// The receive is now a genuine async wait, so on a quiet LAN no datagram
/// arrives for minutes. A flag check only at the top of the loop would keep
/// the socket open and the task alive long after the user disabled discovery.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// A peer discovered on the local network.
///
/// Contains only an **ephemeral session token** — NOT the peer's
/// permanent identity key. The token changes every hour, preventing
/// linkability across sessions or networks.
#[derive(Debug, Clone)]
pub struct LanPeer {
    /// Ephemeral session token (rotates hourly, NOT a permanent key).
    #[allow(dead_code)]
    pub session_token: [u8; 32],
    /// Hex of the session token (for display/lookup).
    pub token_hex: String,
    /// TCP address to connect to (for direct TCP or hole-punch).
    pub connect_addr: SocketAddr,
    /// Timestamp of the most recent announcement from this peer.
    pub last_seen: u64,
}

/// Active LAN discovery state.
pub struct LanDiscoveryState {
    /// Known peers on the local network, keyed by peer_key_hex.
    pub peers: HashMap<String, LanPeer>,
    /// Whether LAN discovery is enabled.
    pub enabled: bool,
}

impl LanDiscoveryState {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            enabled: false, // ⚠️ OFF by default — privacy first
        }
    }

    /// Remove peers that haven't announced within the expiry window.
    pub fn expire_stale_peers(&mut self) {
        let now = now_unix_secs();
        let cutoff = now.saturating_sub(PEER_EXPIRY_SECS);
        self.peers.retain(|_, peer| peer.last_seen >= cutoff);
    }

    /// Insert or refresh a peer, keeping the table bounded.
    ///
    /// This used to be a bare `peers.insert(key, peer)` with no upper bound,
    /// and the key is a 32-byte token chosen by the *sender* of an entirely
    /// unauthenticated datagram. A single host on the LAN could therefore send
    /// N packets with N distinct tokens and grow this map — each entry a
    /// candidate `connect_addr` the UI offers to dial, and, through
    /// `dht::lan_dht_seeds`, a DHT node — without limit, all inside the
    /// 90-second expiry window.
    ///
    /// Eviction is least-recently-seen first, which is the useful direction
    /// here: an attacker's freshly-minted tokens are the *most* recent
    /// entries, so they displace peers that have gone quiet — entries that are
    /// within seconds of the expiry sweep anyway — instead of growing the
    /// table without limit. A genuine peer that announces again immediately
    /// takes its place back at the top. Ties are broken on the key so eviction
    /// never depends on `HashMap` iteration order.
    pub fn insert_peer(&mut self, peer: LanPeer) {
        let key = peer.token_hex.clone();
        self.peers.insert(key, peer);

        while self.peers.len() > MAX_LAN_PEERS {
            // Snapshot the key out before the `remove`: taking it from the
            // iterator directly would keep an immutable borrow of `peers` live
            // across the mutable one and fail to borrow-check.
            let oldest: Option<String> = self
                .peers
                .iter()
                .min_by_key(|(k, p)| (p.last_seen, (*k).as_str()))
                .map(|(k, _)| (*k).clone());
            // `None` is unreachable while the map is non-empty; break rather
            // than spin so a future change can never turn this into a hang.
            let Some(k) = oldest else { break };
            self.peers.remove(&k);
            tracing::debug!(evicted = %k, "LAN peer table full — evicted oldest");
        }
    }
}

/// Error type for LAN discovery operations.
#[derive(Debug, thiserror::Error)]
pub enum LanDiscoveryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("crypto error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),
    /// LAN discovery refuses to run while Tor routing is enabled.
    ///
    /// LAN multicast never touches the Tor circuit, so enabling Tor provides no
    /// protection against it at all while the announcer keeps publishing this
    /// node's listening port to every host on the local network every 30
    /// seconds. Checked *inside* `start` so the refusal cannot depend on the
    /// order in which the two toggles were flipped — see [`start`].
    #[error("LAN discovery is disabled while Tor routing is enabled")]
    TorEnabled,
}

/// Bind a UDP socket for multicast reception on `port`, with address reuse set so
/// several instances on one host can all listen.
///
/// `SO_REUSEADDR`/`SO_REUSEPORT` must be set **before** `bind`, which
/// `std::net::UdpSocket::bind` cannot express — `std` exposes no socket-option
/// API at all. `socket2` is used rather than hand-rolled `extern "C"`
/// declarations because this is exactly the portability trap it exists to
/// abstract: a raw `socket()` returns `i32` on Unix but a `usize`-wide `SOCKET`
/// on Windows whose failure value is `INVALID_SOCKET` rather than `-1`, and
/// closing one needs `closesocket`, not the CRT `close`. Getting any of that
/// wrong compiles on one platform and corrupts the descriptor table on the
/// other. It was already in the dependency tree, so this pins it rather than
/// adding anything.
fn bind_multicast_listener(port: u16) -> Result<UdpSocket, LanDiscoveryError> {
    let domain = socket2::Domain::IPV4;
    let socket = socket2::Socket::new(domain, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))
        .map_err(LanDiscoveryError::Io)?;

    // Both before `bind`. `SO_REUSEADDR` is what lets a second instance bind the
    // same fixed port; without it the second process gets `EADDRINUSE` and
    // discovery silently stops working for that user. A failure here is fatal
    // for the same reason — carrying on would bind a socket that cannot receive
    // anything and report discovery as running.
    socket.set_reuse_address(true).map_err(LanDiscoveryError::Io)?;
    // Not fatal on its own: this option is absent on some platforms, and its
    // absence degrades to `SO_REUSEADDR` semantics rather than breaking a single
    // instance. Log it, because it is the difference between two instances
    // coexisting and the second one failing to start.
    if let Err(e) = socket.set_reuse_port(true) {
        tracing::warn!(
            error = %e,
            "SO_REUSEPORT unavailable - a second M2M instance on this host may not be \
             able to discover peers"
        );
    }

    socket
        .bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port).into())
        .map_err(LanDiscoveryError::Io)?;

    // `into()` hands the `socket2::Socket` to a `std` socket, which then owns
    // and closes the descriptor.
    Ok(socket.into())
}

/// Build a LAN discovery announcement packet using an ephemeral session token.
///
/// Packet format:
///   [version: u8] [listen_port: u16 BE] [session_token: 32B]
///   [timestamp: u64 BE]
///
/// No signature needed — the session token is ephemeral and carries
/// no linkable information. Identity is established during X3DH
/// handshake after connection.
///
/// Total: 1 + 2 + 32 + 8 = 43 bytes
fn build_announcement(listen_port: u16, session_token: &[u8; 32]) -> Vec<u8> {
    let timestamp = now_unix_secs();

    let mut packet = Vec::with_capacity(1 + 2 + 32 + 8);
    packet.push(LAN_DISCOVERY_VERSION);
    packet.extend_from_slice(&listen_port.to_be_bytes());
    packet.extend_from_slice(session_token);
    packet.extend_from_slice(&timestamp.to_be_bytes());

    packet
}

/// Parse a received LAN discovery announcement packet.
///
/// The packet contains an ephemeral session token, NOT a permanent
/// identity key. No signature verification needed — the token has
/// no linkable meaning. Identity is established during X3DH.
fn parse_announcement(packet: &[u8], sender: SocketAddr) -> Option<LanPeer> {
    if packet.len() != 43 {
        tracing::trace!(len = packet.len(), "ignoring LAN packet with wrong length");
        return None;
    }

    let mut offset = 0;

    // Version byte
    let version = packet[offset];
    offset += 1;
    if version != LAN_DISCOVERY_VERSION {
        return None;
    }

    // Listen port
    let listen_port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
    offset += 2;

    // Session token (32 bytes — ephemeral, no linkable info)
    let mut session_token = [0u8; 32];
    session_token.copy_from_slice(&packet[offset..offset + 32]);
    offset += 32;

    // Timestamp
    let timestamp = u64::from_be_bytes([
        packet[offset],
        packet[offset + 1],
        packet[offset + 2],
        packet[offset + 3],
        packet[offset + 4],
        packet[offset + 5],
        packet[offset + 6],
        packet[offset + 7],
    ]);

    // Reject stale timestamps (more than 5 minutes old)
    let now = now_unix_secs();
    if timestamp.saturating_add(300) < now {
        tracing::trace!("ignoring stale LAN announcement (timestamp too old)");
        return None;
    }
    // Reject future timestamps (clock skew protection, max 30s ahead)
    if timestamp > now.saturating_add(30) {
        tracing::trace!("ignoring LAN announcement with future timestamp");
        return None;
    }

    // The datagram is not authenticated in any way: the 32 bytes are an
    // ephemeral token the sender invented for this packet and nothing about
    // them can be verified. The source address is therefore the *only* field
    // that can be checked, and accepting an unchecked one is how an off-LAN
    // sender turns a spoofed multicast packet into both a diallable peer (the
    // UI offers `connect_addr` verbatim) and a DHT announce target, which
    // receives this node's real listen address and current ephemeral id.
    if !is_acceptable_lan_source(sender.ip()) {
        tracing::debug!(
            ip = %sender.ip(),
            "ignoring LAN announcement from a non-LAN source address"
        );
        return None;
    }

    // Port 0 is never connectable, and announcing it is pure table pollution:
    // it would occupy a bounded slot for a peer that can never be dialled.
    if listen_port == 0 {
        tracing::debug!("ignoring LAN announcement claiming listen port 0");
        return None;
    }

    let connect_addr = SocketAddr::new(sender.ip(), listen_port);
    let token_hex = hex::encode(session_token);

    Some(LanPeer {
        session_token,
        token_hex,
        connect_addr,
        last_seen: now,
    })
}

/// Whether `ip` is a source address LAN discovery may accept a peer from.
///
/// A peer discovered over LAN multicast is by definition on a local network,
/// so its address has to fall in RFC 1918 private space, RFC 3927 link-local
/// space, or loopback (two instances on one machine). Anything else — a routed
/// segment, a spoofed source, a crafted packet from anywhere — is rejected
/// before it can become a connectable peer or a DHT bootstrap seed.
///
/// Deliberately *not* accepted: CGNAT `100.64/10` and the RFC 5737 / RFC 2544
/// documentation and benchmarking ranges. None of them is a range a home or
/// office LAN is addressed from, and widening the accept set widens the set of
/// spoofable source addresses for no gain. The cost of a false negative is one
/// missed peer; the cost of a false positive is a deanonymisation primitive.
fn is_acceptable_lan_source(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        // The group is IPv4-only and the socket is bound to 0.0.0.0, so an
        // IPv6 source is never a legitimate announcement. Rejecting the whole
        // family also stops an IPv4-mapped source (`::ffff:192.168.1.5`) from
        // reaching the v4 classification as anything other than itself.
        IpAddr::V6(_) => false,
    }
}

/// Start the LAN discovery service.
///
/// This spawns two background tasks:
/// 1. **Listener**: Binds a UDP multicast socket and processes incoming announcements
/// 2. **Announcer**: Periodically broadcasts our ephemeral session token
///
/// The session token is ephemeral (rotates hourly) and is NOT your
/// permanent Ed25519 identity key. Observers on the same WiFi see
/// only a random token that changes frequently.
///
/// LAN discovery is **OFF by default**. Enable it in Settings.
///
/// Returns `Err` — and starts nothing — if Tor is enabled, or if the socket
/// cannot be bound or cannot join the multicast group. It never reports
/// success for a listener that would not have worked.
pub async fn start(
    listen_addr: Arc<RwLock<Option<std::net::SocketAddr>>>,
    lan_state: Arc<RwLock<LanDiscoveryState>>,
    ephemeral_id: Arc<RwLock<crate::ephemeral_id::EphemeralPeerId>>,
    cancel: Arc<AtomicBool>,
) -> Result<(), LanDiscoveryError> {
    // Defence in depth against the Tor refusal in
    // `commands::discovery::set_discovery_config`: that check is ordered
    // *before* the state handles are installed, and the Tor flag is global
    // and can be flipped at any time from Settings. Checking here as well
    // means a LAN announcer cannot exist at all while Tor is on, regardless
    // of toggle ordering or of a future caller that forgets the check — the
    // shape that let enabling Tor fail to stop the 30-second multicast that
    // publishes this node's listening port to every host on the network.
    if crate::tor::is_enabled() {
        return Err(LanDiscoveryError::TorEnabled);
    }

    // ═══ Bind to MULTICAST_PORT, not to an ephemeral port ═══
    //
    // This bound to `0.0.0.0:0`, a kernel-chosen ephemeral port, while
    // announcements are sent to `MULTICAST_PORT` (38553). A multicast datagram
    // is delivered only to sockets that have joined the group **and** whose
    // local port matches the destination port, so this socket could never
    // receive one — not its own, not anybody else's. LAN discovery did not work
    // at all: no peer was ever discovered, between any two instances, ever. The
    // feature looked correct because every other part of it was.
    //
    // Binding a fixed port is what makes two instances on one host collide, which
    // is why `bind_multicast_listener` sets `SO_REUSEADDR` before binding. This
    // is the standard multicast-receiver pattern: every instance binds the same
    // port with reuse set, and the kernel delivers a copy of each datagram to
    // each of them.
    //
    // The user-visible consequence: port 38553/udp must be allowed through the
    // firewall for discovery to work. That is inherent to multicast discovery,
    // not something to be optimised away by breaking the feature again.
    let socket = bind_multicast_listener(MULTICAST_PORT)?;

    // Do not loop our own announcements back to us. With loopback on, every
    // send is delivered to this host too, so `parse_announcement` accepted it
    // and the node listed *itself* as a discovered LAN peer — an entry the UI
    // then offered to connect to, and (via `dht::lan_dht_seeds`) a DHT node to
    // announce our own address to.
    socket.set_multicast_loop_v4(false).map_err(LanDiscoveryError::Io)?;

    // TTL 1 keeps the datagram on the local link, which is the entire intent
    // of LAN discovery. The platform default happens to be 1 on the common
    // stacks, but "happens to be" is not a property this disclosure should
    // depend on: a datagram that escaped onto a routed segment would be
    // readable by every AS between here and wherever it landed.
    socket.set_multicast_ttl_v4(1).map_err(LanDiscoveryError::Io)?;

    // Join the multicast group. This used to be `let _ = ...`, i.e. the error
    // was constructed and thrown away: a failed join (no route to the group,
    // the interface already having joined it, a firewall) left the socket bound
    // but subscribed to nothing, `start` still logged "LAN discovery started",
    // and the Settings toggle reported discovery as ON while no announcement
    // was ever sent or received. No join, no discovery — say so.
    socket
        .join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)
        .map_err(LanDiscoveryError::Io)?;

    // Hand the socket to the reactor. The listener used to call a *blocking*
    // `std::net::UdpSocket::recv_from` from inside `tokio::spawn`, with a 5s
    // read timeout, in a loop with no other await point. That occupies one
    // tokio worker thread essentially 100% of the time for as long as
    // discovery is enabled — half the runtime on a 2-core VM, starving every
    // receive loop, heartbeat and self-destruct timer in the messaging path —
    // and it also meant `cancel` was only observed after each 5s block.
    // `from_std` requires non-blocking mode to already be set.
    socket.set_nonblocking(true).map_err(LanDiscoveryError::Io)?;
    let socket = Arc::new(tokio::net::UdpSocket::from_std(socket).map_err(LanDiscoveryError::Io)?);
    let socket_listener = socket.clone();
    let socket_announcer = socket.clone();

    // Number of tasks this call owns. The caller (`commands::discovery`) only
    // holds the cancel flag, not our `JoinHandle`s, so completion is published
    // through the state object it already keeps a handle to: `enabled` is true
    // only while at least one task is still running. Without that signal the
    // stop path logged "LAN discovery DISABLED" and dropped the state handles
    // while the announcer was still mid-sleep, about to send one more
    // multicast of this node's real listening port.
    let live_tasks = Arc::new(AtomicUsize::new(2));

    {
        let mut state = lan_state.write().await;
        state.enabled = true;
    }

    tracing::info!(port = MULTICAST_PORT, "LAN discovery started");

    // ── Listener task ──
    let lan_state_listener = lan_state.clone();
    let live_tasks_listener = live_tasks.clone();
    let cancel_listener = cancel.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 512];
        loop {
            if cancel_listener.load(Ordering::SeqCst) {
                break;
            }
            // The receive is awaited, and the cancel flag is re-read on a
            // short interval alongside it. Waiting only for a datagram would
            // keep the socket subscribed to the group after the user turned
            // discovery off, so this host would still ingest announcements —
            // and the loop would still be holding the multicast subscription —
            // until somebody happened to speak on the LAN.
            let received = tokio::select! {
                res = socket_listener.recv_from(&mut buf) => Some(res),
                _ = tokio::time::sleep(CANCEL_POLL_INTERVAL) => None,
            };
            let Some(res) = received else {
                continue;
            };
            match res {
                Ok((n, sender)) => {
                    let packet = &buf[..n];
                    if let Some(peer) = parse_announcement(packet, sender) {
                        let mut state = lan_state_listener.write().await;

                        // Bounded insert: an unauthenticated sender controls
                        // the key, so the table cannot be left unbounded.
                        state.insert_peer(peer);
                        state.expire_stale_peers();

                        tracing::debug!(
                            peer_count = state.peers.len(),
                            "LAN peer discovered or updated"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "LAN discovery recv error");
                }
            }
        }
        tracing::info!("LAN discovery listener cancelled");
        task_finished(&lan_state_listener, &live_tasks_listener).await;
    });

    // ── Announcer task ──
    let lan_state_announcer = lan_state.clone();
    let live_tasks_announcer = live_tasks.clone();
    let cancel_announcer = cancel.clone();
    tokio::spawn(async move {
        loop {
            if cancel_announcer.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(ANNOUNCE_INTERVAL).await;

            // Re-read the flag *after* the sleep. Checking only at the top of
            // the loop meant a sleep that began while discovery was still
            // enabled ran to completion after the user disabled it and then
            // multicasted this node's real listening port anyway — a presence
            // disclosure published after the UI said discovery was off.
            if cancel_announcer.load(Ordering::SeqCst) {
                break;
            }

            // Check if network changed — rotate ephemeral ID
            {
                let eid = ephemeral_id.read().await;
                if eid.should_rotate() {
                    drop(eid);
                    let mut eid = ephemeral_id.write().await;
                    *eid = crate::ephemeral_id::EphemeralPeerId::generate();
                    tracing::debug!("LAN discovery session token rotated");
                }
            }

            let listen_port = {
                let addr = listen_addr.read().await;
                match *addr {
                    Some(sa) => sa.port(),
                    None => continue,
                }
            };

            let session_token = {
                let eid = ephemeral_id.read().await;
                eid.id
            };

            let packet = build_announcement(listen_port, &session_token);

            // Everything above this line awaits, so the flag has to be read
            // once more immediately before the send: this is the last point at
            // which cancellation can still prevent a disclosure.
            if cancel_announcer.load(Ordering::SeqCst) {
                break;
            }

            match socket_announcer
                .send_to(&packet, SocketAddr::new(IpAddr::V4(MULTICAST_ADDR), MULTICAST_PORT))
                .await
            {
                Ok(n) => {
                    tracing::trace!(bytes = n, "LAN announcement sent");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "LAN announcement send failed");
                }
            }
        }
        tracing::info!("LAN discovery announcer cancelled");
        task_finished(&lan_state_announcer, &live_tasks_announcer).await;
    });

    Ok(())
}

/// Publish "LAN discovery is no longer running" once the last task has exited.
///
/// The stop path in `commands::discovery` cannot await our `JoinHandle`s — it
/// only holds the cancel flag — so it polls `LanDiscoveryState::enabled`
/// instead. Without this, it cleared the state handles and logged "DISABLED"
/// while the announcer was still live, so `get_discovered_peers` reported an
/// empty list for a service that was still running and about to publish this
/// node's listening port one more time.
async fn task_finished(lan_state: &Arc<RwLock<LanDiscoveryState>>, live_tasks: &Arc<AtomicUsize>) {
    // `fetch_sub` returns the *previous* count, so only the task that takes it
    // from 1 to 0 is the last one out.
    if live_tasks.fetch_sub(1, Ordering::SeqCst) == 1 {
        let mut state = lan_state.write().await;
        state.enabled = false;
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod lan_discovery_tests {
    use super::*;

    #[test]
    fn test_build_announcement_success() {
        let session_token = [0xABu8; 32];
        let packet = build_announcement(9876, &session_token);

        // Packet format: version(1) + port(2) + token(32) + timestamp(8) = 43 bytes
        assert_eq!(packet.len(), 43, "announcement should be 43 bytes");
        assert_eq!(packet[0], LAN_DISCOVERY_VERSION, "version byte mismatch");

        // Listen port at offset 1-2
        let port = u16::from_be_bytes([packet[1], packet[2]]);
        assert_eq!(port, 9876);

        // Session token at offset 3-34
        assert_eq!(&packet[3..35], &session_token, "session token should match");
    }

    #[test]
    fn test_parse_valid_announcement() {
        let session_token = [0xCDu8; 32];
        let packet = build_announcement(5555, &session_token);

        let sender = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)), 9999);
        let peer = parse_announcement(&packet, sender).unwrap();

        assert_eq!(peer.session_token, session_token);
        assert_eq!(peer.connect_addr.port(), 5555);
        assert_eq!(peer.connect_addr.ip(), sender.ip());
    }

    #[test]
    fn test_parse_rejects_wrong_length() {
        let packet = vec![0u8; 50]; // Wrong length (should be 43)
        let sender = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 1234);
        assert!(parse_announcement(&packet, sender).is_none());
    }

    #[test]
    fn test_parse_rejects_unknown_version() {
        let session_token = [0xEEu8; 32];
        let mut packet = build_announcement(3333, &session_token);
        packet[0] = 0xFF; // Unknown version

        let sender = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3)), 7777);
        assert!(parse_announcement(&packet, sender).is_none());
    }

    #[test]
    fn test_expire_stale_peers() {
        let mut state = LanDiscoveryState::new();
        let old_time = now_unix_secs().saturating_sub(PEER_EXPIRY_SECS + 10);

        state.peers.insert(
            "old_peer".to_string(),
            LanPeer {
                session_token: [0xAA; 32],
                token_hex: "aa".to_string(),
                connect_addr: "10.0.0.1:1234".parse().unwrap(),
                last_seen: old_time,
            },
        );

        state.expire_stale_peers();
        assert!(state.peers.is_empty(), "stale peer should be removed");
    }

    #[test]
    fn test_keep_recent_peers() {
        let mut state = LanDiscoveryState::new();
        let now = now_unix_secs();

        state.peers.insert(
            "recent_peer".to_string(),
            LanPeer {
                session_token: [0xBB; 32],
                token_hex: "bb".to_string(),
                connect_addr: "10.0.0.2:5678".parse().unwrap(),
                last_seen: now,
            },
        );

        state.expire_stale_peers();
        assert_eq!(state.peers.len(), 1, "recent peer should be kept");
    }

    #[test]
    fn test_different_session_tokens_produce_different_packets() {
        let token_a = [0xAAu8; 32];
        let token_b = [0xBBu8; 32];

        let packet_a = build_announcement(1111, &token_a);
        let packet_b = build_announcement(1111, &token_b);

        // Same port, different tokens — packets should differ in the token section
        assert_ne!(
            packet_a[3..35],
            packet_b[3..35],
            "different tokens should produce different packets"
        );
    }

    fn v4(s: &str) -> IpAddr {
        IpAddr::V4(s.parse::<Ipv4Addr>().unwrap())
    }

    /// A spoofed or off-LAN source must not become a peer. `parse_announcement`
    /// authenticates nothing, so the address check is the only thing standing
    /// between a crafted datagram and a diallable `connect_addr` that the UI
    /// offers the user (and that `dht::lan_dht_seeds` hands to
    /// `announce_to_node`).
    #[test]
    fn test_parse_rejects_non_lan_source_addresses() {
        let packet = build_announcement(4444, &[0x5Au8; 32]);

        for ip in ["8.8.8.8", "1.1.1.1", "203.0.113.7", "100.64.0.1"] {
            let sender = SocketAddr::new(v4(ip), 9999);
            assert!(
                parse_announcement(&packet, sender).is_none(),
                "{ip} is not a LAN source address and must be rejected"
            );
        }

        // IPv6 cannot arrive on this IPv4-only socket, and must not be
        // smuggled through as an IPv4-mapped or IPv4-compatible address.
        for ip in ["2001:db8::1", "::ffff:192.168.1.5", "::1"] {
            let sender = SocketAddr::new(ip.parse::<std::net::IpAddr>().unwrap(), 9999);
            assert!(
                parse_announcement(&packet, sender).is_none(),
                "{ip} must be rejected on an IPv4-only group"
            );
        }
    }

    #[test]
    fn test_parse_accepts_lan_source_addresses() {
        let packet = build_announcement(4444, &[0x5Au8; 32]);

        for ip in [
            "192.168.1.42",
            "10.0.0.1",
            "172.16.5.5",
            "169.254.10.10",
            "127.0.0.1",
        ] {
            let sender = SocketAddr::new(v4(ip), 9999);
            assert!(
                parse_announcement(&packet, sender).is_some(),
                "{ip} is a legitimate LAN source and must be accepted"
            );
        }
    }

    /// Port 0 is not connectable, so accepting it only burns a bounded table
    /// slot on an entry nothing can ever dial.
    #[test]
    fn test_parse_rejects_zero_listen_port() {
        let packet = build_announcement(0, &[0x6Bu8; 32]);
        let sender = SocketAddr::new(v4("192.168.1.42"), 9999);
        assert!(parse_announcement(&packet, sender).is_none());
    }

    fn peer_named(name: &str, addr: &str, last_seen: u64) -> LanPeer {
        LanPeer {
            session_token: [0x7C; 32],
            token_hex: name.to_string(),
            connect_addr: addr.parse().unwrap(),
            last_seen,
        }
    }

    /// The table used to be an unbounded `HashMap` keyed on a 32-byte token the
    /// *sender* chose, so one LAN host could mint unlimited entries — each a
    /// diallable address the UI offers — inside the expiry window.
    #[test]
    fn test_insert_peer_is_bounded() {
        let mut state = LanDiscoveryState::new();
        let now = now_unix_secs();

        for i in 0..(MAX_LAN_PEERS * 4) {
            state.insert_peer(peer_named(
                &format!("tok{i}"),
                &format!("192.168.1.{}:4000", (i % 250) + 1),
                now,
            ));
        }

        assert!(
            state.peers.len() <= MAX_LAN_PEERS,
            "peer table must stay bounded, got {}",
            state.peers.len()
        );
    }

    /// A peer that is actively announcing must never be the one evicted,
    /// however many quiet entries the attacker has seeded behind it.
    #[test]
    fn test_insert_peer_evicts_least_recently_seen() {
        let mut state = LanDiscoveryState::new();
        let now = now_unix_secs();

        // Fill the table with entries that have already gone quiet.
        for i in 0..MAX_LAN_PEERS {
            state.insert_peer(peer_named(
                &format!("flood{i}"),
                &format!("192.168.1.{}:4000", (i % 250) + 20),
                now - 10,
            ));
        }

        // The genuine peer announces now, overflowing the bound by one.
        state.insert_peer(peer_named("genuine", "192.168.1.10:4000", now));

        assert_eq!(state.peers.len(), MAX_LAN_PEERS);
        assert!(
            state.peers.contains_key("genuine"),
            "the most recently seen peer must not be the one evicted"
        );
        assert!(!state.peers.contains_key("flood0"));
    }

    /// Refreshing an existing peer must not be treated as a new insert. A table
    /// sitting exactly at the bound is the case that catches it: a "refresh"
    /// that incremented the count would evict a peer for the crime of talking
    /// to us again.
    #[test]
    fn test_insert_peer_refresh_keeps_table_size() {
        let mut state = LanDiscoveryState::new();
        let now = now_unix_secs();

        for i in 0..MAX_LAN_PEERS {
            state.insert_peer(peer_named(
                &format!("peer{i}"),
                &format!("192.168.1.{}:4000", (i % 250) + 20),
                now,
            ));
        }
        assert_eq!(
            state.peers.len(),
            MAX_LAN_PEERS,
            "table should sit exactly at the bound"
        );

        // An existing peer announces again, five seconds later.
        state.insert_peer(peer_named("peer0", "192.168.1.20:4000", now + 5));

        assert_eq!(
            state.peers.len(),
            MAX_LAN_PEERS,
            "a refresh must refresh in place, not add a row"
        );
        assert_eq!(state.peers.get("peer0").map(|p| p.last_seen), Some(now + 5));
    }
}
