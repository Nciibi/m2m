/// M2M — Tor Proxy Module
///
/// Provides SOCKS5 proxy support for routing TCP connections through
/// the Tor network. When enabled, all outgoing peer connections are
/// routed through the local Tor daemon (127.0.0.1:9050), making the
/// user's real IP address mathematically impossible to discover.
///
/// Prerequisites:
/// - Tor must be installed and running locally on port 9050.
/// - For hidden services, the user must configure Tor separately.
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::net::TcpStream;
use tokio_socks::tcp::Socks5Stream;
use thiserror::Error;

/// Default Tor SOCKS5 proxy address.
pub const TOR_PROXY_ADDR: &str = "127.0.0.1:9050";

/// Global flag for whether Tor routing is enabled.
static TOR_ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error)]
#[expect(dead_code, reason = "Reserved; used only by tor::connect")]
pub enum TorError {
    #[error("Tor SOCKS5 connection failed: {0}")]
    ConnectionFailed(String),
    #[expect(dead_code, reason = "Reserved error variant for unreachable proxy")]
    #[error("Tor proxy not reachable at {0}")]
    ProxyUnreachable(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Enable or disable Tor routing for all outgoing connections.
pub fn set_enabled(enabled: bool) {
    TOR_ENABLED.store(enabled, Ordering::SeqCst);
    if enabled {
        tracing::info!("Tor routing ENABLED — all connections will use SOCKS5 proxy");
    } else {
        tracing::info!("Tor routing DISABLED — using direct TCP connections");
    }
}

/// Check if Tor routing is currently enabled.
pub fn is_enabled() -> bool {
    TOR_ENABLED.load(Ordering::SeqCst)
}

/// Connect to a peer, routing through Tor if enabled.
/// Falls back to direct TCP if Tor is disabled.
#[expect(dead_code, reason = "Reserved; used by network::connect")]
pub async fn connect(addr: SocketAddr) -> Result<TcpStream, TorError> {
    if is_enabled() {
        connect_via_tor(addr).await
    } else {
        TcpStream::connect(addr)
            .await
            .map_err(TorError::Io)
    }
}

/// Connect to a target address through the Tor SOCKS5 proxy.
#[expect(dead_code, reason = "Reserved; called only by tor::connect")]
async fn connect_via_tor(target: SocketAddr) -> Result<TcpStream, TorError> {
    tracing::debug!(target = %target, proxy = TOR_PROXY_ADDR, "connecting via Tor SOCKS5");

    let stream = Socks5Stream::connect(TOR_PROXY_ADDR, target)
        .await
        .map_err(|e| TorError::ConnectionFailed(e.to_string()))?;

    Ok(stream.into_inner())
}

/// Check if the Tor proxy is reachable by attempting a TCP connection to it.
pub async fn check_proxy_reachable() -> bool {
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        TcpStream::connect(TOR_PROXY_ADDR),
    )
    .await
    {
        Ok(Ok(_)) => {
            tracing::info!("Tor proxy reachable at {}", TOR_PROXY_ADDR);
            true
        }
        _ => {
            tracing::warn!("Tor proxy NOT reachable at {}", TOR_PROXY_ADDR);
            false
        }
    }
}

/// Network settings state for the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NetworkSettings {
    pub tor_enabled: bool,
    pub tor_proxy_addr: String,
    pub tor_reachable: bool,
    pub public_ip: Option<String>,
}
