/// M2M — STUN Module
///
/// Lightweight STUN client for NAT traversal.
/// Sends a STUN Binding Request to a public STUN server to discover
/// the user's public IP:port as seen by the internet.
///
/// This is used to generate invite links that work across the internet,
/// not just on local networks. The STUN protocol is minimal and does
/// not leak any sensitive data — it only reveals the public IP which
/// is already visible to the network anyway.
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

use thiserror::Error;

/// Default STUN servers (Google and Cloudflare — well-known, reliable).
const STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun.cloudflare.com:3478",
];

/// STUN request timeout.
const STUN_TIMEOUT: Duration = Duration::from_secs(5);

/// Magic cookie as defined in RFC 5389.
const STUN_MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN Binding Request message type.
const BINDING_REQUEST: u16 = 0x0001;

/// STUN Binding Response (success).
const BINDING_RESPONSE: u16 = 0x0101;

/// XOR-MAPPED-ADDRESS attribute type.
const XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// MAPPED-ADDRESS attribute type (fallback).
const MAPPED_ADDRESS: u16 = 0x0001;

#[derive(Debug, Error)]
pub enum StunError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("STUN request timed out")]
    Timeout,
    #[error("invalid STUN response")]
    InvalidResponse,
    #[error("no XOR-MAPPED-ADDRESS in response")]
    NoMappedAddress,
    #[error("all STUN servers failed")]
    AllServersFailed,
    #[error("DNS resolution failed: {0}")]
    DnsError(String),
}

/// Result of a STUN binding request.
#[derive(Debug, Clone)]
pub struct StunResult {
    /// The public IP:port as seen by the STUN server.
    pub public_addr: SocketAddr,
    /// The STUN server that responded.
    pub server: String,
}

/// Discover the public IP address by querying STUN servers.
/// Tries each server in order until one succeeds.
pub async fn discover_public_addr() -> Result<StunResult, StunError> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    for server_str in STUN_SERVERS {
        match try_stun_server(&socket, server_str).await {
            Ok(addr) => {
                tracing::info!(public_addr = %addr, server = server_str, "STUN discovery succeeded");
                return Ok(StunResult {
                    public_addr: addr,
                    server: server_str.to_string(),
                });
            }
            Err(e) => {
                tracing::debug!(server = server_str, error = %e, "STUN server failed, trying next");
            }
        }
    }

    Err(StunError::AllServersFailed)
}

/// Send a STUN Binding Request to a single server and parse the response.
async fn try_stun_server(socket: &UdpSocket, server: &str) -> Result<SocketAddr, StunError> {
    // Resolve the server address
    let addr: SocketAddr = tokio::net::lookup_host(server)
        .await
        .map_err(|e| StunError::DnsError(e.to_string()))?
        .next()
        .ok_or_else(|| StunError::DnsError("no addresses found".to_string()))?;

    // Build the STUN Binding Request (RFC 5389 minimal)
    let transaction_id: [u8; 12] = rand::random();
    let request = build_binding_request(&transaction_id);

    // Send
    socket.send_to(&request, addr).await?;

    // Receive with timeout
    let mut buf = [0u8; 576]; // STUN responses are small
    let (len, _from) = timeout(STUN_TIMEOUT, socket.recv_from(&mut buf))
        .await
        .map_err(|_| StunError::Timeout)?
        .map_err(StunError::Io)?;

    // Parse response
    parse_binding_response(&buf[..len], &transaction_id)
}

/// Build a minimal STUN Binding Request.
fn build_binding_request(transaction_id: &[u8; 12]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(20);
    // Message type: Binding Request
    pkt.extend_from_slice(&BINDING_REQUEST.to_be_bytes());
    // Message length: 0 (no attributes)
    pkt.extend_from_slice(&0u16.to_be_bytes());
    // Magic cookie
    pkt.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    // Transaction ID (12 bytes)
    pkt.extend_from_slice(transaction_id);
    pkt
}

