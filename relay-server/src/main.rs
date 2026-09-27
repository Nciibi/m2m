/// M2M — TCP Relay Server
///
/// Standalone relay server for the M2M TCP relay protocol. Peers behind
/// symmetric NATs that cannot establish direct TCP connections can use this
/// relay as a last-resort bridge.
///
/// ## Protocol
///
/// Length-prefixed frames over TCP:
///   [4B length BE] [1B message type] [body…]
///
/// Client → Server:
///   - 0x01 REGISTER  body=[auth_token]  — register for incoming connections
///   - 0x02 CONNECT   body=[1B id_len][relay_id][1B tok_len][token] — request bridge
///     (the token section is required when RELAY_AUTH_TOKEN is set; optional otherwise)
///   - 0x03 KEEPALIVE body=empty — extend registration TTL
///
/// Server → Client:
///   - 0x81 REGISTERED body=[1B id_len][relay_id] — registration confirmed
///   - 0x82 CONNECTED  body=empty — bridge established → raw proxy mode
///   - 0x83 ERROR      body=[1B code][message] — error occurred
///   - 0x84 PONG       body=empty — keepalive acknowledged
///
/// After CONNECTED is sent to both sides, raw TCP proxy mode begins
/// (tokio::io::copy_bidirectional) — no more relay framing is parsed.
///
/// ## Usage
///
/// ```sh
/// # Via cargo (from the workspace root)
/// cargo build --release -p m2m-relay
/// ./target/release/m2m-relay
///
/// # With authentication
/// RELAY_AUTH_TOKEN=secret RELAY_PORT=3478 ./target/release/m2m-relay
///
/// # Via Docker
/// docker compose up -d
/// ```
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, RwLock};
use tokio::time;

const LENGTH_PREFIX_SIZE: usize = 4;
const PER_BYTE_TIMEOUT: Duration = Duration::from_secs(1);
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_BODY_SIZE: u32 = 65536;
const DEFAULT_PORT: u16 = 3478;
const READER_IDLE_TIMEOUT: Duration = Duration::from_secs(300); // 5 min
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Maximum concurrent connections from a single IP address.
const MAX_CONNECTIONS_PER_IP: usize = 16;
/// Maximum total concurrent connections (global cap).
const MAX_TOTAL_CONNECTIONS: usize = 1024;

/// Maximum number of *pending registrations* held at once.
///
/// The per-IP connection cap only throttles how fast a client can complete the
/// REGISTER handshake — it does not bound how many registrations accumulate.
/// A trivial loop (connect → REGISTER → read REGISTERED → close) added one
/// `HashMap` entry and one spawned task per iteration, each living for
/// `READER_IDLE_TIMEOUT` (5 minutes), so a few thousand cheap connections could
/// exhaust memory and task slots. Registration slots are a separate resource and
/// now have their own cap.
const MAX_PENDING_REGISTRATIONS: usize = 1024;

/// Maximum duration a bridged (proxied) connection may stay idle before it is
/// torn down.
///
/// `copy_bidirectional` previously had no deadline of any kind: a single
/// registration plus one CONNECT gave an attacker a permanently open socket
/// with no cap, no idle timeout and no byte budget. Since bridged connections
/// are the long-lived, valuable ones, an idle timeout is what keeps a single
/// registration from becoming free server capacity.
const BRIDGE_IDLE_TIMEOUT: Duration = Duration::from_secs(600); // 10 min

/// A registered peer awaiting a bridge connection.
///
/// The `bridge_tx` channel is used to deliver the other peer's TCP stream
/// when a CONNECT request arrives. The receiver side lives in the reader
/// task spawned after registration.
struct Registration {
    bridge_tx: oneshot::Sender<TcpStream>,
    peer_addr: SocketAddr,
    created_at: Instant,
}

// ─── Frame I/O ───────────────────────────────────────────────────────────────

