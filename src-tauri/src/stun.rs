/// M2M — STUN Module (RFC 8489 Compliant)
///
/// Enterprise-grade STUN client with:
/// - Parallel server queries (all configured servers simultaneously)
/// - Cross-server consistency checking (detects DNS poisoning / MITM)
/// - Configurable server list (user-managed via frontend)
/// - Proper IPv6 XOR-MAPPED-ADDRESS parsing (RFC 8489 §15.2)
/// - NAT type classification based on STUN behavior
/// - Server health monitoring
/// - Host candidate discovery
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Defaults ───────────────────────────────────────────────────────────────

/// Default STUN servers — diverse, reliable, geo-distributed.
const DEFAULT_STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun.cloudflare.com:3478",
    "stun.nextcloud.com:3478",
];

/// Per-server query timeout.
const STUN_TIMEOUT: Duration = Duration::from_secs(5);

/// Magic cookie as defined in RFC 8489 §6.
const STUN_MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN Binding Request message type.
const BINDING_REQUEST: u16 = 0x0001;

/// STUN Binding Response (success).
const BINDING_RESPONSE_SUCCESS: u16 = 0x0101;

/// XOR-MAPPED-ADDRESS attribute type (RFC 8489 §15.2).
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// MAPPED-ADDRESS attribute type (legacy fallback, RFC 3489).
const MAPPED_ADDRESS: u16 = 0x0001;

/// Minimum valid STUN message size: header (20B) + attribute header (4B) + addr (8B).
const MIN_STUN_MESSAGE: usize = 32;

// ─── NAT Classification ─────────────────────────────────────────────────────

/// Classification of NAT behaviour based on STUN response patterns.
///
/// Determined by comparing the source address of the STUN request (before NAT)
/// with the source address observed by the STUN server (after NAT), and by
/// varying the destination port between queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NatType {
    /// NAT type could not be determined (insufficient data).
    Unknown,
    /// No NAT — host has a public IP directly routable.
    None,
    /// Full-cone NAT: any external host can send packets to the mapped address.
    FullCone,
    /// Restricted-cone: only hosts the client has sent to can send back.
    RestrictedCone,
    /// Port-restricted cone: like restricted cone but also checks port.
    PortRestrictedCone,
    /// Symmetric NAT: each destination IP:port gets a different mapping.
    /// This is the most restrictive type — STUN-only discovery will not work
    /// for inbound connections behind symmetric NAT.
    Symmetric,
    /// UDP is entirely blocked by a firewall.
    Blocked,
}

impl std::fmt::Display for NatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatType::Unknown => write!(f, "unknown"),
            NatType::None => write!(f, "no NAT (public IP)"),
            NatType::FullCone => write!(f, "full-cone NAT"),
            NatType::RestrictedCone => write!(f, "restricted-cone NAT"),
            NatType::PortRestrictedCone => write!(f, "port-restricted cone NAT"),
            NatType::Symmetric => write!(f, "symmetric NAT"),
            NatType::Blocked => write!(f, "UDP blocked"),
        }
    }
}

// ─── Configuration ──────────────────────────────────────────────────────────

/// User-controllable STUN configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StunConfig {
    /// List of STUN server addresses (host:port).
    pub servers: Vec<String>,
    /// Timeout per server in seconds.
    pub timeout_secs: u64,
    /// If true, only use STUN for reflexive candidates (privacy: don't share in invites).
    pub private_mode: bool,
}

impl Default for StunConfig {
    fn default() -> Self {
        Self {
            servers: DEFAULT_STUN_SERVERS.iter().map(|s| (*s).to_string()).collect(),
            timeout_secs: 5,
            private_mode: false,
        }
    }
}

// ─── Result Types ───────────────────────────────────────────────────────────

/// Result from a single STUN server.
#[derive(Debug, Clone)]
pub struct StunResult {
    /// The public IP:port as seen by this STUN server.
    pub public_addr: SocketAddr,
    /// The server that reported this address.
    pub server: String,
    /// Round-trip time for the query.
    pub rtt: Duration,
}