/// Parse a STUN Binding Response and extract the XOR-MAPPED-ADDRESS.
fn parse_binding_response(data: &[u8], expected_txn: &[u8; 12]) -> Result<SocketAddr, StunError> {
    if data.len() < 20 {
        return Err(StunError::InvalidResponse);
    }

    // Verify message type
    let msg_type = u16::from_be_bytes([data[0], data[1]]);
    if msg_type != BINDING_RESPONSE {
        return Err(StunError::InvalidResponse);
    }

    // Verify magic cookie
    let cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if cookie != STUN_MAGIC_COOKIE {
        return Err(StunError::InvalidResponse);
    }

    // Verify transaction ID
    if &data[8..20] != expected_txn {
        return Err(StunError::InvalidResponse);
    }

    let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
    let attrs = &data[20..20 + msg_len.min(data.len() - 20)];

    // Parse attributes looking for XOR-MAPPED-ADDRESS or MAPPED-ADDRESS
    let mut offset = 0;
    while offset + 4 <= attrs.len() {
        let attr_type = u16::from_be_bytes([attrs[offset], attrs[offset + 1]]);
        let attr_len = u16::from_be_bytes([attrs[offset + 2], attrs[offset + 3]]) as usize;
        let attr_data = &attrs[offset + 4..offset + 4 + attr_len.min(attrs.len() - offset - 4)];

        if attr_type == XOR_MAPPED_ADDRESS {
            return parse_xor_mapped_address(attr_data);
        }
        if attr_type == MAPPED_ADDRESS {
            return parse_mapped_address(attr_data);
        }

        // Attributes are padded to 4-byte boundaries
        offset += 4 + ((attr_len + 3) & !3);
    }

    Err(StunError::NoMappedAddress)
}

/// Parse an XOR-MAPPED-ADDRESS attribute (RFC 5389 §15.2).
fn parse_xor_mapped_address(data: &[u8]) -> Result<SocketAddr, StunError> {
    if data.len() < 8 {
        return Err(StunError::InvalidResponse);
    }

    let family = data[1];
    let x_port = u16::from_be_bytes([data[2], data[3]]) ^ (STUN_MAGIC_COOKIE >> 16) as u16;

    match family {
        0x01 => {
            // IPv4
            let x_addr = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) ^ STUN_MAGIC_COOKIE;
            let ip = std::net::Ipv4Addr::from(x_addr);
            Ok(SocketAddr::new(std::net::IpAddr::V4(ip), x_port))
        }
        0x02 => {
            // IPv6 (XOR with magic cookie + transaction ID — simplified)
            if data.len() < 20 {
                return Err(StunError::InvalidResponse);
            }
            // For simplicity, just extract the basic IPv6 case
            let mut addr_bytes = [0u8; 16];
            addr_bytes.copy_from_slice(&data[4..20]);
            // XOR with magic cookie (first 4 bytes) — simplified
            let cookie_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
            for i in 0..4 {
                addr_bytes[i] ^= cookie_bytes[i];
            }
            let ip = std::net::Ipv6Addr::from(addr_bytes);
            Ok(SocketAddr::new(std::net::IpAddr::V6(ip), x_port))
        }
        _ => Err(StunError::InvalidResponse),
    }
}

/// Parse a plain MAPPED-ADDRESS attribute (fallback for older servers).
fn parse_mapped_address(data: &[u8]) -> Result<SocketAddr, StunError> {
    if data.len() < 8 {
        return Err(StunError::InvalidResponse);
    }

    let family = data[1];
    let port = u16::from_be_bytes([data[2], data[3]]);

    match family {
        0x01 => {
            let ip = std::net::Ipv4Addr::new(data[4], data[5], data[6], data[7]);
            Ok(SocketAddr::new(std::net::IpAddr::V4(ip), port))
        }
        _ => Err(StunError::InvalidResponse),
    }
}
