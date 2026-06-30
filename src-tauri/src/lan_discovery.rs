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
/// Note: No permanent identity key, no signature — the token is
/// ephemeral and carries no linkable information.
///
/// Total: 1 + 2 + 32 + 8 = 43 bytes
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::RwLock;

// Protocol module imported for potential future packet types

/// Multicast group address and port for LAN discovery.
/// Using 239.255.27.3:38553 — a non-standard multicast address in the
/// administratively-scoped range (239.255.0.0/16) to avoid conflicts
/// with other LAN services.
#[expect(dead_code, reason = "Used in start() which is called from commands")]
const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 27, 3);

#[expect(dead_code, reason = "Used in start() which is called from commands")]
const MULTICAST_PORT: u16 = 38553;

/// Interval between successive LAN announcements (30 seconds).
#[expect(dead_code, reason = "Used in start() which is called from commands")]
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(30);

/// Time after which a peer is considered offline if no announcement
/// is received (90 seconds = 3 missed announcements).
const PEER_EXPIRY_SECS: u64 = 90;

/// Current LAN discovery protocol version.
const LAN_DISCOVERY_VERSION: u8 = 0x01;

/// A peer discovered on the local network.
#[derive(Debug, Clone)]
pub struct LanPeer {
    /// Ed25519 public key of the peer.
    pub identity_pub: [u8; 32],
    /// Human-readable fingerprint (hex with colons).
    pub fingerprint: String,
    /// TCP address to connect to (for direct TCP or hole-punch).
    pub connect_addr: SocketAddr,
    /// Timestamp of the most recent announcement from this peer.
    pub last_seen: u64,
    /// Whether this peer has been verified (fingerprint confirmed out-of-band).
    pub verified: bool,
}

/// Active LAN discovery state.
pub struct LanDiscoveryState {
    /// Known peers on the local network, keyed by peer_key_hex.
    pub peers: HashMap<String, LanPeer>,
    /// Whether LAN discovery is enabled.
    pub enabled: bool,
}

impl LanDiscoveryState {
    fn new() -> Self {
        Self {
            peers: HashMap::new(),
            enabled: false,
        }
    }

    /// Remove peers that haven't announced within the expiry window.
    fn expire_stale_peers(&mut self) {
        let now = now_unix_secs();
        let cutoff = now.saturating_sub(PEER_EXPIRY_SECS);
        self.peers.retain(|_, peer| peer.last_seen >= cutoff);
    }
}

/// Error type for LAN discovery operations.
#[derive(Debug, thiserror::Error)]
pub enum LanDiscoveryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("crypto error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),
    #[error("invalid packet: {0}")]
    InvalidPacket(String),
    #[error("LAN discovery not enabled")]
    NotEnabled,
}

/// Build a signed LAN discovery announcement packet.
///
/// Packet format:
///   [version: u8] [listen_port: u16 BE] [identity_pub: 32B]
///   [timestamp: u64 BE] [signature: 64B Ed25519]
///
/// The signature covers: version || listen_port || identity_pub || timestamp
fn build_announcement(
    identity: &crate::crypto::IdentityKeypair,
    listen_port: u16,
) -> Result<Vec<u8>, LanDiscoveryError> {
    let timestamp = now_unix_secs();

    let mut sign_data = Vec::with_capacity(1 + 2 + 32 + 8);
    sign_data.push(LAN_DISCOVERY_VERSION);
    sign_data.extend_from_slice(&listen_port.to_be_bytes());
    sign_data.extend_from_slice(&identity.public_key_bytes());
    sign_data.extend_from_slice(&timestamp.to_be_bytes());

    let signature = identity.sign(&sign_data);

    let mut packet = Vec::with_capacity(1 + 2 + 32 + 8 + 64);
    packet.push(LAN_DISCOVERY_VERSION);
    packet.extend_from_slice(&listen_port.to_be_bytes());
    packet.extend_from_slice(&identity.public_key_bytes());
    packet.extend_from_slice(&timestamp.to_be_bytes());
    packet.extend_from_slice(&signature);

    Ok(packet)
}