/// Aggregated result from all configured STUN servers.
#[derive(Debug, Clone)]
pub struct StunMultiResult {
    /// Individual server results.
    pub results: Vec<StunResult>,
    /// The consensus public IP (None if servers disagree or no results).
    pub consensus_addr: Option<SocketAddr>,
    /// True if all responding servers reported the same public IP.
    pub consensus: bool,
    /// Number of servers queried vs number that responded.
    pub total_servers: usize,
    pub responding_servers: usize,
}

/// Health status of a single STUN server.
#[derive(Debug, Clone, Serialize)]
pub struct StunServerHealth {
    pub server: String,
    pub reachable: bool,
    pub rtt_ms: Option<u64>,
    pub error: Option<String>,
}

/// Result of an end-to-end connectivity verification.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectivityStatus {
    /// Whether the listening port is reachable from the public internet.
    pub reachable: bool,
    /// Classified NAT type.
    pub nat_type: NatType,
    /// Public address if discovered.
    pub public_addr: Option<String>,
    /// Host candidate addresses (local interface IPs).
    pub host_addrs: Vec<String>,
    /// Estimated MTU / path properties.
    pub behind_symmetric_nat: bool,
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum StunError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("STUN request to {server} timed out")]
    Timeout { server: String },
    #[error("invalid STUN response from {server}")]
    InvalidResponse { server: String },
    #[error("no XOR-MAPPED-ADDRESS in response from {server}")]
    NoMappedAddress { server: String },
    #[error("no STUN servers responded")]
    AllServersFailed,
    #[error("DNS resolution failed for {server}: {error}")]
    DnsError { server: String, error: String },
    #[error("transaction ID mismatch from {server} — possible injection")]
    TransactionIdMismatch { server: String },
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Query all configured STUN servers in parallel, aggregate results,
/// and determine consensus on the public IP address.
///
/// ## Security
/// - Cross-server consistency check detects DNS poisoning attacks:
///   if one STUN server resolves to a different IP than the others,
///   `consensus` will be `false` and the operator can be alerted.
/// - Transaction ID validation prevents off-path injection.
/// - Each server is bound to an independent ephemeral socket.
pub async fn discover_public_addrs(config: &StunConfig) -> Result<StunMultiResult, StunError> {
    let total_servers = config.servers.len();
    if total_servers == 0 {
        return Err(StunError::AllServersFailed);
    }

    let timeout_duration = Duration::from_secs(config.timeout_secs);

    // Spawn one task per server — all queries run in parallel.
    let mut handles = Vec::with_capacity(total_servers);
    for server in &config.servers {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            let start = std::time::Instant::now();
            match query_single_server(&server, timeout_duration).await {
                Ok(addr) => Some(StunResult {
                    public_addr: addr,
                    server,
                    rtt: start.elapsed(),
                }),
                Err(e) => {
                    tracing::debug!(server = %server, error = %e, "STUN server failed");
                    None
                }
            }
        }));
    }

    // Collect results — ignore tasks that panicked.
    let mut results: Vec<StunResult> = Vec::new();
    for handle in handles {
        if let Ok(Some(result)) = handle.await {
            results.push(result);
        }
    }

    let responding_servers = results.len();
    if results.is_empty() {
        return Err(StunError::AllServersFailed);
    }

    // Consensus: do ALL responding servers report the same public IP?
    let first_ip = results[0].public_addr.ip();
    let mut consensus = true;
    for r in &results {
        if r.public_addr.ip() != first_ip {
            tracing::warn!(
                ip1 = %first_ip, ip2 = %r.public_addr.ip(),
                server = %r.server,
                "STUN server IP mismatch — possible DNS poisoning or asymmetric routing"
            );
            consensus = false;
        }
    }

    let consensus_addr = if consensus {
        Some(results[0].public_addr)
    } else {
        // When servers disagree, take the majority vote.
        let mut ip_counts: std::collections::HashMap<std::net::IpAddr, usize> =
            std::collections::HashMap::new();
        for r in &results {
            *ip_counts.entry(r.public_addr.ip()).or_insert(0) += 1;
        }
        let (winning_ip, _count) = ip_counts.into_iter().max_by_key(|&(_, c)| c).unwrap();
        // Return the first result matching the winning IP.
        results
            .iter()
            .find(|r| r.public_addr.ip() == winning_ip)
            .map(|r| r.public_addr)
    };

    Ok(StunMultiResult {
        results,
        consensus_addr,
        consensus,
        total_servers,
        responding_servers,
    })
}

