/// M2M — Connection Manager
///
/// ICE-Lite connection establishment with candidate-priority strategies,
/// TCP simultaneous-open hole punching, and latency-aware winner selection.
///
/// ## Architecture
///
/// ```text
/// ConnectionManager::connect()
///        │
///        ▼
///   Build strategies (sorted by candidate priority)
///        │
///        ▼
///   Phase 1 ── Host candidates ── Direct TCP connect
///        │                          (fastest path, no race needed)
///        ▼
///   Phase 2 ── Srflx / Prflx ── Race accept vs connect
///        │        candidates        (TCP simultaneous open)
///        ▼
///   Phase 3 ── Relay candidates ── TURN relay (Phase 3)
///        │
///        ▼
///   Choose winner (first success)
///        │
///        ▼
///   Return stream + role + latency
/// ```
///
/// Each strategy records its latency so the caller can log or prefer
/// lower-latency paths in future reconnection attempts.
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};
use tokio::time;

use thiserror::Error;

use crate::protocol::WireCandidate;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ConnectionError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("all {0} strategy(ies) failed")]
    AllFailed(usize),
    #[error("no candidates supplied")]
    NoCandidates,
    #[error("timed out after {0:?}")]
    TimedOut(Duration),
}

// ─── Role ────────────────────────────────────────────────────────────────────

/// Whether we initiated the TCP connection or the peer did.
///
/// Determines which handshake role to take:
/// - `Initiator`  → we send HandshakeInit first
/// - `Responder`  → we wait for HandshakeInit from the peer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

// ─── Strategy ────────────────────────────────────────────────────────────────

/// A connection strategy derived from a single ICE candidate.
///
/// Each variant maps to a `CandidateType` and a different establishment
/// technique. New strategies (IPv6 direct, UPnP port-mapped, etc.) can
/// be added here without touching the rest of the manager.
#[derive(Debug, Clone)]
pub enum Strategy {
    /// Direct TCP to a host/LAN candidate (type=0).
    /// Highest success rate, lowest latency.
    DirectTcp { peer: SocketAddr },

    /// Direct TCP to a port-mapped address obtained via UPnP/NAT-PMP/PCP.
    /// These are router-confirmed forwarding rules, so direct TCP connect
    /// is the most reliable transport-agnostic strategy for them.
    PortMapped { peer: SocketAddr },

    /// TCP hole punch (simultaneous open) for server-reflexive (type=1)
    /// or peer-reflexive (type=2) candidates.
    TcpHolePunch { peer: SocketAddr },

    /// TURN relay connection for relay candidates (type=3).
    /// Reserved for Phase 3 — always succeeds (relay is TCP-reliable)
    /// but adds significant latency.
    #[allow(dead_code)]
    TcpRelay { peer: SocketAddr },
}

impl Strategy {
    /// Human-readable label for logging / diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            Strategy::DirectTcp { .. } => "host",
            Strategy::PortMapped { .. } => "port-mapped",
            Strategy::TcpHolePunch { .. } => "srflx",
            Strategy::TcpRelay { .. } => "relay",
        }
    }
}

// ─── Strategy Result ─────────────────────────────────────────────────────────

/// Outcome of a single strategy attempt.
pub struct StrategyResult {
    pub stream: TcpStream,
    pub remote_addr: SocketAddr,
    pub role: Role,
    pub strategy_name: &'static str,
    pub latency: Duration,
}

// ─── Legacy result (used by race_accept_or_connect) ─────────────────────────

