/// M2M — DHT Peer Discovery
///
/// ⚠️ **PRIVACY WARNING** ⚠️
///
/// This module is **OFF by default** for a reason: publishing your
/// presence to a DHT makes you *discoverable* but also *traceable*.
/// Anyone monitoring the DHT can see when you're online and what IP
/// you're using.
///
/// If you enable this, M2M uses an **ephemeral peer ID** — a random
/// 32-byte token that changes every 24 hours or whenever your IP
/// changes. This is NOT your permanent Ed25519 identity key, so
/// observers cannot link your DHT activity back to your real identity.
/// However: your IP address is still visible to DHT nodes.
///
/// ## When to use
///
/// - **Safe**: You want friends to find you without sharing invite links
/// - **Unsafe**: You're in a high-risk environment, using Tor, or want
///   maximum metadata protection — leave it OFF and use invite links.
///
/// ## Design
///
/// Lightweight Kademlia-style DHT client that uses configurable
/// bootstrap nodes for peer discovery. Announced peer IDs are
/// ephemeral (rotated periodically and on network change), NOT
/// your permanent identity key.
///
/// - **Announce**: Publish `(ephemeral_id, listen_addr)` — no permanent key exposed
/// - **Lookup**: Query for a peer by their ephemeral ID
/// - **Bootstrap**: Connect to bootstrap nodes to join the DHT network
///
/// ## NAT Awareness
///
/// Peers behind symmetric NATs act as "client-only" nodes (query and
/// announce only, don't serve routing table entries).
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time;

use thiserror::Error;


// ─── Constants ─────────────────────────────────────────────────────────────────

/// DHT protocol version.
#[expect(dead_code, reason = "Reserved for DHT wire protocol negotiation")]
const DHT_PROTOCOL_VERSION: u8 = 0x01;

/// Maximum DHT message body size (64 KiB).
const MAX_DHT_BODY: u32 = 65536;

/// TCP connect timeout for DHT operations.
const DHT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How often to re-announce our presence (10 minutes).
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(600);

/// How long until a peer's announcement expires (30 minutes).
const PEER_EXPIRY_SECS: u64 = 1800;

#[expect(dead_code, reason = "Reserved for DHT bootstrap limiting")]
const MAX_BOOTSTRAP_NODES: usize = 5;

#[expect(dead_code, reason = "Reserved for DHT parallel lookup limiting")]
const MAX_LOOKUP_PARALLEL: usize = 3;

// ─── DHT Message Types ─────────────────────────────────────────────────────────

#[expect(dead_code, reason = "Reserved DHT wire protocol message types")]
const DHT_PING: u8 = 0x01;
#[expect(dead_code, reason = "Reserved for DHT pong response")]
const DHT_PONG: u8 = 0x02;
const DHT_ANNOUNCE: u8 = 0x03;
const DHT_ANNOUNCE_OK: u8 = 0x04;
#[expect(dead_code, reason = "Reserved for DHT node lookup protocol")]
const DHT_FIND_NODE: u8 = 0x05;
#[expect(dead_code, reason = "Reserved for DHT node response protocol")]
const DHT_NODE_RESPONSE: u8 = 0x06;
#[expect(dead_code, reason = "Reserved for DHT error responses")]
const DHT_ERROR: u8 = 0xFF;

// ─── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum DhtError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("connection timeout")]
    Timeout,
    #[error("bad response: {0}")]
    BadResponse(String),
    #[expect(dead_code, reason = "Reserved error variant for peer lookup failures")]
    #[error("peer not found")]
    PeerNotFound,
    #[expect(dead_code, reason = "Error when DHT has not bootstrapped yet")]
    #[error("not bootstrapped")]
    NotBootstrapped,
    #[expect(dead_code, reason = "Error when DHT is disabled")]
    #[error("DHT not enabled")]
    NotEnabled,
}

// ─── Types ─────────────────────────────────────────────────────────────────────

/// A peer entry from the DHT.
///
/// Contains ONLY an ephemeral peer ID and IP address — NO permanent
/// identity key. Ephemeral IDs rotate on network change and every
/// 24 hours, preventing linkability.
#[derive(Debug, Clone)]
pub struct DhtPeer {
    /// The peer's ephemeral ID (rotates periodically, NOT their identity key).
    pub peer_id: [u8; 32],
    /// Current TCP address for connecting.
    pub connect_addr: Option<SocketAddr>,
    /// Protocol version the peer supports.
    pub protocol_version: u8,
    /// Last time this peer was seen (unix seconds).
    pub last_seen: u64,
}