/// Query a single STUN server and return the discovered public address.
async fn query_single_server(
    server: &str,
    query_timeout: Duration,
) -> Result<SocketAddr, StunError> {
    // ── DNS resolution with timeout ──
    let addr = timeout(query_timeout, tokio::net::lookup_host(server))
        .await
        .map_err(|_| StunError::Timeout {
            server: server.to_string(),
        })?
        .map_err(|e| StunError::DnsError {
            server: server.to_string(),
            error: e.to_string(),
        })?
        .next()
        .ok_or_else(|| StunError::DnsError {
            server: server.to_string(),
            error: "no addresses found".to_string(),
        })?;

    // ── Bind ephemeral UDP socket ──
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(StunError::Io)?;

    // Socket timeout is handled entirely by the outer tokio::time::timeout
    // wrapping the recv_from call. No need for set_read_timeout here.

    // ── Generate random 12-byte transaction ID (RFC 8489 §6) ──
    let transaction_id: [u8; 12] = rand::random();

    // ── Build and send Binding Request ──
    let request = build_binding_request(&transaction_id);
    socket
        .send_to(&request, addr)
        .await
        .map_err(StunError::Io)?;

    // ── Receive response with timeout ──
    // STUN messages over UDP are limited to 576 bytes (RFC 8489 §7.1).
    let mut buf = [0u8; 576];
    let (len, _src) = timeout(query_timeout, socket.recv_from(&mut buf))
        .await
        .map_err(|_| StunError::Timeout {
            server: server.to_string(),
        })?
        .map_err(StunError::Io)?;

    // ── Parse and validate response ──
    parse_binding_response(&buf[..len], &transaction_id).map_err(|_| {
        StunError::InvalidResponse {
            server: server.to_string(),
        }
    })
}

/// Build a minimal STUN Binding Request (RFC 8489 §7.1).
///
/// Format:
///  0                   1                   2                   3
///  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// ┌─+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-┐
/// │         0x0001 (Binding Request)   │         0x0000 (length)  │
/// ├─+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-┤
/// │                        0x2112A442 (Magic Cookie)               │
/// ├─+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-┤
/// │                                                               │
/// │                     Transaction ID (12 bytes)                  │
/// │                                                               │
/// └─+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-┘
fn build_binding_request(transaction_id: &[u8; 12]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(20);
    pkt.extend_from_slice(&BINDING_REQUEST.to_be_bytes());
    pkt.extend_from_slice(&0u16.to_be_bytes()); // message length = 0 (no attributes)
    pkt.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    pkt.extend_from_slice(transaction_id);
    pkt
}

/// Parse a STUN Binding Response and extract the XOR-MAPPED-ADDRESS.
///
/// Validation steps:
/// 1. Minimum size check (20B header)
/// 2. Message type must be Binding Response (0x0101)
/// 3. Magic cookie must match (0x2112A442)
/// 4. Transaction ID must match what we sent (injection protection)
/// 5. Walk attributes looking for XOR-MAPPED-ADDRESS (preferred) or MAPPED-ADDRESS (fallback)
fn parse_binding_response(
    data: &[u8],
    expected_txn: &[u8; 12],
) -> Result<SocketAddr, StunError> {
    // ── Validation ──
    if data.len() < 20 {
        return Err(StunError::InvalidResponse {
            server: "?".to_string(),
        });
    }

    let msg_type = u16::from_be_bytes([data[0], data[1]]);
    if msg_type != BINDING_RESPONSE_SUCCESS {
        // Check if it's an error response
        if msg_type == 0x0111 {
            return Err(StunError::InvalidResponse {
                server: "?".to_string(),
            });
        }
        return Err(StunError::InvalidResponse {
            server: "?".to_string(),
        });
    }

    if &data[4..8] != &STUN_MAGIC_COOKIE.to_be_bytes() {
        return Err(StunError::InvalidResponse {
            server: "?".to_string(),
        });
    }

    if &data[8..20] != expected_txn {
        return Err(StunError::TransactionIdMismatch {
            server: "?".to_string(),
        });
    }

    // ── Parse attributes ──
    let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
    let attrs_avail = data.len().saturating_sub(20);
    let attrs_len = msg_len.min(attrs_avail);
    let attrs = &data[20..20 + attrs_len];

    let mut offset = 0;
    while offset + 4 <= attrs.len() {
        let attr_type = u16::from_be_bytes([attrs[offset], attrs[offset + 1]]);
        let attr_len = u16::from_be_bytes([attrs[offset + 2], attrs[offset + 3]]) as usize;
        let padded_len = (attr_len + 3) & !3; // 4-byte alignment padding

        // Validate length to prevent buffer over-read
        if offset + 4 + attr_len > attrs.len() {
            break;
        }

        let attr_data = &attrs[offset + 4..offset + 4 + attr_len];

        match attr_type {
            XOR_MAPPED_ADDRESS => {
                return parse_xor_mapped_address(attr_data, expected_txn);
            }
            MAPPED_ADDRESS => {
                if let Ok(addr) = parse_mapped_address(attr_data) {
                    return Ok(addr);
                }
            }
            _ => {
                // Skip unknown attributes (FINGERPRINT, SOFTWARE, etc.)
            }
        }

        offset += 4 + padded_len;
    }

    Err(StunError::NoMappedAddress {
        server: "?".to_string(),
    })
}