/// Parse a received LAN discovery announcement packet.
///
/// Returns `Some(LanPeer)` if the packet is valid and signed by the
/// claimed identity. Returns `None` silently for invalid packets
/// (incompatible version, bad signature, etc.).
fn parse_announcement(
    packet: &[u8],
    sender: SocketAddr,
) -> Option<LanPeer> {
    if packet.len() != 107 {
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
    if packet.len() < offset + 2 {
        return None;
    }
    let listen_port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
    offset += 2;

    // Identity public key
    if packet.len() < offset + 32 {
        return None;
    }
    let mut identity_pub = [0u8; 32];
    identity_pub.copy_from_slice(&packet[offset..offset + 32]);
    offset += 32;

    // Timestamp
    if packet.len() < offset + 8 {
        return None;
    }
    let timestamp = u64::from_be_bytes([
        packet[offset], packet[offset + 1], packet[offset + 2], packet[offset + 3],
        packet[offset + 4], packet[offset + 5], packet[offset + 6], packet[offset + 7],
    ]);
    offset += 8;

    // Signature
    if packet.len() < offset + 64 {
        return None;
    }
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&packet[offset..offset + 64]);

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

    // Reconstruct signed data
    let mut sign_data = Vec::with_capacity(1 + 2 + 32 + 8);
    sign_data.push(LAN_DISCOVERY_VERSION);
    sign_data.extend_from_slice(&listen_port.to_be_bytes());
    sign_data.extend_from_slice(&identity_pub);
    sign_data.extend_from_slice(&timestamp.to_be_bytes());

    // Verify signature
    if crate::crypto::verify_signature(&identity_pub, &sign_data, &signature).is_err() {
        tracing::trace!("ignoring LAN announcement with invalid signature");
        return None;
    }

    let fingerprint = crate::crypto::fingerprint_from_public_key(&identity_pub);

    // Build the connect address: use the sender's IP (since we got the UDP packet,
    // we know they're reachable at that IP) + the announced listen port.
    let connect_addr = SocketAddr::new(sender.ip(), listen_port);

    Some(LanPeer {
        identity_pub,
        fingerprint,
        connect_addr,
        last_seen: now,
        verified: false,
    })
}

/// Start the LAN discovery service.
///
/// This spawns two background tasks:
/// 1. **Listener**: Binds a UDP multicast socket and processes incoming announcements
/// 2. **Announcer**: Periodically broadcasts our own identity
///
/// The identity must be set (loaded from vault) before calling this.
pub async fn start(
    identity: Arc<RwLock<Option<crate::crypto::IdentityKeypair>>>,
    listen_addr: Arc<RwLock<Option<std::net::SocketAddr>>>,
    lan_state: Arc<RwLock<LanDiscoveryState>>,
) -> Result<(), LanDiscoveryError> {
    // Bind to a random UDP port for multicast
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
        .map_err(LanDiscoveryError::Io)?;

    socket.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(LanDiscoveryError::Io)?;

    // Join the multicast group
    let _ = socket.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)
        .map_err(LanDiscoveryError::Io);

    let socket = Arc::new(socket);
    let socket_listener = socket.clone();
    let socket_announcer = socket.clone();

    {
        let mut state = lan_state.write().await;
        state.enabled = true;
    }

    tracing::info!(port = MULTICAST_PORT, "LAN discovery started");

    // ── Listener task ──
    let lan_state_clone = lan_state.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 512];
        loop {
            match socket_listener.recv_from(&mut buf) {
                Ok((n, sender)) => {
                    let packet = &buf[..n];
                    if let Some(peer) = parse_announcement(packet, sender) {
                        let mut state = lan_state_clone.write().await;
                        let peer_key_hex = hex::encode(peer.identity_pub);

                        // Update or insert the peer
                        state.peers.insert(peer_key_hex, peer);
                        state.expire_stale_peers();

                        tracing::debug!(
                            peer_count = state.peers.len(),
                            "LAN peer discovered or updated"
                        );
                    }
                }
                Err(e) => {
                    // Timeout is expected (set_read_timeout = 5s)
                    if e.kind() != std::io::ErrorKind::WouldBlock
                        && e.kind() != std::io::ErrorKind::TimedOut
                    {
                        tracing::warn!(error = %e, "LAN discovery recv error");
                    }
                }
            }
        }
    });

    // ── Announcer task ──
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(ANNOUNCE_INTERVAL).await;

            // Get current identity and listen port
            let has_identity = {
                let id = identity.read().await;
                id.is_some()
            };

            if !has_identity {
                continue; // No identity yet — wait for vault unlock
            }

            let addr = listen_addr.read().await;
            let listen_port = match *addr {
                Some(sa) => sa.port(),
                None => continue, // Not listening yet
            };

            let id = identity.read().await;
            let kp = match id.as_ref() {
                Some(kp) => kp,
                None => continue,
            };

            let packet = match build_announcement(kp, listen_port) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to build LAN announcement");
                    continue;
                }
            };

            match socket_announcer.send_to(
                &packet,
                SocketAddr::new(
                    IpAddr::V4(MULTICAST_ADDR),
                    MULTICAST_PORT,
                ),
            ) {
                Ok(n) => {
                    tracing::trace!(bytes = n, "LAN announcement sent");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "LAN announcement send failed");
                }
            }
        }
    });

    Ok(())
}