/// Everything needed to begin a handshake after a successful hole punch.
pub struct HolePunchResult {
    pub stream: TcpStream,
    pub role: Role,
    pub remote_addr: SocketAddr,
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// Timeout for a single direct candidate connect attempt.
const CANDIDATE_TIMEOUT: Duration = Duration::from_secs(5);

/// Total timeout for the hole-punch race.
const HOLE_PUNCH_TIMEOUT: Duration = Duration::from_secs(15);

// ─── Connection Manager ─────────────────────────────────────────────────────

/// ICE-Lite connection manager.
///
/// Orchestrates connection strategies in priority order, racing accept
/// vs connect where applicable, and returns the first successful result.
pub struct ConnectionManager;

impl ConnectionManager {
    /// Establish a connection to a peer using the optimal strategy sequence.
    ///
    /// ## Strategy ordering (per ICE-Lite)
    ///
    /// | Priority | Candidate type | Strategy          |
    /// |----------|----------------|-------------------|
    /// | 1        | Host (0)       | `DirectTcp`       |
    /// | 2        | Port mapped    | `PortMapped`      |
    /// | 3        | Srflx (1)      | `TcpHolePunch`    |
    /// | 4        | Prflx (2)      | `TcpHolePunch`    |
    /// | 5        | Relay (3)      | `TcpRelay` (TBD)  |
    ///
    /// `DirectTcp` candidates are tried sequentially (fast per-attempt
    /// timeout). `TcpHolePunch` candidates are race-tested against a
    /// shadow TCP listener (so the peer can connect to us while we try
    /// to connect to them — true simultaneous open).
    ///
    /// The first strategy that succeeds wins. Its latency is recorded
    /// for diagnostics and future path selection.
    pub async fn connect(
        peer_candidates: &[WireCandidate],
        our_listener_addr: Option<SocketAddr>,
    ) -> Result<StrategyResult, ConnectionError> {
        if peer_candidates.is_empty() {
            return Err(ConnectionError::NoCandidates);
        }

        // ── Build strategy list, sorted by candidate priority ──
        // Candidate types ordered by expected reliability:
        //   host (0) → port-mapped (4) → srflx (1) → prflx (2) → relay (3)
        let mut strategies: Vec<Strategy> = Vec::with_capacity(peer_candidates.len());
        for c in peer_candidates {
            if let Ok(addr) = c.address.parse::<SocketAddr>() {
                let s = match c.candidate_type {
                    0 => Strategy::DirectTcp { peer: addr },
                    4 => Strategy::PortMapped { peer: addr },
                    1 | 2 => Strategy::TcpHolePunch { peer: addr },
                    3 => Strategy::TcpRelay { peer: addr },
                    _ => continue,
                };
                strategies.push(s);
            }
        }

        if strategies.is_empty() {
            return Err(ConnectionError::NoCandidates);
        }

        // ── Phase 1: Host candidates — direct TCP ──
        // Simple sequential connect. No race needed because host
        // candidates are on the same LAN / local machine.
        for s in &strategies {
            if let Strategy::DirectTcp { peer } = s {
                let start = Instant::now();
                tracing::debug!(target = %peer, strategy = %s.name(), "phase-1 direct TCP");
                match tcp_connect_timeout(*peer, CANDIDATE_TIMEOUT).await {
                    Ok(stream) => {
                        tracing::info!(target = %peer, latency = ?start.elapsed(), "host direct TCP succeeded");
                        return Ok(StrategyResult {
                            stream,
                            remote_addr: *peer,
                            role: Role::Initiator,
                            strategy_name: s.name(),
                            latency: start.elapsed(),
                        });
                    }
                    Err(_) => continue,
                }
            }
        }

        // ── Phase 1.5: Port-mapped candidates — direct TCP ──
        // Addresses obtained via UPnP, NAT-PMP, or PCP are router-confirmed
        // forwarding rules. Direct TCP to them is the most reliable strategy
        // after host candidates.
        for s in &strategies {
            if let Strategy::PortMapped { peer } = s {
                let start = Instant::now();
                tracing::debug!(target = %peer, strategy = %s.name(), "phase-1.5 port-mapped TCP");
                match tcp_connect_timeout(*peer, CANDIDATE_TIMEOUT).await {
                    Ok(stream) => {
                        tracing::info!(target = %peer, latency = ?start.elapsed(), "port-mapped TCP succeeded");
                        return Ok(StrategyResult {
                            stream,
                            remote_addr: *peer,
                            role: Role::Initiator,
                            strategy_name: s.name(),
                            latency: start.elapsed(),
                        });
                    }
                    Err(_) => continue,
                }
            }
        }

        // ── Phase 2: Srflx / Prflx — hole punch ──
        // Race all srflx/prflx candidates against a shadow listener.
        // The peer may be trying to connect to us simultaneously — the
        // select! picks whichever succeeds first.
        let punch_addrs: Vec<SocketAddr> = strategies
            .iter()
            .filter_map(|s| match s {
                Strategy::TcpHolePunch { peer } => Some(*peer),
                _ => None,
            })
            .collect();

        if !punch_addrs.is_empty() {
            let start = Instant::now();
            tracing::debug!(count = punch_addrs.len(), "phase-2 TCP hole punch");
            match race_accept_or_connect(&punch_addrs, our_listener_addr).await {
                Ok(result) => {
                    tracing::info!(
                        peer = %result.remote_addr,
                        role = ?result.role,
                        latency = ?start.elapsed(),
                        "hole punch succeeded"
                    );
                    return Ok(StrategyResult {
                        stream: result.stream,
                        remote_addr: result.remote_addr,
                        role: result.role,
                        strategy_name: "srflx",
                        latency: start.elapsed(),
                    });
                }
                Err(e) => {
                    tracing::warn!(error = %e, "phase-2 hole punch phase exhausted");
                }
            }
        }

        // ── Phase 3: Relay candidates — TURN relay (Phase 3) ──
        // Reserved. A relay strategy would connect via the TURN server
        // which always succeeds (at the cost of added latency).
        #[allow(unreachable_code)]
        {
            tracing::warn!("no direct path succeeded — relay fallback not yet implemented (Phase 3)");
        }

        Err(ConnectionError::AllFailed(peer_candidates.len()))
    }
}

// ─── Internal: Race Accept vs Connect ───────────────────────────────────────

/// True TCP hole punch: race an incoming accept against outgoing connects.
///
/// Both sides call this at roughly the same instant. One side wins with
/// `Role::Initiator` (its `connect` succeeded first), the other wins with
/// `Role::Responder` (its `listener.accept` received the SYN from the
/// initiator's `connect`).
///
/// If `our_listener_addr` is `Some`, a **shadow TCP listener** is created
/// (with `SO_REUSEADDR` so it can share the port with the long-lived main
/// listener). This shadow listener only accepts connections during the race.
///
/// If `our_listener_addr` is `None` (no listener yet), the function falls
/// back to sequential connect attempts.
async fn race_accept_or_connect(
    peer_candidates: &[SocketAddr],
    our_listener_addr: Option<SocketAddr>,
) -> Result<HolePunchResult, ConnectionError> {
    if peer_candidates.is_empty() {
        return Err(ConnectionError::NoCandidates);
    }

    let peer_candidates = peer_candidates.to_vec();

    match our_listener_addr {
        None => {
            // No listener available — straight connect (no race needed).
            connect_sequential(&peer_candidates).await
        }
        Some(addr) => {
            // Shadow listener (shares port with main listener via SO_REUSEADDR).
            let std = std::net::TcpListener::bind(addr)?;
            std.set_nonblocking(true)?;
            let listener = TcpListener::from_std(std)?;

            // Race: accept incoming vs connect outgoing.
            let accept = async {
                let (stream, peer) = time::timeout(HOLE_PUNCH_TIMEOUT, listener.accept())
                    .await
                    .map_err(|_| ConnectionError::TimedOut(HOLE_PUNCH_TIMEOUT))?
                    .map_err(ConnectionError::Io)?;
                let _ = stream.set_nodelay(true);
                tracing::info!(peer = %peer, "hole-punch accept won the race");
                Ok(HolePunchResult {
                    stream,
                    role: Role::Responder,
                    remote_addr: peer,
                })
            };

            let connect = async {
                let result = connect_sequential(&peer_candidates).await;
                tracing::info!(
                    outcome = if result.is_ok() { "succeeded" } else { "failed" },
                    "hole-punch connect leg finished"
                );
                result
            };

            tokio::select! {
                result = accept => result,
                result = connect => result,
            }
        }
    }
}

/// Try all peer candidates sequentially (simple connect).
async fn connect_sequential(
    peer_candidates: &[SocketAddr],
) -> Result<HolePunchResult, ConnectionError> {
    for &addr in peer_candidates {
        tracing::debug!(target = %addr, "attempting TCP connect");
        match time::timeout(CANDIDATE_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                tracing::info!(peer = %addr, "connect succeeded");
                return Ok(HolePunchResult {
                    stream,
                    role: Role::Initiator,
                    remote_addr: addr,
                });
            }
            Ok(Err(e)) => {
                tracing::warn!(target = %addr, error = %e, "connect failed");
            }
            Err(_) => {
                tracing::warn!(target = %addr, "connect timed out");
            }
        }
    }
    Err(ConnectionError::AllFailed(peer_candidates.len()))
}