/// A DHT bootstrap node configuration.
#[derive(Debug, Clone)]
pub struct BootstrapNode {
    pub address: SocketAddr,
}

/// DHT configuration.
///
/// **Default is OFF** — DHT must be explicitly enabled by the user.
#[derive(Debug, Clone)]
pub struct DhtConfig {
    /// Whether DHT discovery is enabled. **OFF by default.**
    /// Enabling this makes your IP visible to DHT nodes.
    pub enabled: bool,
    /// Bootstrap nodes to connect to on startup.
    pub bootstrap_nodes: Vec<BootstrapNode>,
    /// Whether we're behind a symmetric NAT (client-only mode).
    pub is_symmetric_nat: bool,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            enabled: false,  // ⚠️ OFF by default — privacy first
            bootstrap_nodes: Vec::new(),
            is_symmetric_nat: false,
        }
    }
}

/// Active DHT state.
pub struct DhtState {
    /// Known peers discovered via the DHT.
    pub peers: HashMap<String, DhtPeer>,
    /// DHT config.
    pub config: DhtConfig,
    /// Whether we've bootstrapped into the DHT network.
    pub bootstrapped: bool,
    /// Whether the DHT background task is running.
    pub running: bool,
}

impl DhtState {
    pub fn new(config: DhtConfig) -> Self {
        Self {
            peers: HashMap::new(),
            config,
            bootstrapped: false,
            running: false,
        }
    }

    /// Check whether DHT discovery is enabled.
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    /// Remove peers that haven't been seen within the expiry window.
    pub fn expire_stale_peers(&mut self) {
        let now = now_unix_secs();
        let cutoff = now.saturating_sub(PEER_EXPIRY_SECS);
        self.peers.retain(|_, peer| peer.last_seen >= cutoff);
    }
}

// ─── Wire Protocol ─────────────────────────────────────────────────────────────

/// Build a DHT message: [type (1B)] [body…]
fn build_dht_message(msg_type: u8, body: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(5 + body.len());
    let total_len = (1 + body.len()) as u32;
    msg.extend_from_slice(&total_len.to_be_bytes());
    msg.push(msg_type);
    msg.extend_from_slice(body);
    msg
}

/// Parse a DHT message: returns (type, body).
fn parse_dht_message(data: &[u8]) -> Result<(u8, &[u8]), DhtError> {
    if data.len() < 5 {
        return Err(DhtError::BadResponse("message too short".into()));
    }
    let _len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let msg_type = data[4];
    let body = &data[5..];
    if body.len() as u32 != _len - 1 {
        return Err(DhtError::BadResponse("length mismatch".into()));
    }
    if body.len() > MAX_DHT_BODY as usize {
        return Err(DhtError::BadResponse("body too large".into()));
    }
    Ok((msg_type, body))
}

// ─── Network Operations ────────────────────────────────────────────────────────

/// Send a DHT message to a peer.
async fn dht_send(stream: &mut TcpStream, msg_type: u8, body: &[u8]) -> Result<(), DhtError> {
    let msg = build_dht_message(msg_type, body);
    stream.write_all(&msg).await.map_err(DhtError::Io)
}

/// Read a single DHT message from a peer.
async fn dht_recv(stream: &mut TcpStream) -> Result<(u8, Vec<u8>), DhtError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            DhtError::BadResponse("connection closed".into())
        } else {
            DhtError::Io(e)
        }
    })?;
    let body_len = u32::from_be_bytes(len_buf) as usize;
    if body_len > MAX_DHT_BODY as usize || body_len < 1 {
        return Err(DhtError::BadResponse("invalid body length".into()));
    }
    let mut body = vec![0u8; body_len];
    stream.read_exact(&mut body).await.map_err(DhtError::Io)?;
    Ok((body[0], body[1..].to_vec()))
}

