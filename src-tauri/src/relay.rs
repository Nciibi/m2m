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

use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time;

use thiserror::Error;

use crate::network;
use crate::protocol::{self, PacketType};
use crate::state::AppState;

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
    Keepalive = 0x03,
}

/// How often the client sends `KEEPALIVE` while parked in
/// [`wait_for_bridge`], and how many consecutive unanswered probes are
/// tolerated before the registration is declared dead.
///
/// The relay server reaps a registration after `READER_IDLE_TIMEOUT` (300s)
/// measured from its last keepalive. At 60s with 3 strikes the client has
/// 180s of margin — three lost probes on a healthy link — before it gives up,
/// so a single dropped datagram does not tear down a working invite.
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(60);
const KEEPALIVE_MAX_MISSES: u32 = 3;

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
#[derive(Debug, Clone, serde::Serialize, Default)]
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
    read_relay_frame_with_deadline(stream, network::RELAY_DEFAULT_READ_DEADLINE).await
}

/// [`read_relay_frame`] with an explicit total read deadline.
///
/// The deadline has to be a parameter, not a constant. `read_exact_timeout`
/// applies its budget per *call*, so a caller that is deliberately parked
/// waiting for a reply that may not arrive — the keepalive loop — must pass a
/// budget longer than its own wait, or the inner read always wins the race and
/// the caller's timeout branch is unreachable. That is exactly what happened:
/// the keepalive branch below was dead code and every idle relay registration
/// was torn down after ~1s.
async fn read_relay_frame_with_deadline<R: AsyncReadExt + Unpin>(
    stream: &mut R,
    deadline: Duration,
) -> Result<RelayFrame, RelayError> {
    // Read 4-byte length prefix with Slowloris protection
    let started = std::time::Instant::now();
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    network::read_exact_timeout_with(stream, &mut len_buf, "relay len prefix", deadline)
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

    // Read body with Slowloris protection. The remaining budget is carried
    // over rather than restarted, so a slow length prefix cannot double the
    // time a peer has to occupy this slot.
    let remaining = deadline.saturating_sub(started.elapsed());
    let mut body = vec![0u8; body_len];
    network::read_exact_timeout_with(stream, &mut body, "relay body", remaining)
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
        RelayError::Config(format!(
            "invalid relay address: {}:{}",
            config.host, config.port
        ))
    })?;

    tracing::info!(relay = %relay_addr, "connecting to relay server");

    // Connect to relay server through the Tor-aware chokepoint. A relay is
    // exactly the kind of third party whose traffic must not reveal the
    // user's IP, so this path used to bypass Tor as well.
    let mut stream = crate::dial::dial_with_timeout(relay_addr, RELAY_CONNECT_TIMEOUT)
        .await
        .map_err(relay_dial_err)?;

    let _ = stream.set_nodelay(true);

    // Send REGISTER with optional auth token as body
    let auth_bytes = config.auth_token.as_bytes();
    write_relay_frame(&mut stream, RelayRequest::Register as u8, auth_bytes).await?;

    // Expect REGISTERED response
    let body = expect_relay_response(&mut stream, RelayResponse::Registered).await?;

    if body.is_empty() {
        return Err(RelayError::Protocol(
            "REGISTERED response missing relay_id".into(),
        ));
    }

    let id_len = body[0] as usize;
    if id_len == 0 || id_len > body.len() - 1 {
        return Err(RelayError::Protocol("invalid relay_id length".into()));
    }

    let relay_id = String::from_utf8_lossy(&body[1..=id_len]).to_string();

    tracing::info!(relay_id = %relay_id, relay = %relay_addr, "relay registration successful");

    Ok((stream, relay_id))
}

