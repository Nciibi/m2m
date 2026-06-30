/// M2M — TCP Relay Client
///
/// A lightweight TURN-inspired TCP relay protocol for NAT traversal fallback.
/// When Happy Eyeballs direct strategies fail (e.g. both peers behind symmetric
/// NATs), peers can connect through a TCP relay server that bridges their
/// connections.
///
/// ## Protocol
///
/// All messages are length-prefixed frames over TCP:
///   [4B length BE] [1B message type] [body…]
///
/// Client → Server: REGISTER (0x01), CONNECT (0x02), KEEPALIVE (0x03)
/// Server → Client: REGISTERED (0x81), CONNECTED (0x82), ERROR (0x83), PONG (0x84)
///
/// After CONNECTED, the relay enters raw TCP proxy mode — no more relay framing.
/// The two TCP streams are bidirectionally copied.
///
/// ## Why custom instead of full TURN (RFC 5766)?
///
/// M2M is TCP-only. Full TURN requires HMAC-SHA1, UDP support, and the full
/// Allocate/Refresh/Send/ChannelData lifecycle. A custom TCP relay is simpler,
/// has zero additional crypto dependencies, and is forward-secret by construction
/// (relay never sees plaintext — M2M's XChaCha20-Poly1305 runs on top).
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time;

use thiserror::Error;

use crate::candidate;
use crate::network;
use crate::protocol::{self, PacketType, WireCandidate};
use crate::session::Session;
use crate::state::{AppState, PeerConnection};
use crate::stun;

use crate::commands::util;

// ─── Constants ─────────────────────────────────────────────────────────────────

/// Timeout for TCP connection to the relay server.
const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Timeout for reading a relay control frame (REGISTERED, CONNECTED, etc.).
const RELAY_FRAME_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum relay frame body size (64 KiB — generous for control messages).
const MAX_RELAY_BODY_SIZE: u32 = 65536;

/// Length-prefix size (same as M2M protocol).
const LENGTH_PREFIX_SIZE: usize = 4;

// ─── Error Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("connection timed out")]
    TimedOut,

    #[error("relay frame too large: {size} bytes")]
    FrameTooLarge { size: u32 },

    #[error("relay protocol error: {0}")]
    Protocol(String),

    #[error("relay server error (code {code}): {message}")]
    ServerError { code: u8, message: String },

    #[error("relay closed connection")]
    ConnectionClosed,

    #[error("unexpected relay frame type: {0:#04x}")]
    UnexpectedFrame(u8),

    #[error("config error: {0}")]
    Config(String),
}

// ─── Protocol Types ─────────────────────────────────────────────────────────────

/// Relay message types (client → server).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayRequest {
    Register = 0x01,
    Connect = 0x02,
    #[expect(dead_code, reason = "Reserved relay protocol variant")]
    Keepalive = 0x03,
}

/// Relay message types (server → client).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayResponse {
    Registered = 0x81,
    Connected = 0x82,
    Error = 0x83,
    Pong = 0x84,
}

/// Parsed relay frame.
#[derive(Debug)]
struct RelayFrame {
    msg_type: u8,
    body: Vec<u8>,
}

// ─── Configuration ─────────────────────────────────────────────────────────────

/// Relay server configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelayConfig {
    /// Relay server hostname or IP.
    pub host: String,
    /// Relay server TCP port.
    pub port: u16,
    /// Optional pre-shared key for authentication.
    /// Sent as the body of REGISTER. May be empty for open relays.
    #[serde(default)]
    pub auth_token: String,
}

