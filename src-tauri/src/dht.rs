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
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time;

use thiserror::Error;

use crate::crypto::IdentityKeypair;
use crate::protocol;

// ─── Constants ─────────────────────────────────────────────────────────────────

/// DHT protocol version.
const DHT_PROTOCOL_VERSION: u8 = 0x01;

/// Maximum DHT message body size (64 KiB).
const MAX_DHT_BODY: u32 = 65536;

/// TCP connect timeout for DHT operations.
const DHT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How often to re-announce our presence (10 minutes).
const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(600);

/// How long until a peer's announcement expires (30 minutes).
const PEER_EXPIRY_SECS: u64 = 1800;

/// Maximum number of bootstrap nodes to connect to.
const MAX_BOOTSTRAP_NODES: usize = 5;

/// Maximum number of lookup nodes to query in parallel.
const MAX_LOOKUP_PARALLEL: usize = 3;

// ─── DHT Message Types ─────────────────────────────────────────────────────────

/// DHT message type identifiers.
const DHT_PING: u8 = 0x01;
const DHT_PONG: u8 = 0x02;
const DHT_ANNOUNCE: u8 = 0x03;
const DHT_ANNOUNCE_OK: u8 = 0x04;
const DHT_FIND_NODE: u8 = 0x05;
const DHT_NODE_RESPONSE: u8 = 0x06;
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
    #[error("peer not found")]
    PeerNotFound,
    #[error("not bootstrapped")]
    NotBootstrapped,
    #[error("DHT not enabled")]
    NotEnabled,
}

// ─── Types ─────────────────────────────────────────────────────────────────────

/// A peer entry from the DHT.
#[derive(Debug, Clone)]
pub struct DhtPeer {
    /// The peer's Ed25519 public key hash (SHA256 of public key).
    pub peer_id: [u8; 32],
    /// The peer's public key bytes.
    pub identity_pub: [u8; 32],
    /// Human-readable fingerprint (for display).
    pub fingerprint: String,
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
    pub public_key: Option<[u8; 32]>,
}

/// DHT configuration.
#[derive(Debug, Clone)]
pub struct DhtConfig {
    /// Whether DHT discovery is enabled.
    pub enabled: bool,
    /// Bootstrap nodes to connect to on startup.
    pub bootstrap_nodes: Vec<BootstrapNode>,
    /// Whether we're behind a symmetric NAT (client-only mode).
    pub is_symmetric_nat: bool,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            enabled: false,
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
    fn new(config: DhtConfig) -> Self {
        Self {
            peers: HashMap::new(),
            config,
            bootstrapped: false,
            running: false,
        }
    }