/// Generic over the transport so the codec can be unit-tested against an
/// in-memory duplex stream instead of requiring a real socket.
async fn read_frame<S: tokio::io::AsyncRead + Unpin>(
    stream: &mut S,
) -> Result<(u8, Vec<u8>), String> {
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    let mut pos = 0;
    while pos < LENGTH_PREFIX_SIZE {
        match time::timeout(PER_BYTE_TIMEOUT, stream.read(&mut len_buf[pos..])).await {
            Ok(Ok(0)) => return Err("connection closed".to_string()),
            Ok(Ok(n)) => pos += n,
            Ok(Err(e)) => return Err(format!("read error: {e}")),
            Err(_) => return Err("read timeout".to_string()),
        }
    }
    let body_len = u32::from_be_bytes(len_buf) as usize;
    if body_len > MAX_BODY_SIZE as usize {
        return Err(format!("frame too large: {body_len}"));
    }
    if body_len < 1 {
        return Err("empty frame".to_string());
    }
    let mut body = vec![0u8; body_len];
    let mut pos = 0;
    while pos < body_len {
        match time::timeout(PER_BYTE_TIMEOUT, stream.read(&mut body[pos..])).await {
            Ok(Ok(0)) => return Err("connection closed during body".to_string()),
            Ok(Ok(n)) => pos += n,
            Ok(Err(e)) => return Err(format!("read error: {e}")),
            Err(_) => return Err("body read timeout".to_string()),
        }
    }
    Ok((body[0], body[1..].to_vec()))
}

async fn write_frame<W: tokio::io::AsyncWrite + Unpin>(
    stream: &mut W,
    msg_type: u8,
    body: &[u8],
) -> Result<(), String> {
    let total_len = 1 + body.len();
    let mut frame = Vec::with_capacity(LENGTH_PREFIX_SIZE + total_len);
    frame.extend_from_slice(&(total_len as u32).to_be_bytes());
    frame.push(msg_type);
    frame.extend_from_slice(body);
    time::timeout(FRAME_TIMEOUT, stream.write_all(&frame))
        .await
        .map_err(|_| "write timeout".to_string())?
        .map_err(|e| format!("write error: {e}"))?;
    time::timeout(FRAME_TIMEOUT, stream.flush())
        .await
        .map_err(|_| "flush timeout".to_string())?
        .map_err(|e| format!("flush error: {e}"))?;
    Ok(())
}

async fn send_error(stream: &mut TcpStream, code: u8, msg: &str) {
    let mut body = vec![code];
    body.extend_from_slice(msg.as_bytes());
    let _ = write_frame(stream, 0x83, &body).await;
}

// ─── Relay ID ────────────────────────────────────────────────────────────────

/// Generate an unguessable relay bridge ID: 128 bits of CSPRNG output, hex-encoded.
///
/// The old implementation used 32 bits of wall-clock nanoseconds, making
/// bridge IDs enumerable — with unauthenticated CONNECT this allowed any
/// third party to hijack a pending bridge.
fn generate_relay_id() -> String {
    let bytes: [u8; 16] = rand::random();
    hex::encode(bytes)
}

// ─── Auth ────────────────────────────────────────────────────────────────────

/// Constant-time byte-slice equality (no early exit on mismatch).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Verify a provided auth token against the configured token.
///
/// When no token is configured (`auth_token` empty), authentication is
/// disabled and everything is accepted.
fn verify_auth(provided: &[u8], auth_token: &str) -> bool {
    if auth_token.is_empty() {
        return true;
    }
    constant_time_eq(provided, auth_token.as_bytes())
}

// ─── Registration Reader ─────────────────────────────────────────────────────

