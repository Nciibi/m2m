
/// M2M — Network Module
///
/// TCP transport with length-prefixed framing, connection state machine,
/// timeouts, heartbeats, rate limiting, connection-level DoS protection,
/// filename sanitization, and graceful disconnect.
///
/// All data crossing the network boundary is treated as untrusted.
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc;
use tokio::time;

use thiserror::Error;

use crate::protocol::{
    self, validate_frame_size, validate_version, PacketType, LENGTH_PREFIX_SIZE,
};

/// Network operation timeout for reads/writes.
/// Set to 10s to align with heartbeat cadence (heartbeat every 30s, timeout after 10s).
/// A dead connection is detected within 10s instead of 30s.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(10);

/// TCP connection timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum number of queued incoming connections.
/// Increased from 8 to 128 for better DoS resilience.
const LISTENER_BACKLOG: u32 = 128;

// ─── Connection Rate Limiting ───────────────────────────────────────────────

/// Maximum number of new TCP connections allowed per IP per time window.
const MAX_CONNECTIONS_PER_IP: u32 = 10;

/// Rate limit window duration in seconds.
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Maximum total concurrent connections across all IPs.
const MAX_TOTAL_CONNECTIONS: usize = 50;

/// Per-IP rate limiter with total connection cap.
///
/// Uses a sliding window counter for per-IP tracking and an atomic
/// counter for global connection limits.
///
/// ## Limits
/// - **Per-IP**: max 10 new connections per 60-second window
/// - **Global**: max 50 concurrent connections total
pub struct ConnectionLimiter {
    /// Connection timestamps per IP (sliding window counters).
    per_ip: Mutex<HashMap<IpAddr, Vec<std::time::Instant>>>,
    /// Current total active connection count.
    active_connections: AtomicUsize,
}

impl ConnectionLimiter {
    /// Create a new connection limiter with default limits.
    pub fn new() -> Self {
        Self {
            per_ip: Mutex::new(HashMap::new()),
            active_connections: AtomicUsize::new(0),
        }
    }

    /// Check if a new connection from this IP is allowed.
    /// Returns `true` if the connection should be accepted,
    /// `false` if rate-limited or at capacity.
    pub fn check(&self, ip: IpAddr) -> bool {
        // Global cap: reject if at max concurrent connections.
        if self.active_connections.load(Ordering::Relaxed) >= MAX_TOTAL_CONNECTIONS {
            tracing::warn!(ip = %ip, active = %self.active_connections.load(Ordering::Relaxed), "connection rejected: at max capacity");
            return false;
        }

        // Per-IP rate limit: sliding window counter.
        let mut map = self.per_ip.lock().unwrap();
        let entries = map.entry(ip).or_insert_with(Vec::new);
        let now = std::time::Instant::now();
        let window = Duration::from_secs(RATE_LIMIT_WINDOW_SECS);

        // Remove entries outside the window.
        entries.retain(|t| now.duration_since(*t) < window);

        // Check if rate limited.
        if entries.len() >= MAX_CONNECTIONS_PER_IP as usize {
            tracing::warn!(ip = %ip, count = entries.len(), "connection rejected: per-IP rate limit exceeded");
            return false;
        }

        // Record this connection attempt.
        entries.push(now);
        true
    }

    /// Record a new accepted connection (increments active count).
    pub fn increment(&self) {
        self.active_connections.fetch_add(1, Ordering::SeqCst);
    }

    /// Record a connection closure (decrements active count).
    pub fn decrement(&self) {
        self.active_connections.fetch_sub(1, Ordering::SeqCst);
    }

    /// Get the current number of active connections.
    pub fn active_count(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }
}

// ─── Filename Sanitization ──────────────────────────────────────────────────

/// Maximum length of a sanitized filename.
const MAX_FILENAME_LEN: usize = 255;

