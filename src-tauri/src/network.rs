/// M2M — Network Module
///
/// TCP transport with length-prefixed framing, connection state machine,
/// timeouts, heartbeats, rate limiting, and graceful disconnect.
///
/// All data crossing the network boundary is treated as untrusted.
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{self, Instant};

use thiserror::Error;

use crate::protocol::{
    self, validate_frame_size, validate_version, PacketType, MAX_FRAME_SIZE, LENGTH_PREFIX_SIZE,
    HEARTBEAT_INTERVAL_SECS, HEARTBEAT_TIMEOUT_SECS,
};

/// Network operation timeout for reads/writes.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);

/// TCP connection timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum number of queued incoming connections.
const LISTENER_BACKLOG: u32 = 8;

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

/// Read exactly one length-prefixed frame from a TCP stream.
/// Validates frame size and protocol version before returning.
pub async fn read_frame(stream: &mut TcpStream) -> Result<RawFrame, NetworkError> {
    // Read the 4-byte length prefix
    let mut len_buf = [0u8; LENGTH_PREFIX_SIZE];
    match time::timeout(NETWORK_TIMEOUT, stream.read_exact(&mut len_buf)).await {
        Ok(Ok(())) => {}
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
    match time::timeout(NETWORK_TIMEOUT, stream.read_exact(&mut payload)).await {
        Ok(Ok(())) => {}
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

/// Write a complete frame to a TCP stream.
pub async fn write_frame(
    stream: &mut TcpStream,
    packet_type: PacketType,
    body: &[u8],
) -> Result<(), NetworkError> {
    let frame = protocol::build_frame(packet_type, body)?;

    match time::timeout(NETWORK_TIMEOUT, stream.write_all(&frame)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(NetworkError::Io(e)),
        Err(_) => return Err(NetworkError::WriteTimeout),
    }

    match time::timeout(NETWORK_TIMEOUT, stream.flush()).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(NetworkError::Io(e)),
        Err(_) => return Err(NetworkError::WriteTimeout),
    }

    Ok(())
}

/// Start a TCP listener on the given address.
/// Returns accepted connections via the channel.
pub async fn start_listener(
    addr: SocketAddr,
    tx: mpsc::Sender<(TcpStream, SocketAddr)>,
) -> Result<(), NetworkError> {
    let listener = TcpListener::bind(addr).await?;
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
pub async fn connect(addr: SocketAddr) -> Result<TcpStream, NetworkError> {
    let stream = time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| NetworkError::ConnectionTimeout)?
        .map_err(NetworkError::Io)?;

    tracing::debug!("connected to remote peer");
    Ok(stream)
}

/// Send a heartbeat packet.
pub async fn send_heartbeat(stream: &mut TcpStream) -> Result<(), NetworkError> {
    write_frame(stream, PacketType::Heartbeat, &[]).await
}

/// Send a heartbeat acknowledgment.
pub async fn send_heartbeat_ack(stream: &mut TcpStream) -> Result<(), NetworkError> {
    write_frame(stream, PacketType::HeartbeatAck, &[]).await
}

/// Send a disconnect packet with reason.
pub async fn send_disconnect(
    stream: &mut TcpStream,
    reason: protocol::DisconnectReason,
) -> Result<(), NetworkError> {
    let msg = protocol::DisconnectMessage { reason };
    let body = protocol::serialize(&msg)?;
    write_frame(stream, PacketType::Disconnect, &body).await
}

/// Send an error packet.
pub async fn send_error(
    stream: &mut TcpStream,
    code: protocol::ErrorCode,
    description: &str,
) -> Result<(), NetworkError> {
    let msg = protocol::ErrorMessage {
        code,
        description: description.to_string(),
    };
    let body = protocol::serialize(&msg)?;
    write_frame(stream, PacketType::Error, &body).await
}