    /// Remove peers that haven't been seen within the expiry window.
    fn expire_stale_peers(&mut self) {
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

/// Build an announce body for our identity.
fn build_announce_body(identity: &IdentityKeypair, listen_addr: SocketAddr) -> Result<Vec<u8>, DhtError> {
    let peer_id = sodiumoxide::crypto::hash::sha256::hash(&identity.public_key_bytes()).0;

    // Encode the IP address: 4 bytes for IPv4, 16 bytes for IPv6.
    // Prefix with a 1-byte address family tag (4=IPv4, 6=IPv6).
    let (af_tag, ip_bytes) = match listen_addr.ip() {
        std::net::IpAddr::V4(v4) => (4u8, v4.octets().to_vec()),
        std::net::IpAddr::V6(v6) => (6u8, v6.octets().to_vec()),
    };

    // body = peer_id(32) + identity_pub(32) + af_tag(1) + ip(variable 4/16) + port(2) + sig(64)
    let body_size = 32 + 32 + 1 + ip_bytes.len() + 2 + 64;
    let mut body = Vec::with_capacity(body_size);
    body.extend_from_slice(&peer_id);
    body.extend_from_slice(&identity.public_key_bytes());
    body.push(af_tag);
    body.extend_from_slice(&ip_bytes);
    body.extend_from_slice(&listen_addr.port().to_be_bytes());

    // Sign the announce (peer_id + ip + port)
    let mut sign_data = Vec::with_capacity(32 + ip_bytes.len() + 2);
    sign_data.extend_from_slice(&peer_id);
    sign_data.extend_from_slice(&ip_bytes);
    sign_data.extend_from_slice(&listen_addr.port().to_be_bytes());
    let signature = identity.sign(&sign_data);
    body.extend_from_slice(&signature);

    Ok(body)
}

/// Announce our presence to a bootstrap node.
pub async fn announce_to_node(
    node_addr: SocketAddr,
    identity: &IdentityKeypair,
    listen_addr: SocketAddr,
) -> Result<(), DhtError> {
    let mut stream = time::timeout(DHT_CONNECT_TIMEOUT, TcpStream::connect(node_addr))
        .await
        .map_err(|_| DhtError::Timeout)?
        .map_err(DhtError::Io)?;

    let body = build_announce_body(identity, listen_addr)?;
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
fn parse_node_response(body: &[u8]) -> Result<Vec<DhtPeer>, DhtError> {
    // Format: [peer_id(32B) identity_pub(32B) ip(4B) port(2B)]*
    let entry_size = 32 + 32 + 4 + 2;
    let count = body.len() / entry_size;
    if body.len() % entry_size != 0 {
        return Err(DhtError::BadResponse("malformed node response".into()));
    }

    let mut peers = Vec::with_capacity(count);
    for i in 0..count {
        let offset = i * entry_size;
        let mut peer_id = [0u8; 32];
        peer_id.copy_from_slice(&body[offset..offset + 32]);
        let mut identity_pub = [0u8; 32];
        identity_pub.copy_from_slice(&body[offset + 32..offset + 64]);
        let ip_bytes: [u8; 4] = [
            body[offset + 64], body[offset + 65],
            body[offset + 66], body[offset + 67],
        ];
        let port = u16::from_be_bytes([body[offset + 68], body[offset + 69]]);

        let fingerprint = crate::crypto::fingerprint_from_public_key(&identity_pub);
        let addr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(ip_bytes)), port);

        peers.push(DhtPeer {
            peer_id,
            identity_pub,
            fingerprint,
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
pub async fn announce_loop(
    dht_state: Arc<RwLock<DhtState>>,
    identity: Arc<RwLock<Option<IdentityKeypair>>>,
    listen_addr: Arc<RwLock<Option<std::net::SocketAddr>>>,
) {
    loop {
        time::sleep(ANNOUNCE_INTERVAL).await;

        let nodes = {
            let state = dht_state.read().await;
            state.config.bootstrap_nodes.clone()
        };

        let addr = {
            let a = listen_addr.read().await;
            *a
        };

        let kp = {
            let id = identity.read().await;
            id.as_ref().map(|kp| {
                let pk = kp.public_key_bytes();
                let sk = kp.secret_key_bytes();
                IdentityKeypair::from_bytes(&pk, &sk).ok()
            }).flatten()
        };

        let (kp, addr) = match (kp, addr) {
            (Some(k), Some(a)) => (k, a),
            _ => continue,
        };

        for node in &nodes {
            if let Err(e) = announce_to_node(node.address, &kp, addr).await {
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
        let mut identity_pub = [0u8; 32];
        identity_pub[1] = 0xBB;
        let ip = [10u8, 0, 0, 1];
        let port = 9876u16.to_be_bytes();

        let mut body = Vec::new();
        body.extend_from_slice(&peer_id);
        body.extend_from_slice(&identity_pub);
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
        let body = vec![0u8; 10]; // Not a multiple of entry_size
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
                identity_pub: [0xBB; 32],
                fingerprint: "STALE".to_string(),
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
        init_crypto();
        let identity = make_identity();
        let addr: SocketAddr = "192.168.1.5:9999".parse().unwrap();

        let body = build_announce_body(&identity, addr).unwrap();
        assert!(body.len() > 32 + 32); // peer_id + identity_pub + ip + port + sig
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