/// Connect to a DHT node and exchange a ping/pong to verify it's alive.
async fn dht_ping(addr: SocketAddr) -> Result<Duration, DhtError> {
    let start = std::time::Instant::now();
    let mut stream = time::timeout(DHT_CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| DhtError::Timeout)?
        .map_err(DhtError::Io)?;

    dht_send(&mut stream, DHT_PING, &[]).await?;

    let (resp_type, _) = time::timeout(DHT_CONNECT_TIMEOUT, dht_recv(&mut stream))
        .await
        .map_err(|_| DhtError::Timeout)?
        .map_err(|_| DhtError::Timeout)?;

    if resp_type != DHT_PONG {
        return Err(DhtError::BadResponse("expected PONG".into()));
    }

    Ok(start.elapsed())
}

// ─── Announce / Lookup ─────────────────────────────────────────────────────────

/// Build an announce body using an ephemeral peer ID.
///
/// The body contains ONLY the ephemeral ID and IP address — NO permanent
/// identity key, NO signature. This prevents observers from linking the
/// ephemeral ID to your real identity.
///
/// Format:
///   [ephemeral_id(32B)] [af_tag(1B)] [ip(4/16B)] [port(2B)]
fn build_announce_body(ephemeral_id: &[u8; 32], listen_addr: SocketAddr) -> Vec<u8> {
    let (af_tag, ip_bytes) = match listen_addr.ip() {
        std::net::IpAddr::V4(v4) => (4u8, v4.octets().to_vec()),
        std::net::IpAddr::V6(v6) => (6u8, v6.octets().to_vec()),
    };

    let mut body = Vec::with_capacity(32 + 1 + ip_bytes.len() + 2);
    body.extend_from_slice(ephemeral_id);
    body.push(af_tag);
    body.extend_from_slice(&ip_bytes);
    body.extend_from_slice(&listen_addr.port().to_be_bytes());
    body
}

/// Announce our presence to a bootstrap node.
/// Uses an ephemeral peer ID — NOT your permanent identity key.
pub async fn announce_to_node(
    node_addr: SocketAddr,
    ephemeral_id: &[u8; 32],
    listen_addr: SocketAddr,
) -> Result<(), DhtError> {
    let mut stream = time::timeout(DHT_CONNECT_TIMEOUT, TcpStream::connect(node_addr))
        .await
        .map_err(|_| DhtError::Timeout)?
        .map_err(DhtError::Io)?;

    let body = build_announce_body(ephemeral_id, listen_addr);
    dht_send(&mut stream, DHT_ANNOUNCE, &body).await?;

    let (resp_type, _) = time::timeout(DHT_CONNECT_TIMEOUT, dht_recv(&mut stream))
        .await
        .map_err(|_| DhtError::Timeout)?
        .map_err(|_| DhtError::Timeout)?;

    if resp_type != DHT_ANNOUNCE_OK {
        return Err(DhtError::BadResponse("announce rejected".into()));
    }

    Ok(())
}

/// Build a FIND_NODE body for a peer ID.
fn build_find_node_body(peer_id: &[u8; 32]) -> Vec<u8> {
    peer_id.to_vec()
}

/// Parse a NODE_RESPONSE body into a list of DHT peers.
///
/// Wire format per entry: [ephemeral_id(32B) ip(4B) port(2B)]
/// Note: NO permanent identity key is transmitted.
fn parse_node_response(body: &[u8]) -> Result<Vec<DhtPeer>, DhtError> {
    let entry_size = 32 + 4 + 2;
    let count = body.len() / entry_size;
    if body.len() % entry_size != 0 {
        return Err(DhtError::BadResponse("malformed node response".into()));
    }

    let mut peers = Vec::with_capacity(count);
    for i in 0..count {
        let offset = i * entry_size;
        let mut peer_id = [0u8; 32];
        peer_id.copy_from_slice(&body[offset..offset + 32]);
        // No identity_pub transmitted — ephemeral IDs are unlinkable
        let ip_offset = offset + 32;
        let ip_bytes: [u8; 4] = [
            body[ip_offset], body[ip_offset + 1],
            body[ip_offset + 2], body[ip_offset + 3],
        ];
        let port_offset = offset + 32 + 4;
        let port = u16::from_be_bytes([body[port_offset], body[port_offset + 1]]);

        let addr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(ip_bytes)), port);

        peers.push(DhtPeer {
            peer_id,
            connect_addr: Some(addr),
            protocol_version: 1,
            last_seen: now_unix_secs(),
        });
    }

    Ok(peers)
}

