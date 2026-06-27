/// M2M — Hole Punch Module
///
/// ICE-Lite connectivity establishment with TCP hole punching,
/// candidate-based connection prioritization, and NAT traversal helpers.
///
/// Provides a strategy-based connection function that tries candidates
/// in priority order and falls back to active TCP simultaneous-open punching
/// and UDP pre-punch for NAT mapping creation.
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::time;

use thiserror::Error;

use crate::protocol::WireCandidate;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum HolePunchError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("all {0} connection attempts failed")]
    AllAttemptsFailed(usize),
    #[error("no candidates available to attempt connection")]
    NoCandidates,
    #[error("connection to {0} timed out")]
    TimedOut(SocketAddr),
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// Timeout for each individual candidate TCP connect attempt.
const CANDIDATE_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for TCP hole punch attempts (shorter than normal connect
/// because both sides are expected to be opening simultaneously).
const HOLE_PUNCH_TIMEOUT: Duration = Duration::from_secs(3);

/// Timeout for UDP prelude send operation.
#[allow(dead_code)]
const UDP_PRELUDE_TIMEOUT: Duration = Duration::from_secs(2);

// ─── Core Functions ─────────────────────────────────────────────────────────

/// Connect to `addr` with a per-candidate timeout.
///
/// This is a lightweight alternative to [`crate::network::connect`] that does
/// not route through Tor and uses a configurable timeout suitable for trying
/// many candidates in succession.
pub async fn tcp_connect_timeout(
    addr: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream, HolePunchError> {
    tracing::debug!(target = %addr, timeout = ?timeout, "attempting TCP connect with timeout");

    match time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => {
            let _ = stream.set_nodelay(true);
            tracing::debug!(target = %addr, "TCP connect succeeded");
            Ok(stream)
        }
        Ok(Err(e)) => {
            tracing::warn!(target = %addr, error = %e, "TCP connect failed");
            Err(HolePunchError::Io(e))
        }
        Err(_) => {
            tracing::warn!(target = %addr, "TCP connect timed out");
            Err(HolePunchError::TimedOut(addr))
        }
    }
}

/// Attempt connection to a peer using the full candidate strategy.
///
/// Strategy (in order of preference):
/// 1. **Host candidates** (type=0) — direct LAN addresses, highest priority.
/// 2. **Server-reflexive candidates** (type=1) — public IP:port from STUN.
/// 3. **TCP hole punch** — simultaneous-open connect on srflx/prflx addresses.
/// 4. **UDP pre-punch + TCP** — send a UDP datagram to create a NAT mapping,
///    then immediately attempt TCP connect.
///
/// Returns the connected `TcpStream` and the `SocketAddr` that succeeded.
pub async fn connect_with_candidates(
    peer_candidates: &[WireCandidate],
) -> Result<(TcpStream, SocketAddr), HolePunchError> {
    if peer_candidates.is_empty() {
        return Err(HolePunchError::NoCandidates);
    }

    // ── Phase 1: Host candidates ──
    // Direct local network addresses — highest probability of success.
    let host_addrs: Vec<SocketAddr> = peer_candidates
        .iter()
        .filter(|c| c.candidate_type == 0)
        .filter_map(|c| c.address.parse::<SocketAddr>().ok())
        .collect();

    for addr in &host_addrs {
        match tcp_connect_timeout(*addr, CANDIDATE_TIMEOUT).await {
            Ok(stream) => return Ok((stream, *addr)),
            Err(_) => continue,
        }
    }

    // ── Phase 2: Server-reflexive candidates ──
    // Public addresses discovered via STUN.
    let srflx_addrs: Vec<SocketAddr> = peer_candidates
        .iter()
        .filter(|c| c.candidate_type == 1)
        .filter_map(|c| c.address.parse::<SocketAddr>().ok())
        .collect();

    for addr in &srflx_addrs {
        match tcp_connect_timeout(*addr, CANDIDATE_TIMEOUT).await {
            Ok(stream) => return Ok((stream, *addr)),
            Err(_) => continue,
        }
    }

    // ── Phase 3: TCP hole punch ──
    // Simultaneous-open connect on srflx and prflx candidates.
    // Both sides call connect() at roughly the same time so their NATs
    // allow the inbound SYN that matches the outbound mapping.
    let punch_addrs: Vec<SocketAddr> = peer_candidates
        .iter()
        .filter(|c| c.candidate_type == 1 || c.candidate_type == 2)
        .filter_map(|c| c.address.parse::<SocketAddr>().ok())
        .collect();

    for addr in &punch_addrs {
        match tcp_hole_punch(*addr, HOLE_PUNCH_TIMEOUT).await {
            Ok(stream) => return Ok((stream, *addr)),
            Err(_) => continue,
        }
    }

    // ── Phase 4: UDP pre-punch + TCP ──
    // Send a UDP datagram to trigger the NAT to create a pin-hole,
    // then immediately follow with a TCP connect. This helps with
    // symmetric and port-restricted NATs where a recent outbound
    // mapping improves the chance of accepting an inbound SYN.
    let pre_punch_addrs: Vec<SocketAddr> = peer_candidates
        .iter()
        .filter_map(|c| c.address.parse::<SocketAddr>().ok())
        .collect();

    for addr in &pre_punch_addrs {
        // Best-effort UDP prelude — errors are non-fatal.
        let _ = udp_prelude(*addr).await;

        match tcp_connect_timeout(*addr, CANDIDATE_TIMEOUT).await {
            Ok(stream) => return Ok((stream, *addr)),
            Err(_) => continue,
        }
    }

    tracing::error!(
        attempted = peer_candidates.len(),
        host = host_addrs.len(),
        srflx = srflx_addrs.len(),
        "all connection candidates exhausted"
    );
    Err(HolePunchError::AllAttemptsFailed(peer_candidates.len()))
}

/// Attempt a TCP simultaneous-open hole punch to `addr`.
///
/// Uses `std::net::TcpStream::connect_timeout` wrapped in
/// `tokio::task::spawn_blocking` to avoid blocking the async runtime.
/// The standard library's `connect_timeout` bypasses the TCP stack's
/// default retransmission backoff, which is essential for the
/// simultaneous-open pattern: both sides call `connect()` at roughly
/// the same instant so their respective NATs allow the inbound SYN
/// that matches the outbound mapping.
pub async fn tcp_hole_punch(
    addr: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream, HolePunchError> {
    tracing::debug!(target = %addr, timeout = ?timeout, "attempting TCP hole punch");

    let result = tokio::task::spawn_blocking(move || {
        std::net::TcpStream::connect_timeout(&addr, timeout)
    })
    .await
    .map_err(|e| {
        HolePunchError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        ))
    })?;

    match result {
        Ok(stream) => {
            let _ = stream.set_nodelay(true);
            // Convert to tokio TcpStream (from_std sets non-blocking mode).
            let tokio_stream = TcpStream::from_std(stream)
                .map_err(|e| HolePunchError::Io(e))?;
            tracing::debug!(target = %addr, "TCP hole punch succeeded");
            Ok(tokio_stream)
        }
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            tracing::warn!(target = %addr, "TCP hole punch timed out");
            Err(HolePunchError::TimedOut(addr))
        }
        Err(e) => {
            tracing::warn!(target = %addr, error = %e, "TCP hole punch failed");
            Err(HolePunchError::Io(e))
        }
    }
}