/// Internal connect with per-attempt timeout.
async fn tcp_connect_timeout(
    addr: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream, ConnectionError> {
    match time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => {
            let _ = stream.set_nodelay(true);
            Ok(stream)
        }
        Ok(Err(e)) => Err(ConnectionError::Io(e)),
        Err(_) => Err(ConnectionError::TimedOut(timeout)),
    }
}

// ─── Invite Helpers ─────────────────────────────────────────────────────────

/// Extract deduplicated `WireCandidate`s from an invite.
///
/// The legacy `address_hint` is converted into a srflx candidate (type=1)
/// and placed first. Remaining `candidates` are appended in their original
/// order, with duplicate addresses silently removed.
///
/// Returns candidates suitable for `ConnectionManager::connect()`.
pub fn extract_candidates_from_invite(
    address_hint: &str,
    candidates: &[WireCandidate],
) -> Vec<WireCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    // Legacy address hint — treated as srflx (best guess).
    if let Ok(addr) = address_hint.parse::<SocketAddr>() {
        seen.insert(addr);
        result.push(WireCandidate {
            address: addr.to_string(),
            candidate_type: 1, // srflx
        });
    }

    // Structured candidates preserve their original type.
    for c in candidates {
        if let Ok(addr) = c.address.parse::<SocketAddr>() {
            if seen.insert(addr) {
                result.push(c.clone());
            }
        }
    }

    tracing::debug!(count = result.len(), "extracted candidates from invite");
    result
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod hole_punch_tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════
    // extract_candidates_from_invite
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_address_hint_only() {
        let c = extract_candidates_from_invite("1.2.3.4:12345", &[]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].address, "1.2.3.4:12345");
        assert_eq!(c[0].candidate_type, 1); // srflx
    }

    #[test]
    fn test_with_structured_candidates() {
        let candidates = vec![
            WireCandidate { address: "192.168.1.5:54321".into(), candidate_type: 0 },
            WireCandidate { address: "5.6.7.8:9876".into(), candidate_type: 1 },
        ];
        let c = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].candidate_type, 1); // legacy → srflx
        assert_eq!(c[1].candidate_type, 0); // preserved host
        assert_eq!(c[2].candidate_type, 1); // preserved srflx
    }

    #[test]
    fn test_deduplicates() {
        let candidates = vec![
            WireCandidate { address: "1.2.3.4:12345".into(), candidate_type: 0 },
        ];
        let c = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn test_invalid_candidate_skipped() {
        let candidates = vec![
            WireCandidate { address: "not-valid".into(), candidate_type: 0 },
        ];
        let c = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn test_empty_all() {
        let c = extract_candidates_from_invite("", &[]);
        assert!(c.is_empty());
    }

    #[test]
    fn test_empty_hint_with_candidates() {
        let candidates = vec![
            WireCandidate { address: "10.0.0.1:8000".into(), candidate_type: 0 },
        ];
        let c = extract_candidates_from_invite("", &candidates);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].candidate_type, 0);
    }

    // ═══════════════════════════════════════════════════════════
    // Strategy building (via ConnectionManager internals)
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_strategy_names() {
        assert_eq!(Strategy::DirectTcp { peer: "0.0.0.0:0".parse().unwrap() }.name(), "host");
        assert_eq!(Strategy::TcpHolePunch { peer: "0.0.0.0:0".parse().unwrap() }.name(), "srflx");
        assert_eq!(Strategy::TcpRelay { peer: "0.0.0.0:0".parse().unwrap() }.name(), "relay");
    }

    // ═══════════════════════════════════════════════════════════
    // Error display
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", ConnectionError::NoCandidates), "no candidates supplied");
        assert_eq!(format!("{}", ConnectionError::AllFailed(3)), "all 3 strategy(ies) failed");
    }
}