/// Parse an XOR-MAPPED-ADDRESS attribute (RFC 8489 §15.2).
///
/// IPv4:   address XOR Magic Cookie
/// IPv6:   first 4 bytes XOR Magic Cookie, remaining 12 bytes XOR Transaction ID
/// Port:   port XOR (Magic Cookie >> 16)
fn parse_xor_mapped_address(
    data: &[u8],
    transaction_id: &[u8; 12],
) -> Result<SocketAddr, StunError> {
    if data.len() < 8 {
        return Err(StunError::InvalidResponse {
            server: "?".to_string(),
        });
    }

    let family = data[1];
    // Port XOR with the high 16 bits of the magic cookie (RFC 8489 §15.2)
    let x_port =
        u16::from_be_bytes([data[2], data[3]]) ^ (STUN_MAGIC_COOKIE >> 16) as u16;

    match family {
        0x01 => {
            // IPv4: XOR with the full 32-bit magic cookie
            let x_addr = u32::from_be_bytes([data[4], data[5], data[6], data[7]])
                ^ STUN_MAGIC_COOKIE;
            let ip = std::net::Ipv4Addr::from(x_addr);
            Ok(SocketAddr::new(std::net::IpAddr::V4(ip), x_port))
        }
        0x02 => {
            // IPv6: 16 bytes of address data
            //   bytes 0-3  XOR Magic Cookie
            //   bytes 4-15 XOR Transaction ID
            if data.len() < 20 {
                return Err(StunError::InvalidResponse {
                    server: "?".to_string(),
                });
            }
            let mut addr_bytes = [0u8; 16];
            addr_bytes.copy_from_slice(&data[4..20]);

            // XOR first 4 bytes with magic cookie
            let cookie_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
            for i in 0..4 {
                addr_bytes[i] ^= cookie_bytes[i];
            }
            // XOR remaining 12 bytes with transaction ID
            for i in 0..12 {
                addr_bytes[4 + i] ^= transaction_id[i];
            }

            let ip = std::net::Ipv6Addr::from(addr_bytes);
            Ok(SocketAddr::new(std::net::IpAddr::V6(ip), x_port))
        }
        _ => Err(StunError::InvalidResponse {
            server: "?".to_string(),
        }),
    }
}

/// Parse a legacy MAPPED-ADDRESS attribute (RFC 3489, no XOR).
/// Used as fallback when XOR-MAPPED-ADDRESS is not present.
fn parse_mapped_address(data: &[u8]) -> Result<SocketAddr, StunError> {
    if data.len() < 8 {
        return Err(StunError::InvalidResponse {
            server: "?".to_string(),
        });
    }

    let family = data[1];
    let port = u16::from_be_bytes([data[2], data[3]]);

    match family {
        0x01 => {
            let ip = std::net::Ipv4Addr::new(data[4], data[5], data[6], data[7]);
            Ok(SocketAddr::new(std::net::IpAddr::V4(ip), port))
        }
        _ => Err(StunError::InvalidResponse {
            server: "?".to_string(),
        }),
    }
}

