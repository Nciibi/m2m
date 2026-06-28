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
///   - 0x02 CONNECT   body=[1B id_len][relay_id] — request bridge to peer
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
/// RELAY_PORT=3478 cargo run --example relay-server
/// RELAY_AUTH_TOKEN=secret cargo run --example relay-server
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

async fn read_frame(stream: &mut TcpStream) -> Result<(u8, Vec<u8>), String> {
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

async fn write_frame(stream: &mut TcpStream, msg_type: u8, body: &[u8]) -> Result<(), String> {
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

fn generate_relay_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32;
    let bytes = [ts as u8, (ts >> 8) as u8, (ts >> 16) as u8, (ts >> 24) as u8];
    hex::encode(bytes)
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

                        // Enter raw TCP proxy mode
                        match tokio::io::copy_bidirectional(&mut alice_stream, &mut bob_stream).await {
                            Ok((a_to_b, b_to_a)) => {
                                tracing::info!(
                                    relay_id = %relay_id,
                                    sent = a_to_b,
                                    received = b_to_a,
                                    "relay connection closed normally"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(relay_id = %relay_id, error = %e, "relay proxy error");
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
        let provided = String::from_utf8_lossy(&auth_body);
        if provided.trim() != auth_token {
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

    // Store the registration (the sender side, to deliver Bob's stream)
    state.write().await.insert(
        relay_id.clone(),
        Registration {
            bridge_tx,
            peer_addr,
            created_at: Instant::now(),
        },
    );

    // Spawn the reader task — it owns the stream and waits for bridge or keepalive
    tokio::spawn(registration_reader(stream, relay_id.clone(), bridge_rx));

    tracing::info!(relay_id = %relay_id, peer = %peer_addr, "client registered");
}

async fn handle_connect(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    body: Vec<u8>,
    state: Arc<RwLock<HashMap<String, Registration>>>,
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
                send_error(&mut stream, 7, "registration target disconnected").await;
            }
        }
        None => {
            tracing::warn!(relay_id = %relay_id, peer = %peer_addr, "unknown relay_id");
            send_error(&mut stream, 4, &format!("unknown relay_id: {relay_id}")).await;
        }
    }
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("relay_server=info")),
        )
        .with_target(false)
        .init();

    let port: u16 = std::env::var("RELAY_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    let auth_token = std::env::var("RELAY_AUTH_TOKEN").unwrap_or_default();

    let addr: SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .expect("invalid address");

    let listener = TcpListener::bind(addr).await.expect("failed to bind");

    let state: Arc<RwLock<HashMap<String, Registration>>> =
        Arc::new(RwLock::new(HashMap::new()));

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
                tracing::info!(removed, remaining = state.len(), "cleaned up expired registrations");
            }
        }
    });

    // Accept connections
    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let state = state.clone();
                let auth = auth_token.clone();
                tokio::spawn(async move {
                    // Read the first frame to determine client's intent
                    match read_frame(&mut stream).await {
                        Ok((msg_type, body)) => {
                            match msg_type {
                                0x01 => handle_register(stream, peer_addr, body, state, &auth).await,
                                0x02 => handle_connect(stream, peer_addr, body, state).await,
                                other => {
                                    tracing::warn!(peer = %peer_addr, msg_type = other, "unknown request");
                                    let _ = send_error(&mut stream, 6, &format!("unknown type {other}")).await;
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