/// Look up a peer by their public key hash.
///
/// Queries the configured bootstrap nodes and returns the first valid response.
pub async fn lookup_peer(
    peer_id: &[u8; 32],
    bootstrap_nodes: &[BootstrapNode],
) -> Result<DhtPeer, DhtError> {
    if bootstrap_nodes.is_empty() {
        return Err(DhtError::NotBootstrapped);
    }

    let target_id = *peer_id; // Copy for the closure
    let body = build_find_node_body(peer_id);

    // Query all bootstrap nodes in parallel, take first success
    let mut handles = Vec::with_capacity(bootstrap_nodes.len().min(MAX_LOOKUP_PARALLEL));
    for node in bootstrap_nodes.iter().take(MAX_LOOKUP_PARALLEL) {
        let addr = node.address;
        let body_clone = body.clone();
        handles.push(tokio::spawn(async move {
            let mut stream = time::timeout(DHT_CONNECT_TIMEOUT, TcpStream::connect(addr))
                .await
                .map_err(|_| DhtError::Timeout)?
                .map_err(DhtError::Io)?;

            dht_send(&mut stream, DHT_FIND_NODE, &body_clone).await?;

            let inner = time::timeout(DHT_CONNECT_TIMEOUT, dht_recv(&mut stream))
                .await
                .map_err(|_| DhtError::Timeout)?;  // timeout → error
            let (resp_type, resp_body) = inner.map_err(|_| DhtError::Timeout)?;  // dht error

            if resp_type != DHT_NODE_RESPONSE {
                return Err(DhtError::BadResponse("expected NODE_RESPONSE".into()));
            }

            let peers = parse_node_response(&resp_body)?;
            peers.into_iter().find(|p| p.peer_id == target_id)
                .ok_or(DhtError::PeerNotFound)
        }));
    }

    // Return the first successful result
    let timeout_dur = Duration::from_secs(10);
    let deadline = std::time::Instant::now() + timeout_dur;
    let mut last_err = DhtError::PeerNotFound;

    for handle in handles {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match time::timeout(remaining, handle).await {
            Ok(Ok(Ok(peer))) => return Ok(peer),
            Ok(Ok(Err(e))) => last_err = e,
            Ok(Err(_)) => {} // JoinError
            Err(_) => {}     // timeout
        }
    }

    Err(last_err)
}

/// Background task that periodically announces to bootstrap nodes.
///
/// Uses an ephemeral peer ID (NOT the permanent identity key).
/// The ephemeral ID is generated fresh at startup and rotated every 24 hours
/// or on network change. Old IDs expire from the DHT automatically.
pub async fn announce_loop(
    dht_state: Arc<RwLock<DhtState>>,
    ephemeral_id: Arc<RwLock<crate::ephemeral_id::EphemeralPeerId>>,
    network_monitor: Arc<RwLock<crate::ephemeral_id::NetworkMonitor>>,
    listen_addr: Arc<RwLock<Option<std::net::SocketAddr>>>,
    cancel: Arc<AtomicBool>,
) {
    // Track the current ephemeral ID so we can re-announce if it rotates
    let mut current_id = ephemeral_id.read().await.id;

    loop {
        if cancel.load(Ordering::SeqCst) {
            tracing::info!("DHT announce loop cancelled");
            return;
        }

        time::sleep(ANNOUNCE_INTERVAL).await;

        // Check if we should rotate the ephemeral ID
        let should_rotate = {
            let eid = ephemeral_id.read().await;
            eid.should_rotate()
        };

        if should_rotate {
            let mut eid = ephemeral_id.write().await;
            *eid = crate::ephemeral_id::EphemeralPeerId::generate();
            current_id = eid.id;
            tracing::info!("DHT ephemeral peer ID rotated");
        }

        // Check for network change
        let network_changed = {
            let mut monitor = network_monitor.write().await;
            let local = crate::commands::util::resolve_local_ip();
            let public = None; // STUN check would be added separately
            monitor.check_for_change(local, public)
        };

        if network_changed {
            let mut eid = ephemeral_id.write().await;
            *eid = crate::ephemeral_id::EphemeralPeerId::generate();
            current_id = eid.id;
            tracing::info!("Network changed — DHT ephemeral peer ID rotated");
        }

        let nodes = {
            let state = dht_state.read().await;
            state.config.bootstrap_nodes.clone()
        };

        let addr = *listen_addr.read().await;

        let addr = match addr {
            Some(a) => a,
            None => continue,
        };

        for node in &nodes {
            if let Err(e) = announce_to_node(node.address, &current_id, addr).await {
                tracing::debug!(node = %node.address, error = %e, "DHT announce failed");
            }
        }
    }
}

