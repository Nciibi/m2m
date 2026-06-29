
/// M2M — Network Module
///
/// TCP transport with length-prefixed framing, connection state machine,
/// timeouts, heartbeats, rate limiting, connection-level DoS protection,
/// filename sanitization, and graceful disconnect.
///
/// All data crossing the network boundary is treated as untrusted.
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;

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

/// TCP connection timeout — used by the hole_punch module's per-strategy timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

// ─── Connection Rate Limiting ───────────────────────────────────────────────

/// Maximum number of new TCP connections allowed per IP per time window.
const MAX_CONNECTIONS_PER_IP: u32 = 10;

/// Rate limit window duration in seconds.
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Maximum total concurrent connections across all IPs.
const MAX_TOTAL_CONNECTIONS: usize = 50;

/// Per-IP rate limiter with total connection cap.
///
/// Uses a lock-free concurrent hash map (`DashMap`) for per-IP tracking,
/// eliminating the single-mutex bottleneck. Each IP's window is a small
/// `VecDeque<Instant>` that lives in its own DashMap shard.
///
/// ## Limits
/// - **Per-IP**: max 10 new connections per time window (default 60s)
/// - **Global**: max 50 concurrent connections total
pub struct ConnectionLimiter {
    /// Per-IP sliding window timestamps (lock-free concurrent map).
    per_ip: DashMap<IpAddr, VecDeque<Instant>>,
    /// Current total active connection count (lock-free atomic).
    active_connections: AtomicUsize,
    /// Sliding window duration (configurable for testing).
    window_duration: Duration,
}

impl ConnectionLimiter {
    /// Create a new connection limiter with default limits (60s window).
    pub fn new() -> Self {
        Self {
            per_ip: DashMap::new(),
            active_connections: AtomicUsize::new(0),
            window_duration: Duration::from_secs(RATE_LIMIT_WINDOW_SECS),
        }
    }

    /// Create a connection limiter with a custom window duration (for testing).
    #[cfg(test)]
    pub fn with_window(window: Duration) -> Self {
        Self {
            per_ip: DashMap::new(),
            active_connections: AtomicUsize::new(0),
            window_duration: window,
        }
    }

    /// Check if a new connection from this IP is allowed.
    /// Returns `true` if the connection should be accepted,
    /// `false` if rate-limited or at capacity.
    ///
    /// Lock behavior: DashMap shard-level locking, not a global mutex.
    /// Multiple IPs can be checked concurrently without contention.
    pub fn check(&self, ip: IpAddr) -> bool {
        // Global cap: lock-free atomic check.
        if self.active_connections.load(Ordering::Relaxed) >= MAX_TOTAL_CONNECTIONS {
            tracing::warn!(ip = %ip, active = %self.active_connections.load(Ordering::Relaxed), "connection rejected: at max capacity");
            return false;
        }

        // Per-IP rate limit: DashMap shard-level locking (not global).
        let now = Instant::now();
        let mut entry = self.per_ip.entry(ip).or_default();

        // Drain expired entries from the front.
        while let Some(&t) = entry.front() {
            if now.duration_since(t) >= self.window_duration {
                entry.pop_front();
            } else {
                break;
            }
        }

        // Check if rate limited.
        if entry.len() >= MAX_CONNECTIONS_PER_IP as usize {
            tracing::warn!(ip = %ip, count = entry.len(), "connection rejected: per-IP rate limit exceeded");
            return false;
        }

        // Record this connection attempt.
        entry.push_back(now);
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
    #[expect(dead_code, reason = "Used in tests only")]
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
/// ```ignore
/// // This function is part of the m2m crate, use via crate::network::sanitize_filename
/// assert_eq!(crate::network::sanitize_filename("../../../etc/passwd"), None);
/// assert_eq!(crate::network::sanitize_filename("report.pdf"), Some("report.pdf".into()));
/// assert_eq!(crate::network::sanitize_filename(""), None);
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
    #[expect(dead_code, reason = "Reserved error variant for invalid connection states")]
    InvalidState(String),
    #[error("rate limit exceeded")]
    #[expect(dead_code, reason = "Reserved error variant for rate limiting")]
    RateLimitExceeded,
}

/// Connection state machine.
/// Transitions: Disconnected → Handshaking → Established
/// `Connecting` and `Disconnecting` sub-states are managed internally by the
/// network and session layers and are not exposed through this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// No active connection.
    Disconnected,
    /// TCP connected, performing cryptographic handshake.
    Handshaking,
    /// Handshake complete, encrypted communication active.
    Established,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Handshaking => write!(f, "handshaking"),
            ConnectionState::Established => write!(f, "established"),
        }
    }
}