impl RelayConfig {
    /// Get the relay server address as `host:port`.
    pub fn addr_str(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Return the address as a SocketAddr (best-effort parse).
    pub fn socket_addr(&self) -> Option<SocketAddr> {
        self.addr_str().parse::<SocketAddr>().ok()
    }
}

/// Current relay connection state (for frontend diagnostics).
#[derive(Debug, Clone, serde::Serialize)]
#[derive(Default)]
pub struct RelayState {
    pub connected: bool,
    pub relay_id: Option<String>,
    pub error: Option<String>,
}


// ─── Frame I/O ─────────────────────────────────────────────────────────────────

/// Read exactly one relay frame from an async reader.
///
/// Uses the shared Slowloris-resistant `network::read_exact_timeout` helper
/// for per-byte timeout protection on all reads.
async fn read_relay_frame<R: AsyncReadExt + Unpin>(
    stream: &mut R,
) -> Result<RelayFrame, RelayError> {
    // Read 4-byte length prefix with Slowloris protection
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    network::read_exact_timeout(stream, &mut len_buf, "relay len prefix")
        .await
        .map_err(|e| match e {
            network::NetworkError::PeerClosed => RelayError::ConnectionClosed,
            network::NetworkError::Io(io) => RelayError::Io(io),
            _ => RelayError::TimedOut,
        })?;

    let body_len = u32::from_be_bytes(len_buf) as usize;

    if body_len > MAX_RELAY_BODY_SIZE as usize {
        return Err(RelayError::FrameTooLarge {
            size: body_len as u32,
        });
    }

    if body_len < 1 {
        return Err(RelayError::Protocol("empty relay frame".into()));
    }

    // Read body with Slowloris protection
    let mut body = vec![0u8; body_len];
    network::read_exact_timeout(stream, &mut body, "relay body")
        .await
        .map_err(|e| match e {
            network::NetworkError::PeerClosed => RelayError::ConnectionClosed,
            network::NetworkError::Io(io) => RelayError::Io(io),
            _ => RelayError::TimedOut,
        })?;

    let msg_type = body[0];
    let payload = body[1..].to_vec();

    Ok(RelayFrame {
        msg_type,
        body: payload,
    })
}

/// Write a relay frame to an async writer.
async fn write_relay_frame<W: AsyncWriteExt + Unpin>(
    stream: &mut W,
    msg_type: u8,
    body: &[u8],
) -> Result<(), RelayError> {
    let total_len = 1 + body.len(); // 1 byte for msg_type
    let mut frame = Vec::with_capacity(LENGTH_PREFIX_SIZE + total_len);
    frame.extend_from_slice(&(total_len as u32).to_be_bytes());
    frame.push(msg_type);
    frame.extend_from_slice(body);

    time::timeout(RELAY_FRAME_TIMEOUT, stream.write_all(&frame))
        .await
        .map_err(|_| RelayError::TimedOut)?
        .map_err(RelayError::Io)?;

    time::timeout(RELAY_FRAME_TIMEOUT, stream.flush())
        .await
        .map_err(|_| RelayError::TimedOut)?
        .map_err(RelayError::Io)?;

    Ok(())
}

/// Expect a specific response type from the relay server.
async fn expect_relay_response<R: AsyncReadExt + Unpin>(
    stream: &mut R,
    expected: RelayResponse,
) -> Result<Vec<u8>, RelayError> {
    let frame = time::timeout(RELAY_FRAME_TIMEOUT, read_relay_frame(stream))
        .await
        .map_err(|_| RelayError::TimedOut)??;

    if frame.msg_type == RelayResponse::Error as u8 {
        let code = frame.body.first().copied().unwrap_or(0);
        let message = if frame.body.len() > 1 {
            String::from_utf8_lossy(&frame.body[1..]).to_string()
        } else {
            "unknown error".to_string()
        };
        return Err(RelayError::ServerError { code, message });
    }

    if frame.msg_type != expected as u8 {
        return Err(RelayError::UnexpectedFrame(frame.msg_type));
    }

    Ok(frame.body)
}

// ─── Relay ID Generation ───────────────────────────────────────────────────────

// ─── Registration ──────────────────────────────────────────────────────────────

/// Register with a relay server.
///
/// Opens a TCP connection to the relay server, sends REGISTER, and waits for
/// REGISTERED. Returns the TCP stream (still speaking relay protocol) and the
/// allocated relay_id.
///
/// The caller should spawn `wait_for_bridge()` on the returned stream to handle
/// incoming relay connections.
pub async fn register(config: &RelayConfig) -> Result<(TcpStream, String), RelayError> {
    let relay_addr = config.socket_addr().ok_or_else(|| {
        RelayError::Config(format!("invalid relay address: {}:{}", config.host, config.port))
    })?;

    tracing::info!(relay = %relay_addr, "connecting to relay server");

    // Connect to relay server
    let mut stream = time::timeout(RELAY_CONNECT_TIMEOUT, TcpStream::connect(relay_addr))
        .await
        .map_err(|_| RelayError::TimedOut)?
        .map_err(RelayError::Io)?;

    let _ = stream.set_nodelay(true);

    // Send REGISTER with optional auth token as body
    let auth_bytes = config.auth_token.as_bytes();
    write_relay_frame(&mut stream, RelayRequest::Register as u8, auth_bytes).await?;

    // Expect REGISTERED response
    let body = expect_relay_response(&mut stream, RelayResponse::Registered).await?;

    if body.is_empty() {
        return Err(RelayError::Protocol("REGISTERED response missing relay_id".into()));
    }

    let id_len = body[0] as usize;
    if id_len == 0 || id_len > body.len() - 1 {
        return Err(RelayError::Protocol("invalid relay_id length".into()));
    }

    let relay_id = String::from_utf8_lossy(&body[1..=id_len]).to_string();

    tracing::info!(relay_id = %relay_id, relay = %relay_addr, "relay registration successful");

    Ok((stream, relay_id))
}

// ─── Bridge via Relay (for Bob / invite consumer) ─────────────────────────────

/// Connect to a peer through the relay server.
///
/// Connects to the relay at `relay_addr`, sends CONNECT with `peer_relay_id`,
/// waits for CONNECTED, and returns the TcpStream now in raw proxy mode.
///
/// This is called from `hole_punch::run_relay()` during Happy Eyeballs.
pub async fn connect_via_relay(
    relay_addr: SocketAddr,
    peer_relay_id: &str,
) -> Result<TcpStream, RelayError> {
    tracing::info!(relay = %relay_addr, peer_relay = %peer_relay_id, "connecting via relay");

    // Connect to relay server
    let mut stream = time::timeout(RELAY_CONNECT_TIMEOUT, TcpStream::connect(relay_addr))
        .await
        .map_err(|_| RelayError::TimedOut)?
        .map_err(RelayError::Io)?;

    let _ = stream.set_nodelay(true);

    // Build CONNECT body: [1B id_len][relay_id bytes]
    let id_bytes = peer_relay_id.as_bytes();
    let id_len = id_bytes.len().min(255) as u8;
    let mut body = vec![id_len];
    body.extend_from_slice(&id_bytes[..id_len as usize]);

    write_relay_frame(&mut stream, RelayRequest::Connect as u8, &body).await?;

    // Expect CONNECTED response
    expect_relay_response(&mut stream, RelayResponse::Connected).await?;

    tracing::info!(relay = %relay_addr, "relay bridge established");

    Ok(stream)
}

// ─── Incoming Bridge Listener (for Alice / invite creator) ────────────────────

/// Wait for an incoming bridge on our relay registration.
///
/// This is spawned as a background task after successful `register()`. It reads
/// relay frames from the stream. When CONNECTED arrives, the stream enters raw
/// proxy mode — we read the first M2M frame (expecting HandshakeInit from the
/// peer) and dispatch to `handle_relay_incoming()`.
pub async fn wait_for_bridge(
    mut relay_stream: TcpStream,
    state: Arc<AppState>,
    app_handle: AppHandle,
) {
    let relay_peer = relay_stream
        .peer_addr()
        .ok()
        .unwrap_or_else(|| "0.0.0.0:0".parse().unwrap());

    tracing::info!(relay = %relay_peer, "relay listener started, waiting for peer");

    // Read relay frames until CONNECTED, ERROR, or disconnect
    loop {
        let frame = match read_relay_frame(&mut relay_stream).await {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(relay = %relay_peer, error = %e, "relay listener: frame read failed");
                break;
            }
        };

        match frame.msg_type {
            t if t == RelayResponse::Connected as u8 => {
                tracing::info!(relay = %relay_peer, "relay bridge connected — entering proxy mode");

                // Stream is now in raw proxy mode. Read the first M2M frame.
                match network::read_frame(&mut relay_stream).await {
                    Ok(m2m_frame) => {
                        if m2m_frame.packet_type != PacketType::HandshakeInit {
                            tracing::warn!(packet_type = ?m2m_frame.packet_type, "relay: expected HandshakeInit");
                            let _ = network::send_error(
                                &mut relay_stream,
                                protocol::ErrorCode::HandshakeFailed,
                                "expected handshake init",
                            )
                            .await;
                            return;
                        }

                        // Note: we can't directly call handle_incoming_connection
                        // because we already read the HandshakeInit frame.
                        // We pass the pre-read frame instead.
                        handle_relay_incoming_with_frame(
                            relay_stream,
                            relay_peer,
                            m2m_frame,
                            state,
                            app_handle,
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!(relay = %relay_peer, error = %e, "relay: failed to read initial M2M frame");
                    }
                }
                return;
            }
            t if t == RelayResponse::Pong as u8 => {
                // Keepalive acknowledged — continue waiting.
                tracing::trace!("relay keepalive acknowledged");
            }
            t if t == RelayResponse::Error as u8 => {
                let code = frame.body.first().copied().unwrap_or(0);
                let msg = if frame.body.len() > 1 {
                    String::from_utf8_lossy(&frame.body[1..]).to_string()
                } else {
                    "unknown".to_string()
                };
                tracing::warn!(relay = %relay_peer, code, error = %msg, "relay server error");
                break;
            }
            other => {
                tracing::warn!(relay = %relay_peer, msg_type = %other, "relay: unexpected frame type");
                // Keep reading — could be a delayed keepalive response
            }
        }
    }

    // Update relay state to disconnected
    let mut relay_state = state.relay_state.write().await;
    *relay_state = RelayState {
        connected: false,
        relay_id: None,
        error: Some("relay connection lost".to_string()),
    };
}

/// Handle an incoming M2M connection that arrived via relay, with a pre-read frame.
///
/// Mirrors `commands::network::handle_incoming_connection()` but takes an already-read
/// HandshakeInit frame (since wait_for_bridge already consumed the first read).
async fn handle_relay_incoming_with_frame(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    frame: network::RawFrame,
    state: Arc<AppState>,
    app_handle: AppHandle,
) {
    let mut session = Session::new();
    {
        let identity = state.identity.read().await;
        let kp = match identity.as_ref() {
            Some(kp) => kp,
            None => {
                tracing::error!("cannot handle relay connection: no identity");
                return;
            }
        };

        // Gather our local candidates for the handshake response
        let config = state.stun_config.read().await;
        let stun_result = stun::discover_public_addrs(&config).await.ok();
        drop(config);

        let host_candidates = candidate::gather_host_candidates();
        let ipv6_candidates = candidate::gather_ipv6_candidates();
        let reflexive_candidates = stun_result
            .as_ref()
            .map(candidate::gather_reflexive_candidates)
            .unwrap_or_default();

        let mut all = host_candidates;
        all.extend(ipv6_candidates);
        all.extend(reflexive_candidates);
        all.sort_by(|a, b| b.priority.cmp(&a.priority));
        let wire_candidates: Vec<WireCandidate> = all.iter().map(|c| WireCandidate {
            address: c.address.clone(),
            candidate_type: c.candidate_type as u8,
            relay_id: None,
        }).collect();

        // Update state with gathered candidates
        {
            let mut cand_state = state.candidates.write().await;
            *cand_state = all;
        }

        // Same handshake flow as handle_incoming_connection
        let x25519_pub = state.x25519_identity.read().await
            .as_ref().map(|k| k.public_key_bytes()).unwrap_or([0u8; 32]);
        if let Err(e) = session.handshake_as_responder(&mut stream, kp, &frame, wire_candidates, x25519_pub).await {
            tracing::warn!(error = %e, "relay handshake failed for incoming connection");
            let _ = network::send_error(
                &mut stream,
                protocol::ErrorCode::HandshakeFailed,
                "handshake failed",
            )
            .await;
            return;
        }
    } // identity borrow dropped here

    let peer_key_hex = hex::encode(session.peer_identity_pub);
    let peer_fingerprint = session.peer_fingerprint();

    // Split the stream for the receive loop
    let (read_half, write_half) = stream.into_split();

    let conn = PeerConnection {
        write_half,
        session,
        remote_addr: peer_addr,
        strategy_name: "relay".to_string(),
    };

    let mut conns = state.connections.write().await;
    conns.insert(peer_key_hex.clone(), Arc::new(tokio::sync::Mutex::new(conn)));
    drop(conns);

    // Notify frontend
    let _ = app_handle.emit("m2m://connection", crate::commands::ConnectionEvent {
        peer_key_hex: peer_key_hex.clone(),
        state: "established".to_string(),
        peer_fingerprint: Some(peer_fingerprint.clone()),
        peer_verified: false,
    });

    tracing::info!(peer = %peer_key_hex, "peer connected via relay");

    // Upsert peer in key store
    if let Some(peer_key_bytes) = util::decode_peer_key_logged(&peer_key_hex) {
        let ks = state.key_store.lock().await;
        if let Some(ref store) = *ks {
            let _ = store.upsert_peer(&peer_key_bytes, &peer_fingerprint, None);
        }
    }

    // Start the receive loop (using the one from commands/network)
    crate::commands::network::spawn_receive_loop(app_handle, state, read_half, peer_key_hex, None);
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    /// Helper: create a duplex relay "server" that responds to REGISTER.
    async fn mock_relay_register_ok(mut rx: tokio::io::DuplexStream) {
        // Read REGISTER frame
        let frame = read_relay_frame(&mut rx).await.unwrap();
        assert_eq!(frame.msg_type, RelayRequest::Register as u8);

        // Send REGISTERED with relay_id "test123"
        let body = vec![7u8]; // id_len
        let mut resp = vec![b't', b'e', b's', b't', b'1', b'2', b'3'];
        resp.insert(0, body[0]);
        write_relay_frame(&mut rx, RelayResponse::Registered as u8, &resp)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_register_protocol_roundtrip() {
        let (mut client, server) = duplex(65536);
        tokio::spawn(async move {
            mock_relay_register_ok(server).await;
        });

        // Write REGISTER
        write_relay_frame(&mut client, RelayRequest::Register as u8, b"").await.unwrap();

        // Read REGISTERED response
        let body = expect_relay_response(&mut client, RelayResponse::Registered).await.unwrap();
        let id_len = body[0] as usize;
        let relay_id = String::from_utf8_lossy(&body[1..=id_len]).to_string();
        assert_eq!(relay_id, "test123");
    }

    #[tokio::test]
    async fn test_connect_success() {
        let (mut client, mut server) = duplex(65536);

        // Simulate CONNECT → CONNECTED exchange
        let server_handle = tokio::spawn(async move {
            let frame = read_relay_frame(&mut server).await.unwrap();
            assert_eq!(frame.msg_type, RelayRequest::Connect as u8);
            // Verify relay_id in body
            let id_len = frame.body[0] as usize;
            let relay_id = String::from_utf8_lossy(&frame.body[1..=id_len]).to_string();
            assert_eq!(relay_id, "peer123");

            write_relay_frame(&mut server, RelayResponse::Connected as u8, &[])
                .await
                .unwrap();
        });

        write_relay_frame(&mut client, RelayRequest::Connect as u8, &[7, b'p', b'e', b'e', b'r', b'1', b'2', b'3'])
            .await
            .unwrap();

        expect_relay_response(&mut client, RelayResponse::Connected)
            .await
            .unwrap();

        server_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_server_error() {
        let (mut client, mut server) = duplex(65536);

        tokio::spawn(async move {
            let _ = read_relay_frame(&mut server).await.unwrap();
            // Send ERROR response
            write_relay_frame(&mut server, RelayResponse::Error as u8, &[1, b'u', b'n', b'k', b'n', b'o', b'w', b'n'])
                .await
                .unwrap();
        });

        write_relay_frame(&mut client, RelayRequest::Connect as u8, &[4, b't', b'e', b's', b't'])
            .await
            .unwrap();

        let err = expect_relay_response(&mut client, RelayResponse::Connected).await;
        assert!(err.is_err());
        match err {
            Err(RelayError::ServerError { code, .. }) => assert_eq!(code, 1),
            other => panic!("expected ServerError, got {:?}", other),
        }
    }

    #[test]
    fn test_config_addr_str() {
        let config = RelayConfig {
            host: "relay.example.com".to_string(),
            port: 3478,
            auth_token: String::new(),
        };
        assert_eq!(config.addr_str(), "relay.example.com:3478");
        // Hostname won't parse as SocketAddr
        assert!(config.socket_addr().is_none());

        let config2 = RelayConfig {
            host: "1.2.3.4".to_string(),
            port: 3478,
            auth_token: String::new(),
        };
        assert_eq!(config2.socket_addr(), Some("1.2.3.4:3478".parse().unwrap()));
    }

    #[test]
    fn test_relay_state_default() {
        let state = RelayState::default();
        assert!(!state.connected);
        assert!(state.relay_id.is_none());
        assert!(state.error.is_none());
    }

    #[tokio::test]
    async fn test_frame_read_write_roundtrip() {
        let (mut a, mut b) = duplex(65536);

        // Write a frame from a
        write_relay_frame(&mut a, 0x42, b"hello relay").await.unwrap();

        // Read it at b
        let frame = read_relay_frame(&mut b).await.unwrap();
        assert_eq!(frame.msg_type, 0x42);
        assert_eq!(frame.body, b"hello relay");
    }

    #[tokio::test]
    async fn test_empty_body_frame() {
        let (mut a, mut b) = duplex(65536);

        write_relay_frame(&mut a, 0x01, &[]).await.unwrap();

        let frame = read_relay_frame(&mut b).await.unwrap();
        assert_eq!(frame.msg_type, 0x01);
        assert!(frame.body.is_empty());
    }

    #[tokio::test]
    async fn test_read_on_closed_connection() {
        let (a, mut b) = duplex(65536);
        drop(a); // close write side

        let result = read_relay_frame(&mut b).await;
        assert!(result.is_err());
    }
}