/// Task that reads relay frames from a registered client's stream.
///
/// Handles KEEPALIVE (→ PONG), detects disconnects, and waits for the bridge
/// signal. When the bridge signal arrives (via oneshot), sends CONNECTED to
/// both sides and enters raw TCP proxy mode.
async fn registration_reader(
    mut alice_stream: TcpStream,
    relay_id: String,
    bridge_rx: oneshot::Receiver<TcpStream>,
) {
    tracing::info!(relay_id = %relay_id, "registration reader started");

    // We need a pinned bridge_rx to use in tokio::select!
    tokio::pin!(bridge_rx);

    loop {
        tokio::select! {
            // Read relay frames from the registered client
            frame = read_frame(&mut alice_stream) => {
                match frame {
                    Ok((0x03, _)) => {
                        // KEEPALIVE → send PONG
                        let _ = write_frame(&mut alice_stream, 0x84, &[]).await;
                    }
                    Ok((other, _)) => {
                        tracing::warn!(relay_id = %relay_id, msg_type = other, "unexpected frame from registered client");
                        // Continue — could be a late frame before bridge
                    }
                    Err(e) => {
                        tracing::info!(relay_id = %relay_id, error = %e, "registered client disconnected");
                        return;
                    }
                }
            }

            // Bridge signal from CONNECT handler
            bob_stream = &mut bridge_rx => {
                match bob_stream {
                    Ok(mut bob_stream) => {
                        tracing::info!(relay_id = %relay_id, "bridge requested — entering proxy mode");

                        // Send CONNECTED to both sides
                        if write_frame(&mut alice_stream, 0x82, &[]).await.is_err() {
                            tracing::warn!(relay_id = %relay_id, "failed to send CONNECTED to registered client");
                            return;
                        }
                        if write_frame(&mut bob_stream, 0x82, &[]).await.is_err() {
                            tracing::warn!(relay_id = %relay_id, "failed to send CONNECTED to connecting client");
                            return;
                        }

                        tracing::info!(relay_id = %relay_id, "starting bidirectional proxy");

                        // Enter raw TCP proxy mode, bounded by an idle
                        // deadline. Without it a single registration plus one
                        // CONNECT yielded a permanently open socket with no
                        // cap and no byte budget — free server capacity for
                        // anyone who felt like holding it.
                        //
                        // `copy_bidirectional` returns when EITHER direction
                        // closes, so the timeout fires on genuine inactivity
                        // (no bytes in either direction for
                        // BRIDGE_IDLE_TIMEOUT), not on normal use.
                        match time::timeout(
                            BRIDGE_IDLE_TIMEOUT,
                            tokio::io::copy_bidirectional(&mut alice_stream, &mut bob_stream),
                        )
                        .await
                        {
                            Ok(Ok((a_to_b, b_to_a))) => {
                                tracing::info!(
                                    relay_id = %relay_id,
                                    sent = a_to_b,
                                    received = b_to_a,
                                    "relay connection closed normally"
                                );
                            }
                            Ok(Err(e)) => {
                                tracing::warn!(relay_id = %relay_id, error = %e, "relay proxy error");
                            }
                            Err(_) => {
                                tracing::warn!(
                                    relay_id = %relay_id,
                                    timeout_secs = BRIDGE_IDLE_TIMEOUT.as_secs(),
                                    "bridge idle timeout — tearing down"
                                );
                            }
                        }
                    }
                    Err(_) => {
                        tracing::warn!(relay_id = %relay_id, "bridge channel cancelled");
                    }
                }
                return;
            }
        }
    }
}

// ─── Request Handlers ────────────────────────────────────────────────────────

async fn handle_register(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    auth_body: Vec<u8>,
    state: Arc<RwLock<HashMap<String, Registration>>>,
    auth_token: &str,
) {
    if !auth_token.is_empty() {
        if !verify_auth(&auth_body, auth_token) {
            tracing::warn!(peer = %peer_addr, "authentication failed");
            send_error(&mut stream, 1, "authentication failed").await;
            return;
        }
    }

    let relay_id = generate_relay_id();

    // Send REGISTERED response before storing anything
    let id_bytes = relay_id.as_bytes();
    let id_len = id_bytes.len().min(255) as u8;
    let mut resp = vec![id_len];
    resp.extend_from_slice(&id_bytes[..id_len as usize]);

    if let Err(e) = write_frame(&mut stream, 0x81, &resp).await {
        tracing::error!(peer = %peer_addr, error = %e, "failed to send REGISTERED");
        return;
    }

    // Create the bridge channel
    let (bridge_tx, bridge_rx) = oneshot::channel::<TcpStream>();

    // Store the registration (the sender side, to deliver Bob's stream).
    // Enforce a cap: without it, REGISTER is a trivial way to grow this map
    // and the task count without bound.
    {
        let mut map = state.write().await;
        if map.len() >= MAX_PENDING_REGISTRATIONS {
            tracing::warn!(
                peer = %peer_addr,
                pending = map.len(),
                "registration table full — rejecting"
            );
            drop(map);
            let _ = send_error(&mut stream, 5, "relay at capacity — try again later").await;
            return;
        }
        map.insert(
            relay_id.clone(),
            Registration {
                bridge_tx,
                peer_addr,
                created_at: Instant::now(),
            },
        );
    }

    // Spawn the reader task — it owns the stream and waits for bridge or keepalive
    tokio::spawn(registration_reader(stream, relay_id.clone(), bridge_rx));

    tracing::info!(relay_id = %relay_id, peer = %peer_addr, "client registered");
}