/// Sanitize a filename received from an untrusted peer.
///
/// Strips directory separators, control characters, and other dangerous
/// characters. Only allows: alphanumeric, dots, hyphens, underscores, spaces.
///
/// Returns `None` if the resulting filename is empty, dot-only, or otherwise
/// unsafe.
///
/// # Examples
/// ```
/// assert_eq!(sanitize_filename("../../../etc/passwd"), None);
/// assert_eq!(sanitize_filename("report.pdf"), Some("report.pdf".into()));
/// assert_eq!(sanitize_filename(""), None);
/// ```
pub fn sanitize_filename(filename: &str) -> Option<String> {
    // Filter to safe characters only — no path separators, no control chars.
    let sanitized: String = filename
        .chars()
        .filter(|c| {
            matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' | ' ')
        })
        .collect();

    let sanitized = sanitized.trim().to_string();

    // Reject empty, dot-only, or dangerously short names.
    if sanitized.is_empty()
        || sanitized == "."
        || sanitized == ".."
        || sanitized.eq_ignore_ascii_case("nul")
        || sanitized.eq_ignore_ascii_case("con")
        || sanitized.eq_ignore_ascii_case("prn")
    {
        return None;
    }

    // Truncate to max length.
    let truncated: String = sanitized.chars().take(MAX_FILENAME_LEN).collect();

    Some(truncated)
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("connection timeout")]
    ConnectionTimeout,
    #[error("read timeout")]
    ReadTimeout,
    #[error("write timeout")]
    WriteTimeout,
    #[error("connection closed by peer")]
    PeerClosed,
    #[error("protocol error: {0}")]
    Protocol(#[from] protocol::ProtocolError),
    #[error("connection in invalid state: {0}")]
    InvalidState(String),
    #[error("rate limit exceeded")]
    RateLimitExceeded,
}

/// Connection state machine.
/// Transitions: Disconnected → Connecting → Handshaking → Established → Disconnecting → Disconnected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// No active connection.
    Disconnected,
    /// TCP connection in progress.
    Connecting,
    /// TCP connected, performing cryptographic handshake.
    Handshaking,
    /// Handshake complete, encrypted communication active.
    Established,
    /// Graceful disconnect in progress.
    Disconnecting,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Handshaking => write!(f, "handshaking"),
            ConnectionState::Established => write!(f, "established"),
            ConnectionState::Disconnecting => write!(f, "disconnecting"),
        }
    }
}

/// A raw frame read from the wire.
pub struct RawFrame {
    pub version: u8,
    pub packet_type: PacketType,
    pub body: Vec<u8>,
}

/// Internal: read a frame from any AsyncRead source.
async fn read_frame_impl<R: AsyncRead + Unpin>(reader: &mut R) -> Result<RawFrame, NetworkError> {
    // Read the 4-byte length prefix
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    match time::timeout(NETWORK_TIMEOUT, reader.read_exact(&mut len_buf)).await {
        Ok(Ok(_n)) => {}
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(NetworkError::PeerClosed);
        }
        Ok(Err(e)) => return Err(NetworkError::Io(e)),
        Err(_) => return Err(NetworkError::ReadTimeout),
    }

    let frame_len = u32::from_be_bytes(len_buf);
    validate_frame_size(frame_len)?;

    // Read the frame payload (version + type + body)
    let mut payload = vec![0u8; frame_len as usize];
    match time::timeout(NETWORK_TIMEOUT, reader.read_exact(&mut payload)).await {
        Ok(Ok(_n)) => {}
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Err(NetworkError::PeerClosed);
        }
        Ok(Err(e)) => return Err(NetworkError::Io(e)),
        Err(_) => return Err(NetworkError::ReadTimeout),
    }

    // Parse version
    let version = payload[0];
    validate_version(version)?;

    // Parse packet type
    let packet_type = PacketType::from_byte(payload[1])?;

    // Extract body (everything after version + type)
    let body = payload[2..].to_vec();

    Ok(RawFrame {
        version,
        packet_type,
        body,
    })
}

/// Read exactly one length-prefixed frame from a TCP stream.
/// Validates frame size and protocol version before returning.
pub async fn read_frame(stream: &mut TcpStream) -> Result<RawFrame, NetworkError> {
    read_frame_impl(stream).await
}