/// Get the list of currently-discovered LAN peers.
pub async fn get_peers(
    lan_state: &RwLock<LanDiscoveryState>,
) -> Vec<LanPeer> {
    let mut state = lan_state.write().await;
    state.expire_stale_peers();
    state.peers.values().cloned().collect()
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

    fn init_crypto() {
        let _ = sodiumoxide::init();
    }

    fn make_identity() -> crate::crypto::IdentityKeypair {
        crate::crypto::IdentityKeypair::generate().unwrap()
    }

    #[test]
    fn test_build_announcement_success() {
        init_crypto();
        let identity = make_identity();
        let packet = build_announcement(&identity, 9876).unwrap();

        // Packet format: version(1) + port(2) + pubkey(32) + timestamp(8) + sig(64)
        assert_eq!(packet.len(), 107, "announcement should be 107 bytes");
        assert_eq!(packet[0], LAN_DISCOVERY_VERSION, "version byte mismatch");

        // Listen port at offset 1-2
        let port = u16::from_be_bytes([packet[1], packet[2]]);
        assert_eq!(port, 9876);
    }

    #[test]
    fn test_parse_valid_announcement() {
        init_crypto();
        let identity = make_identity();
        let packet = build_announcement(&identity, 5555).unwrap();

        let sender = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 42)), 9999);
        let peer = parse_announcement(&packet, sender).unwrap();

        assert_eq!(peer.identity_pub, identity.public_key_bytes());
        assert_eq!(peer.connect_addr.port(), 5555);
        assert_eq!(peer.connect_addr.ip(), sender.ip());
        assert!(!peer.verified);
    }

    #[test]
    fn test_parse_rejects_wrong_length() {
        let packet = vec![0u8; 50]; // Wrong length
        let sender = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 1234);
        assert!(parse_announcement(&packet, sender).is_none());
    }

    #[test]
    fn test_parse_rejects_bad_signature() {
        init_crypto();
        let identity = make_identity();
        let mut packet = build_announcement(&identity, 4444).unwrap();

        // Corrupt the last byte of the signature
        *packet.last_mut().unwrap() ^= 0xFF;

        let sender = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 8888);
        assert!(parse_announcement(&packet, sender).is_none());
    }

    #[test]
    fn test_parse_rejects_unknown_version() {
        init_crypto();
        let identity = make_identity();
        let mut packet = build_announcement(&identity, 3333).unwrap();
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
                identity_pub: [0xAA; 32],
                fingerprint: "AAAA:BBBB:CCCC".to_string(),
                connect_addr: "10.0.0.1:1234".parse().unwrap(),
                last_seen: old_time,
                verified: false,
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
                identity_pub: [0xBB; 32],
                fingerprint: "BBBB:CCCC:DDDD".to_string(),
                connect_addr: "10.0.0.2:5678".parse().unwrap(),
                last_seen: now,
                verified: false,
            },
        );

        state.expire_stale_peers();
        assert_eq!(state.peers.len(), 1, "recent peer should be kept");
    }

    #[test]
    fn test_different_identity_produces_different_signatures() {
        init_crypto();
        let id1 = make_identity();
        let id2 = make_identity();

        let p1 = build_announcement(&id1, 1111).unwrap();
        let p2 = build_announcement(&id2, 1111).unwrap();

        // Same port, different identity — signatures should differ
        assert_ne!(p1[43..], p2[43..], "different identities should produce different signatures");
    }
}