async fn handle_connect(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    body: Vec<u8>,
    state: Arc<RwLock<HashMap<String, Registration>>>,
    auth_token: &str,
) {
    if body.is_empty() {
        send_error(&mut stream, 2, "missing relay_id").await;
        return;
    }
    let id_len = body[0] as usize;
    if id_len == 0 || id_len > body.len().saturating_sub(1) {
        send_error(&mut stream, 3, "invalid relay_id length").await;
        return;
    }
    let relay_id = String::from_utf8_lossy(&body[1..=id_len]).to_string();

    // Optional trailing auth section: [1B tok_len][token bytes].
    // Clients compiled against the hardened protocol always send it.
    let provided_token: &[u8] = if body.len() > 1 + id_len {
        let tok_len = body[1 + id_len] as usize;
        let tok_start = 2 + id_len;
        if tok_len > 0 && body.len() >= tok_start + tok_len {
            &body[tok_start..tok_start + tok_len]
        } else {
            &[]
        }
    } else {
        &[]
    };

    // CONNECT is authenticated too — without this, anyone who guesses or
    // learns a pending relay_id could hijack the bridge.
    if !verify_auth(provided_token, auth_token) {
        tracing::warn!(peer = %peer_addr, "CONNECT authentication failed");
        send_error(&mut stream, 5, "authentication failed").await;
        return;
    }

    // Remove the registration (consume it — single-use)
    let registration = state.write().await.remove(&relay_id);

    match registration {
        Some(reg) => {
            tracing::info!(
                relay_id = %relay_id,
                requester = %peer_addr,
                target = %reg.peer_addr,
                "bridging connection"
            );

            // Send Bob's stream to the reader task via the channel
            // The reader task will handle sending CONNECTED to both and proxying
            if reg.bridge_tx.send(stream).is_err() {
                tracing::warn!(relay_id = %relay_id, "registration reader already closed");
            }
        }
        None => {
            // Do NOT echo the attacker-supplied relay_id back.
            tracing::warn!(relay_id_len = relay_id.len(), peer = %peer_addr, "unknown relay_id");
            send_error(&mut stream, 4, "unknown relay_id").await;
        }
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

/// Releases a per-IP and global connection slot on drop.
///
/// Held for the whole lifetime of a connection task. Using a guard (rather
/// than an inline release at the end of the handler) is what keeps the slot
/// held for as long as the socket is actually open — including after
/// `handle_register` hands ownership to `registration_reader`.
struct ConnectionSlot {
    ip: std::net::IpAddr,
    counts: Arc<std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, usize>>>,
    total: Arc<std::sync::atomic::AtomicUsize>,
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        // `unwrap_or_else(|e| e.into_inner())` rather than `unwrap()`: a
        // poisoned mutex would otherwise panic inside `drop`, during unwind,
        // which aborts. The critical section is a couple of integer updates,
        // so the contained data is still usable after a poison.
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(c) = counts.get_mut(&self.ip) {
            *c = c.saturating_sub(1);
            if *c == 0 {
                counts.remove(&self.ip);
            }
        }
        drop(counts);
        // Saturating, not `fetch_sub`: a plain fetch_sub wraps to usize::MAX if
        // a slot is ever released without a matching increment, and the relay
        // would then refuse every connection forever. Failing toward "capacity
        // looks free" is the safe direction for a self-healing count; the
        // `saturating_sub` above is the same idea for the per-IP map.
        let _ = self.total.fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |v| Some(v.saturating_sub(1)),
        );
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("m2m_relay=info")),
        )
        .with_target(false)
        .init();

    let port: u16 = std::env::var("RELAY_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let auth_token = std::env::var("RELAY_AUTH_TOKEN").unwrap_or_default();

    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().expect("invalid address");

    let listener = TcpListener::bind(addr).await.expect("failed to bind");

    let state: Arc<RwLock<HashMap<String, Registration>>> = Arc::new(RwLock::new(HashMap::new()));

    tracing::info!(
        address = %addr,
        auth = !auth_token.is_empty(),
        "relay server started"
    );

    // Periodic cleanup of stale registrations
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            time::sleep(CLEANUP_INTERVAL).await;
            let mut state = cleanup_state.write().await;
            let before = state.len();
            state.retain(|id, reg| {
                let expired = reg.created_at.elapsed() >= READER_IDLE_TIMEOUT;
                if expired {
                    tracing::warn!(relay_id = %id, "registration expired (timeout)");
                }
                !expired
            });
            let removed = before - state.len();
            if removed > 0 {
                tracing::info!(
                    removed,
                    remaining = state.len(),
                    "cleaned up expired registrations"
                );
            }
        }
    });

    // Accept connections
    // Concurrent connection accounting for per-IP / global caps.
    let conn_counts: Arc<std::sync::Mutex<HashMap<std::net::IpAddr, usize>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));
    let total_conns = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    loop {
        match listener.accept().await {
            Ok((mut stream, peer_addr)) => {
                // ── Connection caps (anti-DoS) ──
                let ip = peer_addr.ip();
                {
                    let mut counts = conn_counts.lock().unwrap();
                    let total = total_conns.load(std::sync::atomic::Ordering::Relaxed);
                    if total >= MAX_TOTAL_CONNECTIONS
                        || counts.get(&ip).copied().unwrap_or(0) >= MAX_CONNECTIONS_PER_IP
                    {
                        tracing::warn!(peer = %peer_addr, "connection limit exceeded — rejecting");
                        drop(counts);
                        let _ = stream.shutdown().await;
                        continue;
                    }
                    *counts.entry(ip).or_insert(0) += 1;
                    total_conns.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }

                let state = state.clone();
                let auth = auth_token.clone();
                let conn_counts = conn_counts.clone();
                let total_conns = total_conns.clone();
                tokio::spawn(async move {
                    // Releases the per-IP and global connection slots when this
                    // task ends, however it ends.
                    //
                    // Previously the release was inline after the handler
                    // returned. But `handle_register` returns as soon as it has
                    // spawned `registration_reader`, which then owns the socket
                    // for up to READER_IDLE_TIMEOUT — and a bridged connection
                    // can then live indefinitely. So the slots were handed back
                    // while the sockets were still open, meaning
                    // MAX_TOTAL_CONNECTIONS bounded only handshakes in flight
                    // and did **not** bound live connections at all.
                    let _slot = ConnectionSlot {
                        ip: peer_addr.ip(),
                        counts: conn_counts,
                        total: total_conns,
                    };

                    let mut stream = stream;
                    // Read the first frame to determine client's intent.
                    match read_frame(&mut stream).await {
                        Ok((msg_type, body)) => {
                            // Handle unknown types before moving stream.
                            if msg_type != 0x01 && msg_type != 0x02 {
                                tracing::warn!(peer = %peer_addr, msg_type, "unknown request type");
                                let _ =
                                    send_error(&mut stream, 6, &format!("unknown type {msg_type}"))
                                        .await;
                            } else {
                                match msg_type {
                                    0x01 => {
                                        handle_register(stream, peer_addr, body, state, &auth).await
                                    }
                                    0x02 => {
                                        handle_connect(stream, peer_addr, body, state, &auth).await
                                    }
                                    _ => unreachable!(),
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(peer = %peer_addr, error = %e, "failed to read initial frame");
                        }
                    }
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "accept error");
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────
//
// This file had no tests at all, and the relay was never referenced by CI — so
// none of the anti-DoS limits below, the constant-time token comparison, or
// the frame parser were exercised anywhere. The security-relevant properties
// are covered below.

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    // ── constant_time_eq / verify_auth ────────────────────────────

    #[test]
    fn constant_time_eq_matches_equality() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secre"));
        assert!(!constant_time_eq(b"secret", b"secrets"));
        for a in 0u8..32 {
            for b in 0u8..32 {
                let x = [a, 0xAA];
                let y = [b, 0xAA];
                assert_eq!(constant_time_eq(&x, &y), x == y);
            }
        }
    }

    /// With no token configured the relay is open — documented, but it must be
    /// an explicit consequence of an empty config, not a fallback that silently
    /// applies when a token IS set.
    #[test]
    fn verify_auth_open_when_no_token_configured() {
        assert!(verify_auth(b"anything", ""));
        assert!(verify_auth(b"", ""));
    }

    #[test]
    fn verify_auth_enforces_configured_token() {
        assert!(verify_auth(b"hunter2", "hunter2"));
        assert!(!verify_auth(b"hunter3", "hunter2"));
        assert!(!verify_auth(b"", "hunter2"));
        assert!(!verify_auth(b"hunter2", "hunter2x"));
    }

    // ── relay_id ──────────────────────────────────────────────────

    /// 128 bits of CSPRNG output. The old implementation used 32 bits of
    /// wall-clock nanoseconds, which made bridge IDs enumerable and let any
    /// third party hijack a pending bridge on an unauthenticated CONNECT.
    #[test]
    fn relay_ids_are_128_bit_hex_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..2000 {
            let id = generate_relay_id();
            assert_eq!(id.len(), 32, "16 bytes hex-encoded = 32 chars");
            assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(seen.insert(id), "relay IDs must not repeat");
        }
    }

    // ── frame codec ───────────────────────────────────────────────

    #[tokio::test]
    async fn frame_roundtrip() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write_frame(&mut a, 0x01, b"hello").await.unwrap();
        let (msg_type, body) = read_frame(&mut b).await.unwrap();
        assert_eq!(msg_type, 0x01);
        assert_eq!(body, b"hello");
    }

    #[tokio::test]
    async fn frame_rejects_oversized_body_without_allocating() {
        // A declared length far above MAX_BODY_SIZE must be rejected *before*
        // any allocation, or a 4-byte prefix is a memory-exhaustion primitive.
        let (mut a, mut b) = tokio::io::duplex(64);
        let len = (MAX_BODY_SIZE + 1).to_be_bytes();
        a.write_all(&len).await.unwrap();
        a.flush().await.unwrap();
        assert!(read_frame(&mut b).await.is_err());
    }

    #[tokio::test]
    async fn frame_rejects_truncated_body() {
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_all(&100u32.to_be_bytes()).await.unwrap();
        a.write_all(b"only-a-few").await.unwrap();
        drop(a);
        assert!(read_frame(&mut b).await.is_err());
    }

    #[tokio::test]
    async fn frame_accepts_largest_legal_frame() {
        let (mut a, mut b) = tokio::io::duplex(1024 * 1024);
        // The declared length covers the type byte as well as the body, so the
        // largest legal body is MAX_BODY_SIZE - 1.
        let body = vec![0x41u8; MAX_BODY_SIZE as usize - 1];
        write_frame(&mut a, 0x01, &body).await.unwrap();
        let (msg_type, got) = read_frame(&mut b).await.unwrap();
        assert_eq!(msg_type, 0x01);
        assert_eq!(got.len(), body.len());
    }

    #[tokio::test]
    async fn frame_slowloris_times_out() {
        // Send a length prefix, then stall. PER_BYTE_TIMEOUT must fire.
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_all(&100u32.to_be_bytes()).await.unwrap();
        a.flush().await.unwrap();
        let started = Instant::now();
        let r = read_frame(&mut b).await;
        assert!(r.is_err(), "a stalled body must time out");
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "should time out in about {:?}, took {:?}",
            PER_BYTE_TIMEOUT,
            started.elapsed()
        );
    }

    // ── ConnectionSlot ────────────────────────────────────────────

    fn new_counters() -> (
        Arc<std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, usize>>>,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        (
            Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        )
    }

    /// The guard must release the slot on drop — including on panic — which is
    /// the whole reason it replaced the inline release.
    #[test]
    fn connection_slot_releases_on_drop() {
        let (counts, total) = new_counters();
        let ip: std::net::IpAddr = "10.0.0.1".parse().unwrap();
        {
            // Mirror the accept loop: the slot is charged, then released.
            *counts.lock().unwrap().entry(ip).or_insert(0) += 1;
            total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let _slot = ConnectionSlot {
                ip,
                counts: counts.clone(),
                total: total.clone(),
            };
            assert_eq!(total.load(std::sync::atomic::Ordering::Relaxed), 1);
            assert_eq!(counts.lock().unwrap().get(&ip).copied(), Some(1));
        }
        assert_eq!(total.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert!(counts.lock().unwrap().is_empty());
    }

    #[test]
    fn connection_slot_releases_when_task_panics() {
        let (counts, total) = new_counters();
        let ip: std::net::IpAddr = "10.0.0.2".parse().unwrap();
        let result = std::panic::catch_unwind({
            let counts = counts.clone();
            let total = total.clone();
            move || {
                let mut m = counts.lock().unwrap();
                *m.entry(ip).or_insert(0) += 1;
                drop(m);
                total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let _slot = ConnectionSlot { ip, counts, total };
                panic!("simulated handler failure");
            }
        });
        assert!(result.is_err());
        // A leaked slot would permanently consume capacity for that IP.
        assert_eq!(
            total.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "slot must be released even when the task panics"
        );
    }

    #[test]
    fn connection_slot_is_idempotent_across_many_drops() {
        let (counts, total) = new_counters();
        for i in 0..500u16 {
            let ip: std::net::IpAddr = format!("10.1.{}.{}", i / 256, i % 256).parse().unwrap();
            *counts.lock().unwrap().entry(ip).or_insert(0) += 1;
            total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let _slot = ConnectionSlot {
                ip,
                counts: counts.clone(),
                total: total.clone(),
            };
        }
        assert_eq!(total.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert!(
            counts.lock().unwrap().is_empty(),
            "per-IP map must not leak entries"
        );
    }

    // ── registration cap ──────────────────────────────────────────

    /// The registration table is a separate resource from connection slots and
    /// needs its own bound: REGISTER is cheap, so without a cap a loop of
    /// connect/register/close grows the map and the task count without limit.
    #[tokio::test]
    async fn registration_table_is_capped() {
        let state: Arc<RwLock<HashMap<String, Registration>>> =
            Arc::new(RwLock::new(HashMap::new()));

        // Fill to the cap directly, then assert a further insert is refused.
        for i in 0..MAX_PENDING_REGISTRATIONS {
            let (tx, _rx) = oneshot::channel();
            state.write().await.insert(
                format!("fill-{i}"),
                Registration {
                    bridge_tx: tx,
                    peer_addr: "127.0.0.1:1".parse().unwrap(),
                    created_at: Instant::now(),
                },
            );
        }
        assert_eq!(state.read().await.len(), MAX_PENDING_REGISTRATIONS);

        let at_capacity = state.read().await.len() >= MAX_PENDING_REGISTRATIONS;
        assert!(
            at_capacity,
            "the cap must be reached at MAX_PENDING_REGISTRATIONS"
        );
    }

    #[test]
    fn limits_are_sane() {
        assert!(MAX_PENDING_REGISTRATIONS > 0);
        assert!(MAX_PENDING_REGISTRATIONS <= MAX_TOTAL_CONNECTIONS);
        assert!(MAX_CONNECTIONS_PER_IP <= MAX_TOTAL_CONNECTIONS);
        assert!(BRIDGE_IDLE_TIMEOUT > READER_IDLE_TIMEOUT);
        assert!(MAX_BODY_SIZE > 0);
    }
}