/// Send a single UDP datagram to `addr` to create a NAT mapping.
///
/// Many NATs create a temporary UDP pin-hole when they observe an outgoing
/// UDP packet. A subsequent TCP SYN from the same internal address to the
/// same external address can sometimes reuse this mapping, improving the
/// chance of a successful TCP connection behind symmetric or port-restricted
/// NATs.
///
/// This is a best-effort operation — failures are logged but not propagated
/// to the caller.
pub async fn udp_prelude(addr: SocketAddr) -> Result<(), HolePunchError> {
    tracing::debug!(target = %addr, "sending UDP prelude");

    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;

    match time::timeout(Duration::from_secs(2), socket.send_to(&[0u8], addr)).await {
        Ok(Ok(n)) => {
            tracing::debug!(target = %addr, bytes_sent = n, "UDP prelude sent");
            Ok(())
        }
        Ok(Err(e)) => {
            tracing::warn!(target = %addr, error = %e, "UDP prelude send failed");
            Err(HolePunchError::Io(e))
        }
        Err(_) => {
            tracing::warn!(target = %addr, "UDP prelude send timed out");
            // Timeout on a UDP send is unusual but not fatal for the caller.
            Ok(())
        }
    }
}

/// Extract `SocketAddr` candidates from an invite's address hint and
/// candidate list.
///
/// Parses the `address_hint` first (the primary advertised address), then
/// appends any additional addresses from `candidates` that parse
/// successfully. Duplicates are removed, and the address hint (when valid)
/// is always first in the returned list.
pub fn extract_candidates_from_invite(
    address_hint: &str,
    candidates: &[WireCandidate],
) -> Vec<SocketAddr> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    // Primary address from the invite hint.
    if let Ok(addr) = address_hint.parse::<SocketAddr>() {
        seen.insert(addr);
        result.push(addr);
    }

    // Additional addresses from handshake candidates.
    for c in candidates {
        if let Ok(addr) = c.address.parse::<SocketAddr>() {
            if seen.insert(addr) {
                result.push(addr);
            }
        }
    }

    tracing::debug!(
        count = result.len(),
        hint_valid = !address_hint.is_empty(),
        "extracted candidates from invite"
    );
    result
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod hole_punch_tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════
    // extract_candidates_from_invite — parsing
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_extract_address_hint_only() {
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &[]);
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "1.2.3.4:12345".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_extract_with_candidates() {
        let candidates = vec![
            WireCandidate { address: "192.168.1.5:54321".to_string(), candidate_type: 0 },
            WireCandidate { address: "5.6.7.8:9876".to_string(), candidate_type: 1 },
        ];
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(addrs.len(), 3);
        assert_eq!(addrs[0], "1.2.3.4:12345".parse::<SocketAddr>().unwrap());
        assert_eq!(addrs[1], "192.168.1.5:54321".parse::<SocketAddr>().unwrap());
        assert_eq!(addrs[2], "5.6.7.8:9876".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_extract_deduplicates() {
        let candidates = vec![
            WireCandidate { address: "1.2.3.4:12345".to_string(), candidate_type: 0 },
            WireCandidate { address: "1.2.3.4:12345".to_string(), candidate_type: 1 },
        ];
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "1.2.3.4:12345".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_extract_invalid_candidate_skipped() {
        let candidates = vec![
            WireCandidate { address: "not-a-valid-addr".to_string(), candidate_type: 0 },
        ];
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "1.2.3.4:12345".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_extract_empty_all() {
        let addrs = extract_candidates_from_invite("", &[]);
        assert!(addrs.is_empty());
    }

    #[test]
    fn test_extract_empty_hint_with_candidates() {
        let candidates = vec![
            WireCandidate { address: "10.0.0.1:8000".to_string(), candidate_type: 0 },
        ];
        let addrs = extract_candidates_from_invite("", &candidates);
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "10.0.0.1:8000".parse::<SocketAddr>().unwrap());
    }

    // ═══════════════════════════════════════════════════════════
    // Error types — display and Debug
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_error_display() {
        let err = HolePunchError::NoCandidates;
        assert_eq!(format!("{err}"), "no candidates available to attempt connection");

        let err = HolePunchError::AllAttemptsFailed(3);
        assert_eq!(format!("{err}"), "all 3 connection attempts failed");

        let err = HolePunchError::TimedOut("1.2.3.4:5678".parse::<SocketAddr>().unwrap());
        assert_eq!(format!("{err}"), "connection to 1.2.3.4:5678 timed out");
    }
}