// ─── Auxiliary ─────────────────────────────────────────────────────────────────

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod dht_tests {
    use super::*;

    fn init_crypto() {
        let _ = sodiumoxide::init();
    }

    fn make_identity() -> IdentityKeypair {
        IdentityKeypair::generate().unwrap()
    }

    #[test]
    fn test_build_dht_message() {
        let body = b"hello";
        let msg = build_dht_message(DHT_ANNOUNCE, body);

        // Length prefix (4B) + type (1B) + body
        assert_eq!(msg.len(), 4 + 1 + body.len());
        let len = u32::from_be_bytes([msg[0], msg[1], msg[2], msg[3]]);
        assert_eq!(len, 1 + body.len() as u32);
        assert_eq!(msg[4], DHT_ANNOUNCE);
        assert_eq!(&msg[5..], body);
    }

    #[test]
    fn test_parse_dht_message_valid() {
        let body = b"test data";
        let msg = build_dht_message(DHT_PONG, body);
        let (typ, parsed_body) = parse_dht_message(&msg).unwrap();
        assert_eq!(typ, DHT_PONG);
        assert_eq!(parsed_body, body);
    }

    #[test]
    fn test_parse_dht_message_too_short() {
        assert!(parse_dht_message(&[0u8; 3]).is_err());
    }

    #[test]
    fn test_parse_node_response() {
        let mut peer_id = [0u8; 32];
        peer_id[0] = 0xAA;
        // New wire format: no identity_pub — just ephemeral_id(32) + ip(4) + port(2)
        let ip = [10u8, 0, 0, 1];
        let port = 9876u16.to_be_bytes();

        let mut body = Vec::new();
        body.extend_from_slice(&peer_id);
        body.extend_from_slice(&ip);
        body.extend_from_slice(&port);

        let peers = parse_node_response(&body).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_id[0], 0xAA);
        assert_eq!(peers[0].connect_addr.unwrap().port(), 9876);
        assert_eq!(peers[0].connect_addr.unwrap().ip().to_string(), "10.0.0.1");
    }

    #[test]
    fn test_parse_node_response_malformed() {
        // Old 70-byte entry size (32+32+4+2) should fail new wire format (32+4+2=38)
        let body = vec![0u8; 70];
        assert!(parse_node_response(&body).is_err());
    }

    #[test]
    fn test_parse_node_response_empty() {
        let peers = parse_node_response(&[]).unwrap();
        assert!(peers.is_empty());
    }

    #[test]
    fn test_expire_stale_peers() {
        let mut state = DhtState::new(DhtConfig::default());
        let old_time = now_unix_secs().saturating_sub(PEER_EXPIRY_SECS + 10);

        state.peers.insert(
            "stale".to_string(),
            DhtPeer {
                peer_id: [0xAA; 32],
                connect_addr: None,
                protocol_version: 1,
                last_seen: old_time,
            },
        );

        state.expire_stale_peers();
        assert!(state.peers.is_empty(), "stale peer should be expired");
    }

    #[test]
    fn test_build_announce_body() {
        let ephemeral_id = [0xABu8; 32];
        let addr: SocketAddr = "192.168.1.5:9999".parse().unwrap();

        let body = build_announce_body(&ephemeral_id, addr);
        // Format: ephemeral_id(32) + af_tag(1) + ip(4) + port(2) = 39 bytes
        assert_eq!(body.len(), 32 + 1 + 4 + 2);
        // First 32 bytes should be the ephemeral ID
        assert_eq!(&body[..32], &[0xABu8; 32]);
    }

    #[test]
    fn test_dht_message_roundtrip() {
        let original = b"m2m-dht-test-payload";
        let msg = build_dht_message(DHT_ANNOUNCE, original);
        let (typ, parsed) = parse_dht_message(&msg).unwrap();
        assert_eq!(typ, DHT_ANNOUNCE);
        assert_eq!(parsed, original);
    }
}