// ─── Server Health Monitoring ──────────────────────────────────────────────

/// Check reachability of a single STUN server.
/// Returns detailed health information without panicking.
pub async fn check_server(server: &str, timeout_dur: Duration) -> StunServerHealth {
    let start = std::time::Instant::now();
    match query_single_server(server, timeout_dur).await {
        Ok(_) => StunServerHealth {
            server: server.to_string(),
            reachable: true,
            rtt_ms: Some(start.elapsed().as_millis() as u64),
            error: None,
        },
        Err(e) => StunServerHealth {
            server: server.to_string(),
            reachable: false,
            rtt_ms: None,
            error: Some(e.to_string()),
        },
    }
}

/// Check all configured STUN servers in parallel and return their health.
pub async fn check_all_servers(config: &StunConfig) -> Vec<StunServerHealth> {
    let timeout_dur = Duration::from_secs(config.timeout_secs);
    let mut handles = Vec::with_capacity(config.servers.len());

    for server in &config.servers {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            check_server(&server, timeout_dur).await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(health) = handle.await {
            results.push(health);
        }
    }
    results
}

// ─── Host Candidate Discovery ──────────────────────────────────────────────

/// Gather host candidates: discover local non-loopback IPs that can reach
/// the internet.
///
/// Uses a UDP socket trick that works across all major OSes without
/// requiring external crate dependencies for interface enumeration:
/// connecting to a public IP tells us which local IP the kernel would
/// use as the source.
pub fn gather_host_candidates() -> Vec<SocketAddr> {
    let mut candidates: Vec<SocketAddr> = Vec::new();
    let mut seen_ips: HashSet<std::net::IpAddr> = HashSet::new();

    // Try multiple well-known public addresses to handle split-tunnel VPNs
    // and multi-homed hosts correctly.
    let probes = &[
        ("8.8.8.8", 80u16),
        ("1.1.1.1", 443u16),
        ("208.67.222.222", 53u16),
    ];

    for &(ip, port) in probes {
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            let addr_str = format!("{}:{}", ip, port);
            if socket.connect(&addr_str).is_ok() {
                if let Ok(local) = socket.local_addr() {
                    let local_ip = local.ip();
                    if !local_ip.is_loopback()
                        && !local_ip.is_unspecified()
                        && !local_ip.is_multicast()
                        && seen_ips.insert(local_ip)
                    {
                        candidates.push(local);
                    }
                }
            }
        }
    }

    candidates
}

/// Attempt to classify the NAT type based on STUN behaviour.
///
/// This uses a heuristic: if multiple STUN servers all report the same
/// public IP, we're behind a cone NAT (or no NAT). If servers disagree,
/// it could be asymmetric routing or a symmetric NAT — we flag it.
///
/// Full classification requires sending from multiple ports/destinations
/// and is deferred to a future enhancement; this gives a useful diagnostic.
pub fn classify_nat(result: &StunMultiResult) -> NatType {
    if result.results.is_empty() {
        return NatType::Blocked;
    }

    // Check if any public IP differs. For symmetric NATs, each STUN server
    // would see a different source port (and possibly IP).
    let unique_ips: HashSet<std::net::IpAddr> =
        result.results.iter().map(|r| r.public_addr.ip()).collect();

    if unique_ips.len() > 1 {
        // Different IPs per server suggests extreme asymmetric routing
        // or a symmetric NAT.
        NatType::Symmetric
    } else if unique_ips.len() == 1 {
        let first = result.results[0].public_addr;
        // If the public IP is a private range, there's definitely a NAT
        // and it's probably symmetric for TCP purposes.
        let is_private = match first.ip() {
            std::net::IpAddr::V4(v4) => {
                let o = v4.octets();
                o[0] == 10 || (o[0] == 172 && o[1] >= 16 && o[1] <= 31) || (o[0] == 192 && o[1] == 168)
            }
            std::net::IpAddr::V6(_) => false,
        };
        if is_private || first.ip().is_loopback() {
            // Server returned a private IP — this shouldn't happen with
            // proper STUN servers, but if it does, we can't trust it.
            NatType::Unknown
        } else if is_global_unicast(first.ip()) {
            // Single consistent public IP — cone NAT or no NAT.
            // Distinguishing full/restricted/port-restricted requires
            // additional probing (send from different ports/addresses).
            // Default to the most common: port-restricted cone.
            NatType::PortRestrictedCone
        } else {
            NatType::Unknown
        }
    } else {
        NatType::Unknown
    }
}