/// Map a chokepoint dial failure onto a relay error.
///
/// A relay is a third party, so its connection is Tor-routed. A relay address
/// that is not Tor-routable (typically a self-hosted relay on a LAN address)
/// is reported as a configuration problem so the user learns why, rather than
/// seeing an opaque connection failure.
fn relay_dial_err(e: crate::dial::DialError) -> RelayError {
    match e {
        crate::dial::DialError::TimedOut(_) => RelayError::TimedOut,
        crate::dial::DialError::Io(e) => RelayError::Io(e),
        crate::dial::DialError::Dial(msg) => RelayError::Protocol(msg),
        crate::dial::DialError::NonTorRoutable(a)
        | crate::dial::DialError::TorLanUnsupported(a) => RelayError::Config(format!(
            "relay address {a} is not reachable over Tor — \
             self-hosted relays must use a public hostname or IP"
        )),
        // Cannot arise on a TCP connect path; kept for exhaustiveness as the
        // UDP chokepoint grows.
        crate::dial::DialError::TorUdpUnsupported => RelayError::Config(
            "outbound UDP queries are blocked in the current transport mode".into(),
        ),
    }
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
    auth_token: &str,
) -> Result<TcpStream, RelayError> {
    tracing::info!(relay = %relay_addr, peer_relay = %peer_relay_id, "connecting via relay");

    // Connect to relay server through the Tor-aware chokepoint — the relay
    // is a third party that must not learn the user's real address.
    let mut stream = crate::dial::dial_with_timeout(relay_addr, RELAY_CONNECT_TIMEOUT)
        .await
        .map_err(relay_dial_err)?;

    let _ = stream.set_nodelay(true);

    // Build CONNECT body v2: [1B id_len][relay_id bytes][1B tok_len][token bytes]
    // Servers without auth configured ignore the trailing token section;
    // servers WITH auth require it (older clients are rejected).
    let id_bytes = peer_relay_id.as_bytes();
    let id_len = id_bytes.len().min(255) as u8;
    let mut body = vec![id_len];
    body.extend_from_slice(&id_bytes[..id_len as usize]);

    let token_bytes = auth_token.as_bytes();
    let tok_len = token_bytes.len().min(255) as u8;
    body.push(tok_len);
    body.extend_from_slice(&token_bytes[..tok_len as usize]);

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

    // Read relay frames until CONNECTED, ERROR, or disconnect.
    //
    // The read is bounded by `KEEPALIVE_INTERVAL` so an idle registration still
    // emits a keepalive. Previously this loop blocked indefinitely in
    // `read_relay_frame` and never sent one, so the server reaped the
    // registration on age alone after 5 minutes while the client continued to
    // report `connected: true` and kept advertising a dead `relay_id` in
    // every invite it generated.
    let mut consecutive_misses: u32 = 0;
    loop {
        // The inner read deadline is deliberately *longer* than the outer wait,
        // so the outer `timeout` is the one that fires and the keepalive branch
        // below is reachable. With the shared default deadline the inner read
        // always won the race, the `Err(_)` arm never executed, and every idle
        // registration was torn down ~1s after registering while the app still
        // reported `connected: true`.
        let inner_deadline = KEEPALIVE_INTERVAL + Duration::from_secs(5);
        let frame = match time::timeout(
            KEEPALIVE_INTERVAL,
            read_relay_frame_with_deadline(&mut relay_stream, inner_deadline),
        )
        .await
        {
            Ok(Ok(f)) => {
                consecutive_misses = 0;
                f
            }
            // Timed out with nothing to read: probe, then keep waiting.
            Err(_) => {
                consecutive_misses += 1;
                if consecutive_misses > KEEPALIVE_MAX_MISSES {
                    tracing::warn!(
                        relay = %relay_peer,
                        misses = consecutive_misses,
                        "relay listener: no response to keepalives — dropping registration"
                    );
                    break;
                }
                if let Err(e) =
                    write_relay_frame(&mut relay_stream, RelayRequest::Keepalive as u8, &[]).await
                {
                    tracing::warn!(relay = %relay_peer, error = %e, "relay listener: keepalive write failed");
                    break;
                }
                continue;
            }
            Ok(Err(e)) => {
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
                        // Accept either handshake initiator, exactly as the
                        // direct-TCP responder does. This previously matched
                        // only `HandshakeInit`, so an X3DH peer tunnelling
                        // through the relay — the path that most needs it, since
                        // relayed peers are often the hardest to reach
                        // directly — was refused outright.
                        if m2m_frame.packet_type != PacketType::HandshakeInit
                            && m2m_frame.packet_type != PacketType::X3DHHandshakeInit
                        {
                            tracing::warn!(packet_type = ?m2m_frame.packet_type, "relay: expected a handshake init");
                            let _ = network::send_error(
                                &mut relay_stream,
                                protocol::ErrorCode::HandshakeFailed,
                                "expected handshake init",
                            )
                            .await;
                            return;
                        }

                        // ── Rate-limit the relay path like the direct path ──
                        //
                        // `start_listening` wraps every direct accept in
                        // `connection_limiter.check/increment/decrement`; this
                        // bridge had none of it, so the relay — a full MITM by
                        // design, and therefore fully under the control of
                        // whoever runs it — could bridge unlimited peers into a
                        // spawned handshake task each and then a permanent
                        // `state.connections` entry holding a socket, a `Session`
                        // and ratchet state. It also sidestepped the per-IP
                        // defence entirely: every relayed peer arrives from the
                        // relay's single address, so keying on the source IP sees
                        // one peer, not the many behind it.
                        //
                        // Mirrors `start_listening` exactly, including the reap
                        // (without it `check()` inserts a per-IP entry for every
                        // address it is about to reject, and the map grows without
                        // bound) and the `RateLimitExceeded` error frame, so the
                        // peer is told why instead of watching a silent close.
                        let ip = relay_peer.ip();
                        let reaped = state.connection_limiter.reap();
                        if reaped > 0 {
                            tracing::debug!(reaped, "reaped expired per-IP rate limit entries");
                        }
                        if !state.connection_limiter.check(ip) {
                            tracing::warn!(
                                relay = %relay_peer,
                                "relayed connection rejected by rate limiter"
                            );
                            let _ = network::send_error(
                                &mut relay_stream,
                                protocol::ErrorCode::RateLimitExceeded,
                                "rate limited — too many connections",
                            )
                            .await;
                            return;
                        }

                        // Note: we can't directly call handle_incoming_connection
                        // because we already read the HandshakeInit frame.
                        // We pass the pre-read frame instead.
                        let limiter_state = state.clone();
                        state.connection_limiter.increment();
                        handle_relay_incoming_with_frame(
                            relay_stream,
                            relay_peer,
                            m2m_frame,
                            state,
                            app_handle,
                        )
                        .await;
                        // Same accounting as the direct path: the count tracks
                        // handshakes in flight, not live sessions, which is why
                        // `complete_inbound_connection` also caps the size of the
                        // connection map itself.
                        limiter_state.connection_limiter.decrement();
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
/// This used to be a ~150-line fork of the direct-TCP inbound path. It had
/// already drifted: it ran a live STUN discovery on an unauthenticated
/// connection (the DoS amplification the direct path avoids, and an outright
/// Tor bypass), it held `state.identity` across the handshake, and it had no
/// X3DH dispatch — so an X3DH peer using the relay was refused.
///
/// It now delegates to the single shared implementation, which is the whole
/// point: the relay cannot fall behind the direct path again, because there is
/// no second copy to fall behind.
///
/// The per-IP `ConnectionLimiter` accounting lives in [`wait_for_bridge`]
/// rather than here, because that is where the direct path's equivalent lives
/// (`start_listening`): the limiter bounds *attempts* per source address, and
/// the bridge has exactly one source address — the relay — no matter how many
/// peers stand behind it.
async fn handle_relay_incoming_with_frame(
    stream: TcpStream,
    peer_addr: SocketAddr,
    frame: network::RawFrame,
    state: Arc<AppState>,
    app_handle: AppHandle,
) {
    crate::commands::network::complete_inbound_connection(
        &app_handle,
        &state,
        stream,
        peer_addr,
        Some(frame),
    )
    .await;
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
        let body = [7u8]; // id_len
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
        write_relay_frame(&mut client, RelayRequest::Register as u8, b"")
            .await
            .unwrap();

        // Read REGISTERED response
        let body = expect_relay_response(&mut client, RelayResponse::Registered)
            .await
            .unwrap();
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

        write_relay_frame(
            &mut client,
            RelayRequest::Connect as u8,
            &[7, b'p', b'e', b'e', b'r', b'1', b'2', b'3'],
        )
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
            write_relay_frame(
                &mut server,
                RelayResponse::Error as u8,
                &[1, b'u', b'n', b'k', b'n', b'o', b'w', b'n'],
            )
            .await
            .unwrap();
        });

        write_relay_frame(
            &mut client,
            RelayRequest::Connect as u8,
            &[4, b't', b'e', b's', b't'],
        )
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
        write_relay_frame(&mut a, 0x42, b"hello relay")
            .await
            .unwrap();

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