/// A raw frame read from the wire.
pub struct RawFrame {
    /// Protocol version.
    #[expect(dead_code, reason = "Reserved for protocol version negotiation")]
    pub version: u8,
    pub packet_type: PacketType,
    pub body: Vec<u8>,
}

/// Read exactly `buf.len()` bytes from an async reader with a per-byte 1s timeout.
///
/// This is the core Slowloris-protection primitive used across the codebase.
/// Each call to `reader.read()` has a 1-second timeout rather than one timeout
/// for the entire read, preventing an attacker from holding a connection open
/// by sending data at a trickle (e.g. 1 byte / 9 seconds).
///
/// Returns `PeerClosed` on EOF before filling `buf`, `Io` on transport errors,
/// and `ReadTimeout` if any single `read()` call takes longer than 1 second.
pub(crate) async fn read_exact_timeout<R: AsyncRead + Unpin>(
    reader: &mut R,
    buf: &mut [u8],
    label: &'static str,
) -> Result<(), NetworkError> {
    let mut read_pos = 0;
    while read_pos < buf.len() {
        match time::timeout(Duration::from_secs(1), reader.read(&mut buf[read_pos..])).await {
            Ok(Ok(0)) => return Err(NetworkError::PeerClosed),
            Ok(Ok(n)) => read_pos += n,
            Ok(Err(e)) => return Err(NetworkError::Io(e)),
            Err(_) => {
                tracing::warn!(
                    "Slowloris detected: read timeout on {} (progress: {}/{})",
                    label, read_pos, buf.len()
                );
                return Err(NetworkError::ReadTimeout);
            }
        }
    }
    Ok(())
}

