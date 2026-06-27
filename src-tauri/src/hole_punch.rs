/// M2M — Hole Punch Module
///
/// ICE-Lite connectivity establishment with true TCP hole punching.
///
/// ## How TCP hole punching works here
///
/// Both peers race two tasks simultaneously:
///
///   tokio::select! {
///       stream = listener.accept() => /* peer connected to us */
///       stream = connect(candidates) => /* we connected to peer */
///   }
///
/// The NAT sees an outbound SYN (from connection attempt) AND an inbound SYN
/// (from listener accept) at roughly the same time. Many NAT implementations
/// allow the inbound SYN because they can match it to the pending outbound
/// SYN mapping (RFC 793 simultaneous open).
///
/// The first peer to successfully connect — in either direction — wins.
/// The role (Initiator vs Responder) is determined by which side of the
/// select succeeded, and determines who sends HandshakeInit first.
use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::time;

use thiserror::Error;

use crate::protocol::WireCandidate;

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum HolePunchError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("all {0} candidate(s) failed")]
    AllFailed(usize),
    #[error("no candidates to try")]
    NoCandidates,
    #[error("hole punch timed out after {0:?}")]
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
    /// Our `connect()` call won the race.
    Initiator,
    /// Our `listener.accept()` call won the race.
    Responder,
}

/// Everything needed to begin a handshake after a successful hole punch.
pub struct HolePunchResult {
    /// The established TCP stream.
    pub stream: TcpStream,
    /// Whether we initiated or accepted (dictates handshake direction).
    pub role: Role,
    /// Address of the remote peer (from the winning socket).
    pub remote_addr: SocketAddr,
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// Timeout for a single candidate connect attempt.
const CANDIDATE_TIMEOUT: Duration = Duration::from_secs(5);

/// Total timeout for the hole-punch race (accept + connect combined).
/// Must be long enough to cover all sequential candidate attempts.
const HOLE_PUNCH_TIMEOUT: Duration = Duration::from_secs(15);

// ─── Core: Race Accept vs Connect ──────────────────────────────────────────

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
// NOTE: In the current single‑invite architecture the Responder's accept
// will not win until the Initiator also learns the Responder's candidates
// and begins connecting back (future phase — signalling channel or relay).
// The race pattern is still correct: it ensures the Responder is ready to
// accept in both directions, which is necessary (but not sufficient) for
// true simultaneous open.
pub async fn race_accept_or_connect(
    peer_candidates: &[SocketAddr],
    our_listener_addr: Option<SocketAddr>,
) -> Result<HolePunchResult, HolePunchError> {
    if peer_candidates.is_empty() {
        return Err(HolePunchError::NoCandidates);
    }

    let peer_candidates = peer_candidates.to_vec();

    match our_listener_addr {
        None => {
            // No listener available — straight connect (no race needed).
            connect_sequential(&peer_candidates).await
        }
        Some(addr) => {
            // ── Shadow listener (shares port with main listener) ──
            // On Linux/macOS SO_REUSEADDR allows multiple sockets on the
            // same port; the OS delivers each incoming SYN to exactly one,
            // so whichever task calls accept() first receives it.
            let std = std::net::TcpListener::bind(addr)?;
            std.set_nonblocking(true)?;
            let listener = TcpListener::from_std(std)?;

            // ── Race: accept incoming vs connect outgoing ──
            let accept = async {
                let (stream, peer) = time::timeout(HOLE_PUNCH_TIMEOUT, listener.accept())
                    .await
                    .map_err(|_| HolePunchError::TimedOut(HOLE_PUNCH_TIMEOUT))?
                    .map_err(HolePunchError::Io)?;
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

/// Try all peer candidates sequentially.
async fn connect_sequential(
    peer_candidates: &[SocketAddr],
) -> Result<HolePunchResult, HolePunchError> {
    for &addr in peer_candidates {
        tracing::debug!(target = %addr, "attempting TCP connect to peer candidate");
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
    Err(HolePunchError::AllFailed(peer_candidates.len()))
}

// ─── Invite Helpers ─────────────────────────────────────────────────────────

/// Extract deduplicated `SocketAddr` candidates from an invite.
///
/// The legacy `address_hint` is parsed first and placed at the front of the
/// result list (highest priority). Remaining `candidates` are appended in
/// their original order, with duplicates silently removed.
pub fn extract_candidates_from_invite(
    address_hint: &str,
    candidates: &[WireCandidate],
) -> Vec<SocketAddr> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    // Primary address from the invite hint.
    if let Ok(addr) = address_hint.parse::<SocketAddr>() {
        seen.insert(addr);
        result.push(addr);
    }

    // Additional addresses from the candidate list.
    for c in candidates {
        if let Ok(addr) = c.address.parse::<SocketAddr>() {
            if seen.insert(addr) {
                result.push(addr);
            }
        }
    }

    tracing::debug!(
        count = result.len(),
        "extracted candidates from invite"
    );
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
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &[]);
        assert_eq!(addrs.len(), 1);
        assert_eq!(addrs[0], "1.2.3.4:12345".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_with_structured_candidates() {
        let candidates = vec![
            WireCandidate {
                address: "192.168.1.5:54321".into(),
                candidate_type: 0,
            },
            WireCandidate {
                address: "5.6.7.8:9876".into(),
                candidate_type: 1,
            },
        ];
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(addrs.len(), 3);
        assert_eq!(addrs[0], "1.2.3.4:12345".parse::<SocketAddr>().unwrap());
        assert_eq!(addrs[1], "192.168.1.5:54321".parse::<SocketAddr>().unwrap());
        assert_eq!(addrs[2], "5.6.7.8:9876".parse::<SocketAddr>().unwrap());
    }

    #[test]
    fn test_deduplicates() {
        let candidates = vec![
            WireCandidate {
                address: "1.2.3.4:12345".into(),
                candidate_type: 0,
            },
        ];
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(addrs.len(), 1);
    }

    #[test]
    fn test_invalid_candidate_skipped() {
        let candidates = vec![WireCandidate {
            address: "not-a-valid-addr".into(),
            candidate_type: 0,
        }];
        let addrs = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(addrs.len(), 1);
    }

    #[test]
    fn test_empty_all() {
        let addrs = extract_candidates_from_invite("", &[]);
        assert!(addrs.is_empty());
    }

    #[test]
    fn test_empty_hint_with_candidates() {
        let candidates = vec![WireCandidate {
            address: "10.0.0.1:8000".into(),
            candidate_type: 0,
        }];
        let addrs = extract_candidates_from_invite("", &candidates);
        assert_eq!(addrs.len(), 1);
    }

    // ═══════════════════════════════════════════════════════════
    // Error display
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", HolePunchError::NoCandidates),
            "no candidates to try"
        );
        assert_eq!(
            format!("{}", HolePunchError::AllFailed(3)),
            "all 3 candidate(s) failed"
        );
    }
}