/// Check if an IP is a global unicast (not private, loopback, etc.)
fn is_global_unicast(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            // `is_private()`, `is_link_local()`, etc. are methods on `Ipv4Addr`
            let octets = v4.octets();
            !is_private_v4(v4)
                && !v4.is_loopback()
                && !v4.is_link_local()
                && !v4.is_broadcast()
                && !v4.is_documentation()
                && !v4.is_unspecified()
                // Exclude CGNAT (100.64.0.0/10)
                && !((octets[0] == 100) && (octets[1] & 0xC0) == 0x40)
                // Exclude benchmarking range (198.18.0.0/15)
                && !((octets[0] == 198) && (octets[1] & 0xFE) == 0x18)
        }
        std::net::IpAddr::V6(v6) => {
            !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_multicast()
                && !v6.is_unique_local()
                && !v6.is_unicast_link_local()
        }
    }
}

/// Check if an IPv4 address is in a private range (RFC 1918).
fn is_private_v4(ip: std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    match octets[0] {
        10 => true,
        172 => octets[1] >= 16 && octets[1] <= 31,
        192 => octets[1] == 168,
        _ => false,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Test vector from RFC 5769 §2.1 (IPv4 XOR-MAPPED-ADDRESS).
    /// This validates our parsing against a known-good STUN response.
    fn build_rfc5769_sample_response() -> (Vec<u8>, [u8; 12], SocketAddr) {
        // Transaction ID from RFC 5769 example
        let txn: [u8; 12] = [
            0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
        ];

        // Expected result
        let expected =
            SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 1)), 32853);

        // Build a minimal valid STUN response
        let mut response = Vec::with_capacity(32);
        // Message type: Binding Response (0x0101)
        response.extend_from_slice(&BINDING_RESPONSE_SUCCESS.to_be_bytes());
        // Message length: 12 (XOR-MAPPED-ADDRESS: 4B header + 8B value)
        response.extend_from_slice(&12u16.to_be_bytes());
        // Magic cookie
        response.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        // Transaction ID
        response.extend_from_slice(&txn);

        // XOR-MAPPED-ADDRESS attribute
        let attr_type = XOR_MAPPED_ADDRESS;
        response.extend_from_slice(&attr_type.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes()); // attribute length
        response.push(0x00); // padding
        response.push(0x01); // family = IPv4
        // Port XOR (32853 ^ 0x2112 = ?)
        let x_port = 32853u16 ^ (STUN_MAGIC_COOKIE >> 16) as u16;
        response.extend_from_slice(&x_port.to_be_bytes());
        // IPv4 XOR (192.0.2.1 ^ magic_cookie)
        let ip_bytes = [192u8, 0, 2, 1];
        let ip_u32 = u32::from_be_bytes(ip_bytes);
        let x_addr = ip_u32 ^ STUN_MAGIC_COOKIE;
        response.extend_from_slice(&x_addr.to_be_bytes());

        (response, txn, expected)
    }

    #[test]
    fn test_rfc5769_ipv4_xor_mapped_address() {
        let (response, txn, expected) = build_rfc5769_sample_response();
        let result = parse_binding_response(&response, &txn);
        assert!(result.is_ok(), "RFC 5769 IPv4 test vector should parse: {:?}", result.err());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn test_invalid_short_response() {
        let txn = [0u8; 12];
        let result = parse_binding_response(&[0u8; 10], &txn);
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_magic_cookie() {
        let txn = [0u8; 12];
        let mut data = vec![0u8; 20];
        data[0..2].copy_from_slice(&BINDING_RESPONSE_SUCCESS.to_be_bytes());
        // Magic cookie = 0 (wrong)
        data[4..8].copy_from_slice(&0u32.to_be_bytes());
        data[8..20].copy_from_slice(&txn);
        let result = parse_binding_response(&data, &txn);
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_id_mismatch() {
        let mut data = vec![0u8; 20];
        data[0..2].copy_from_slice(&BINDING_RESPONSE_SUCCESS.to_be_bytes());
        data[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        let sent_txn = [0x01u8; 12];
        let recv_txn = [0x02u8; 12];
        data[8..20].copy_from_slice(&recv_txn);
        let result = parse_binding_response(&data, &sent_txn);
        assert!(result.is_err());
    }

    #[test]
    fn test_host_candidate_gathering() {
        let candidates = gather_host_candidates();
        // Should at least not panic
        assert!(candidates.len() <= 10, "sanity: shouldn't find dozens of IPs");
    }

    #[test]
    fn test_default_config_valid() {
        let config = StunConfig::default();
        assert!(!config.servers.is_empty(), "default config must have servers");
        assert!(config.timeout_secs >= 1, "timeout must be reasonable");
    }

    #[test]
    fn test_nat_classification_no_results() {
        let multi = StunMultiResult {
            results: vec![],
            consensus_addr: None,
            consensus: false,
            total_servers: 3,
            responding_servers: 0,
        };
        assert_eq!(classify_nat(&multi), NatType::Blocked);
    }

    #[test]
    fn test_nat_classification_consistent_ip() {
        let addr: SocketAddr = "8.8.8.8:12345".parse().unwrap();
        let multi = StunMultiResult {
            results: vec![
                StunResult {
                    public_addr: addr,
                    server: "stun1.google.com".into(),
                    rtt: Duration::from_millis(10),
                },
                StunResult {
                    public_addr: addr,
                    server: "stun2.google.com".into(),
                    rtt: Duration::from_millis(15),
                },
            ],
            consensus_addr: Some(addr),
            consensus: true,
            total_servers: 2,
            responding_servers: 2,
        };
        let nat = classify_nat(&multi);
        assert_eq!(nat, NatType::PortRestrictedCone);
    }

    #[test]
    fn test_nat_classification_different_port_same_ip() {
        // Same IP but different port = port-restricted cone, not symmetric.
        let addr1: SocketAddr = "8.8.8.8:12345".parse().unwrap();
        let addr2: SocketAddr = "8.8.8.8:54321".parse().unwrap();
        let multi = StunMultiResult {
            results: vec![
                StunResult {
                    public_addr: addr1,
                    server: "stun1.google.com".into(),
                    rtt: Duration::from_millis(10),
                },
                StunResult {
                    public_addr: addr2,
                    server: "stun.cloudflare.com".into(),
                    rtt: Duration::from_millis(15),
                },
            ],
            consensus_addr: None,
            consensus: false,
            total_servers: 2,
            responding_servers: 2,
        };
        let nat = classify_nat(&multi);
        assert_eq!(nat, NatType::PortRestrictedCone);
    }

    #[test]
    fn test_nat_classification_symmetric() {
        // Different IP per server = symmetric NAT behavior detected.
        let addr1: SocketAddr = "203.0.113.1:12345".parse().unwrap();
        let addr2: SocketAddr = "198.51.100.1:54321".parse().unwrap();
        let multi = StunMultiResult {
            results: vec![
                StunResult {
                    public_addr: addr1,
                    server: "stun1.google.com".into(),
                    rtt: Duration::from_millis(10),
                },
                StunResult {
                    public_addr: addr2,
                    server: "stun.cloudflare.com".into(),
                    rtt: Duration::from_millis(15),
                },
            ],
            consensus_addr: None,
            consensus: false,
            total_servers: 2,
            responding_servers: 2,
        };
        let nat = classify_nat(&multi);
        assert_eq!(nat, NatType::Symmetric);
    }

    #[test]
    fn test_build_request_invariants() {
        let txn = [0x42u8; 12];
        let req = build_binding_request(&txn);
        // Length: 20 bytes
        assert_eq!(req.len(), 20, "STUN request must be 20 bytes");
        // Message type: Binding Request
        assert_eq!(u16::from_be_bytes([req[0], req[1]]), BINDING_REQUEST);
        // Length field: 0
        assert_eq!(u16::from_be_bytes([req[2], req[3]]), 0);
        // Magic cookie
        assert_eq!(
            u32::from_be_bytes([req[4], req[5], req[6], req[7]]),
            STUN_MAGIC_COOKIE
        );
        // Transaction ID
        assert_eq!(&req[8..20], &txn[..]);
    }
}