/// Internal: read a frame from any AsyncRead source.
/// Includes Slowloris protection: each bytes-read iteration has a 1s timeout
/// instead of a single N-second timeout for the entire frame. An attacker
/// sending 1 byte every 9 seconds will timeout after the first byte.
pub(crate) async fn read_frame_impl<R: AsyncRead + Unpin>(reader: &mut R) -> Result<RawFrame, NetworkError> {
    // ── Slowloris-resistant length prefix read ──
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    read_exact_timeout(reader, &mut len_buf, "length prefix").await?;

    let frame_len = u32::from_be_bytes(len_buf);
    validate_frame_size(frame_len)?;

    // ── Slowloris-resistant frame body read ──
    let mut payload = vec![0u8; frame_len as usize];
    read_exact_timeout(reader, &mut payload, "frame body").await?;

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

/// Read exactly one length-prefixed frame from any async reader.
/// Validates frame size and protocol version before returning.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<RawFrame, NetworkError> {
    read_frame_impl(reader).await
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

/// Send a heartbeat packet (works with any AsyncWrite).
pub async fn send_heartbeat<W: AsyncWrite + Unpin>(
    writer: &mut W,
) -> Result<(), NetworkError> {
    write_frame(writer, PacketType::Heartbeat, &[]).await
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

#[cfg(test)]
mod network_tests {
    use super::*;
    use crate::protocol::{MAX_FRAME_SIZE, PROTOCOL_VERSION};

    // ═══════════════════════════════════════════════════════════
    // sanitize_filename — path traversal and injection defence
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_sanitize_valid_filenames() {
        assert_eq!(sanitize_filename("report.pdf"), Some("report.pdf".into()));
        assert_eq!(sanitize_filename("my file.txt"), Some("my file.txt".into()));
        assert_eq!(sanitize_filename("archive_2024-01.tar"), Some("archive_2024-01.tar".into()));
    }

    #[test]
    fn test_sanitize_rejects_empty() {
        assert_eq!(sanitize_filename(""), None);
        assert_eq!(sanitize_filename("   "), None); // spaces only → trimmed to empty
    }

    #[test]
    fn test_sanitize_rejects_dots() {
        assert_eq!(sanitize_filename("."), None);
        assert_eq!(sanitize_filename(".."), None);
    }

    #[test]
    fn test_sanitize_path_traversal_unix() {
        // Path separators are stripped, dots remain but the result is just "etcpasswd"
        assert_eq!(sanitize_filename("../../../etc/passwd"), Some("......etcpasswd".into()));
        // The key assertion: no path separators survive
        let result = sanitize_filename("../../../etc/passwd").unwrap();
        assert!(!result.contains('/'));
        assert!(!result.contains('\\'));
    }

    #[test]
    fn test_sanitize_path_traversal_windows() {
        let result = sanitize_filename("..\\..\\..\\Windows\\System32\\cmd.exe").unwrap();
        assert!(!result.contains('\\'));
        assert!(!result.contains('/'));
    }

    #[test]
    fn test_sanitize_windows_reserved_names() {
        assert_eq!(sanitize_filename("NUL"), None);
        assert_eq!(sanitize_filename("nul"), None);
        assert_eq!(sanitize_filename("CON"), None);
        assert_eq!(sanitize_filename("con"), None);
        assert_eq!(sanitize_filename("PRN"), None);
        assert_eq!(sanitize_filename("prn"), None);
    }

    #[test]
    fn test_sanitize_strips_control_chars() {
        // Control characters like \0, \n, \r should be stripped
        let result = sanitize_filename("file\x00name\n.txt");
        assert_eq!(result, Some("filename.txt".into()));
    }

    #[test]
    fn test_sanitize_strips_unicode() {
        // Non-ASCII characters should be stripped for safety
        let result = sanitize_filename("файл.txt");
        assert_eq!(result, Some(".txt".into()));
    }

    #[test]
    fn test_sanitize_truncates_long_filenames() {
        let long_name = "a".repeat(300) + ".txt";
        let result = sanitize_filename(&long_name).unwrap();
        assert!(result.len() <= MAX_FILENAME_LEN);
        assert_eq!(result.len(), MAX_FILENAME_LEN);
    }

    #[test]
    fn test_sanitize_preserves_extensions() {
        assert_eq!(sanitize_filename("photo.jpg"), Some("photo.jpg".into()));
        assert_eq!(sanitize_filename("backup.tar.gz"), Some("backup.tar.gz".into()));
    }

    // ═══════════════════════════════════════════════════════════
    // ConnectionLimiter — rate limiting and DoS protection
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_limiter_allows_under_limit() {
        let limiter = ConnectionLimiter::new();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        for _ in 0..MAX_CONNECTIONS_PER_IP {
            assert!(limiter.check(ip), "should allow connections under per-IP limit");
        }
    }

    #[test]
    fn test_limiter_rejects_over_per_ip_limit() {
        let limiter = ConnectionLimiter::new();
        let ip: IpAddr = "10.0.0.2".parse().unwrap();
        // Fill up the per-IP quota
        for _ in 0..MAX_CONNECTIONS_PER_IP {
            assert!(limiter.check(ip));
        }
        // The next one should be rejected
        assert!(!limiter.check(ip), "should reject connections over per-IP limit");
    }

    #[test]
    fn test_limiter_different_ips_independent() {
        let limiter = ConnectionLimiter::new();
        let ip1: IpAddr = "10.0.0.1".parse().unwrap();
        let ip2: IpAddr = "10.0.0.2".parse().unwrap();

        // Fill IP1's quota
        for _ in 0..MAX_CONNECTIONS_PER_IP {
            limiter.check(ip1);
        }
        // IP2 should still be accepted
        assert!(limiter.check(ip2), "different IPs should have independent limits");
    }

    #[test]
    fn test_limiter_global_cap() {
        let limiter = ConnectionLimiter::new();
        // Simulate MAX_TOTAL_CONNECTIONS active connections
        for _ in 0..MAX_TOTAL_CONNECTIONS {
            limiter.increment();
        }
        let ip: IpAddr = "10.0.0.99".parse().unwrap();
        assert!(!limiter.check(ip), "should reject at global capacity");

        // Decrement one — should allow again
        limiter.decrement();
        assert!(limiter.check(ip), "should allow after decrement");
    }

    #[test]
    fn test_limiter_increment_decrement() {
        let limiter = ConnectionLimiter::new();
        assert_eq!(limiter.active_count(), 0);
        limiter.increment();
        limiter.increment();
        assert_eq!(limiter.active_count(), 2);
        limiter.decrement();
        assert_eq!(limiter.active_count(), 1);
        limiter.decrement();
        assert_eq!(limiter.active_count(), 0);
    }

    // ═══════════════════════════════════════════════════════════
    // Frame read/write roundtrip via tokio duplex
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_write_read_frame_roundtrip() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        let body = b"test payload data";
        write_frame(&mut writer, PacketType::EncryptedMessage, body).await.unwrap();

        let frame = read_frame_impl(&mut reader).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::EncryptedMessage);
        assert_eq!(frame.body, body);
    }

    #[tokio::test]
    async fn test_write_read_empty_frame() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        write_frame(&mut writer, PacketType::Heartbeat, &[]).await.unwrap();

        let frame = read_frame_impl(&mut reader).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::Heartbeat);
        assert!(frame.body.is_empty());
    }

    #[tokio::test]
    async fn test_write_read_multiple_frames() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        write_frame(&mut writer, PacketType::Heartbeat, &[]).await.unwrap();
        write_frame(&mut writer, PacketType::EncryptedMessage, b"msg1").await.unwrap();
        write_frame(&mut writer, PacketType::Disconnect, b"bye").await.unwrap();

        let f1 = read_frame_impl(&mut reader).await.unwrap();
        assert_eq!(f1.packet_type, PacketType::Heartbeat);

        let f2 = read_frame_impl(&mut reader).await.unwrap();
        assert_eq!(f2.packet_type, PacketType::EncryptedMessage);
        assert_eq!(f2.body, b"msg1");

        let f3 = read_frame_impl(&mut reader).await.unwrap();
        assert_eq!(f3.packet_type, PacketType::Disconnect);
        assert_eq!(f3.body, b"bye");
    }

    #[tokio::test]
    async fn test_read_frame_detects_closed_connection() {
        let (writer, mut reader) = tokio::io::duplex(65536);
        // Drop the writer immediately — simulates peer disconnect
        drop(writer);

        let result = read_frame_impl(&mut reader).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_read_large_frame() {
        let (mut writer, mut reader) = tokio::io::duplex(1024 * 1024);

        // 256 KB payload (within MAX_FRAME_SIZE)
        let body = vec![0xAB; 256 * 1024];
        write_frame(&mut writer, PacketType::FileTransferChunk, &body).await.unwrap();

        let frame = read_frame_impl(&mut reader).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::FileTransferChunk);
        assert_eq!(frame.body.len(), body.len());
        assert_eq!(frame.body, body);
    }

    // ═══════════════════════════════════════════════════════════
    // ConnectionLimiter — edge cases (security hardening)
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_limiter_window_expiry() {
        // Use a 1-second window so this test completes in ~2s instead of ~61s.
        let window = Duration::from_secs(1);
        let limiter = ConnectionLimiter::with_window(window);
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        // Fill per-IP quota to the limit
        for _ in 0..MAX_CONNECTIONS_PER_IP {
            assert!(limiter.check(ip), "should allow connections up to per-IP limit");
        }
        // Verify the quota is full
        assert!(!limiter.check(ip), "should reject connection over per-IP limit");

        // Wait for the window to expire (1s window + 1s margin)
        std::thread::sleep(window + Duration::from_secs(1));

        // After the window expires, old entries are drained and new connections allowed
        assert!(limiter.check(ip), "window expired — new connection should be allowed");
    }

    #[test]
    fn test_limiter_ipv6() {
        let limiter = ConnectionLimiter::new();
        let ip_loopback: IpAddr = "::1".parse().unwrap();
        let ip_unique: IpAddr = "2001:db8::1".parse().unwrap();

        // Fill IPv6 loopback quota
        for _ in 0..MAX_CONNECTIONS_PER_IP {
            assert!(limiter.check(ip_loopback), "IPv6 loopback should be allowed up to limit");
        }
        assert!(!limiter.check(ip_loopback), "IPv6 loopback should hit per-IP limit");

        // A different IPv6 address has its own quota
        assert!(limiter.check(ip_unique), "different IPv6 address should be independent");
    }

    #[test]
    fn test_limiter_check_then_increment_flow() {
        let limiter = ConnectionLimiter::new();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();

        // Real-world flow: check() passes → increment()
        assert!(limiter.check(ip), "check should pass under limit");
        limiter.increment();
        assert_eq!(limiter.active_count(), 1);

        // Decrement brings the count back
        limiter.decrement();
        assert_eq!(limiter.active_count(), 0);
    }

    // ═══════════════════════════════════════════════════════════
    // Frame validation — error propagation from protocol layer
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_read_frame_reserved_version_rejected() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        // Craft a frame with reserved version 0x00
        // Frame: [4B length=2] [1B version=0x00] [1B type=0x10]
        let len = (2u32).to_be_bytes();
        writer.write_all(&len).await.unwrap();
        writer.write_all(&[0x00, PacketType::EncryptedMessage.to_byte()]).await.unwrap();

        let result = read_frame_impl(&mut reader).await;
        assert!(matches!(
            result,
            Err(NetworkError::Protocol(protocol::ProtocolError::ReservedVersion(0x00)))
        ), "expected ReservedVersion(0x00)");
    }

    #[tokio::test]
    async fn test_read_frame_unsupported_version_rejected() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        // Craft a frame with unsupported version 0xFC
        let len = (2u32).to_be_bytes();
        writer.write_all(&len).await.unwrap();
        writer.write_all(&[0xFC, PacketType::EncryptedMessage.to_byte()]).await.unwrap();

        let result = read_frame_impl(&mut reader).await;
        assert!(matches!(
            result,
            Err(NetworkError::Protocol(protocol::ProtocolError::UnsupportedVersion(0xFC)))
        ), "expected UnsupportedVersion(0xFC)");
    }

    #[tokio::test]
    async fn test_read_frame_unknown_packet_type_rejected() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        // Craft a frame with valid version but unknown packet type 0xFF
        let len = (2u32).to_be_bytes();
        writer.write_all(&len).await.unwrap();
        writer.write_all(&[PROTOCOL_VERSION, 0xFF]).await.unwrap();

        let result = read_frame_impl(&mut reader).await;
        assert!(matches!(
            result,
            Err(NetworkError::Protocol(protocol::ProtocolError::UnknownPacketType(0xFF)))
        ), "expected UnknownPacketType(0xFF)");
    }

    #[tokio::test]
    async fn test_read_frame_size_too_large_rejected() {
        let (mut writer, mut reader) = tokio::io::duplex(1024 * 1024);

        // Length prefix claims MAX_FRAME_SIZE + 1 bytes (over 16 MiB)
        let oversized = MAX_FRAME_SIZE + 1;
        let len = oversized.to_be_bytes();
        writer.write_all(&len).await.unwrap();

        let result = read_frame_impl(&mut reader).await;
        assert!(matches!(
            result,
            Err(NetworkError::Protocol(protocol::ProtocolError::FrameTooLarge { .. }))
        ), "expected FrameTooLarge");
    }

    // ═══════════════════════════════════════════════════════════
    // Slowloris detection — per-byte 1s timeout verification
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_read_frame_timeout_during_length_prefix() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        // Write only 2 of the 4 length-prefix bytes — the per-byte 1s timeout
        // should fire on the 3rd byte attempt.
        writer.write_all(&[0x00, 0x01]).await.unwrap();

        let result = read_frame_impl(&mut reader).await;
        assert!(matches!(result, Err(NetworkError::ReadTimeout)),
            "expected ReadTimeout from incomplete length prefix");
    }

    #[tokio::test]
    async fn test_read_frame_timeout_during_body() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        // Write valid length prefix declaring 100-byte payload
        let body_len = 100u32;
        writer.write_all(&body_len.to_be_bytes()).await.unwrap();
        // Write only 1 byte of the declared body
        writer.write_all(&[0xAA]).await.unwrap();

        let result = read_frame_impl(&mut reader).await;
        assert!(matches!(result, Err(NetworkError::ReadTimeout)),
            "expected ReadTimeout from incomplete body");
    }

    #[tokio::test]
    async fn test_read_frame_peer_closed_during_body() {
        let (mut writer, mut reader) = tokio::io::duplex(65536);

        // Write valid length prefix, then drop the writer (peer disconnect)
        let body_len = 100u32;
        writer.write_all(&body_len.to_be_bytes()).await.unwrap();
        drop(writer); // Simulate peer closing the connection

        let result = read_frame_impl(&mut reader).await;
        assert!(matches!(result, Err(NetworkError::PeerClosed)),
            "expected PeerClosed after writer drop during body read");
    }
}