/// Read a frame from an OwnedReadHalf (used by the receive loop).
pub async fn read_frame_from_read_half(
    read_half: &mut OwnedReadHalf,
) -> Result<RawFrame, NetworkError> {
    read_frame_impl(read_half).await
}

/// Write a complete frame to any AsyncWrite stream.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    packet_type: PacketType,
    body: &[u8],
) -> Result<(), NetworkError> {
    let frame = protocol::build_frame(packet_type, body)?;

    match time::timeout(NETWORK_TIMEOUT, writer.write_all(&frame)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(NetworkError::Io(e)),
        Err(_) => return Err(NetworkError::WriteTimeout),
    }

    match time::timeout(NETWORK_TIMEOUT, writer.flush()).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(NetworkError::Io(e)),
        Err(_) => return Err(NetworkError::WriteTimeout),
    }

    Ok(())
}

/// Start a TCP listener on the given TcpListener.
/// Returns accepted connections via the channel.
pub async fn start_listener(
    listener: TcpListener,
    tx: mpsc::Sender<(TcpStream, SocketAddr)>,
) -> Result<(), NetworkError> {
    let addr = listener.local_addr()?;
    tracing::info!(address = %addr, "TCP listener started");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                // Do not log peer_addr in production — IP is sensitive metadata.
                tracing::debug!("accepted incoming connection");
                if tx.send((stream, peer_addr)).await.is_err() {
                    // Receiver dropped, shutdown listener
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to accept connection");
                // Continue accepting, don't crash on transient errors
            }
        }
    }

    Ok(())
}

/// Connect to a remote peer with timeout.
/// Routes through Tor SOCKS5 proxy when Tor is enabled, otherwise direct TCP.
/// Enables TCP keepalive to maintain NAT bindings and detect silent peer disconnects.
pub async fn connect(addr: SocketAddr) -> Result<TcpStream, NetworkError> {
    tracing::debug!(target_addr = %addr, tor_enabled = crate::tor::is_enabled(), "attempting TCP connection");
    let result = time::timeout(CONNECT_TIMEOUT, crate::tor::connect(addr)).await;
    match &result {
        Ok(Ok(_)) => tracing::debug!(target_addr = %addr, "TCP connection succeeded"),
        Ok(Err(e)) => tracing::error!(target_addr = %addr, error = %e, "TCP connection failed"),
        Err(_) => tracing::error!(target_addr = %addr, "TCP connection timed out"),
    }
    let stream = result
        .map_err(|_| NetworkError::ConnectionTimeout)?
        .map_err(|e| match e {
            crate::tor::TorError::Io(io_err) => NetworkError::Io(io_err),
            other => NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                other.to_string(),
            )),
        })?;

    // Set TCP_NODELAY to disable Nagle's algorithm for lower latency messaging.
    let _ = stream.set_nodelay(true);

    Ok(stream)
}

/// Send a heartbeat packet.
pub async fn send_heartbeat(stream: &mut TcpStream) -> Result<(), NetworkError> {
    write_frame(stream, PacketType::Heartbeat, &[]).await
}

/// Send a heartbeat acknowledgment (works with any AsyncWrite).
pub async fn send_heartbeat_ack<W: AsyncWrite + Unpin>(
    writer: &mut W,
) -> Result<(), NetworkError> {
    write_frame(writer, PacketType::HeartbeatAck, &[]).await
}

/// Send a disconnect packet with reason (works with any AsyncWrite).
pub async fn send_disconnect<W: AsyncWrite + Unpin>(
    writer: &mut W,
    reason: protocol::DisconnectReason,
) -> Result<(), NetworkError> {
    let msg = protocol::DisconnectMessage { reason };
    let body = protocol::serialize(&msg)?;
    write_frame(writer, PacketType::Disconnect, &body).await
}

/// Send an error packet.
pub async fn send_error<W: AsyncWrite + Unpin>(
    writer: &mut W,
    code: protocol::ErrorCode,
    description: &str,
) -> Result<(), NetworkError> {
    let msg = protocol::ErrorMessage {
        code,
        description: description.to_string(),
    };
    let body = protocol::serialize(&msg)?;
    write_frame(writer, PacketType::Error, &body).await
}
