/// M2M — Connection Manager
///
/// Happy Eyeballs-style connection establishment: all strategies are
/// launched concurrently and the first successful connection wins.
/// Remaining tasks are cancelled immediately.
///
/// ## Why parallel?
///
/// Sequential phases waste wall-clock time on strategies that will fail.
/// The user's network might have IPv4 LAN, IPv6, a UPnP mapping, and a
/// manual forward all viable at the same time — the fastest one should
/// win, not the one that happens to be checked first.
///
/// This is the same pattern [RFC 8305 Happy Eyeballs] uses for racing
/// IPv4 and IPv6, extended to every connection strategy M2M supports.
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

// ─── Strategy ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Strategy {
    DirectTcp { peer: SocketAddr },
    Ipv6Direct { peer: SocketAddr },
    PortMapped { peer: SocketAddr },
    TcpHolePunch { peer: SocketAddr },
    #[allow(dead_code)]
    TcpRelay { peer: SocketAddr },
}

impl Strategy {
    pub fn name(&self) -> &'static str {
        match self {
            Strategy::DirectTcp { .. } => "host",
            Strategy::Ipv6Direct { .. } => "ipv6",
            Strategy::PortMapped { .. } => "port-mapped",
            Strategy::TcpHolePunch { .. } => "srflx",
            Strategy::TcpRelay { .. } => "relay",
        }
    }

    fn peer_addr(&self) -> SocketAddr {
        match self {
            Strategy::DirectTcp { peer }
            | Strategy::Ipv6Direct { peer }
            | Strategy::PortMapped { peer }
            | Strategy::TcpHolePunch { peer }
            | Strategy::TcpRelay { peer } => *peer,
        }
    }
}

// ─── Strategy Result ─────────────────────────────────────────────────────────

pub struct StrategyResult {
    pub stream: TcpStream,
    pub remote_addr: SocketAddr,
    pub role: Role,
    pub strategy_name: &'static str,
    pub latency: Duration,
}

// ─── Legacy result (used by race_accept_or_connect) ─────────────────────────

pub struct HolePunchResult {
    pub stream: TcpStream,
    pub role: Role,
    pub remote_addr: SocketAddr,
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// Per-strategy connect timeout.
const STRATEGY_TIMEOUT: Duration = Duration::from_secs(8);

/// Overall timeout before we give up on all strategies.
const OVERALL_TIMEOUT: Duration = Duration::from_secs(20);

// ─── Connection Manager ─────────────────────────────────────────────────────

/// Happy-Eyeballs connection manager.
///
/// Launches all strategies concurrently and returns the first success.
/// See module-level docs for the rationale.
pub struct ConnectionManager;

impl ConnectionManager {
    /// Establish a connection to a peer by racing every available strategy.
    ///
    /// ## Strategy execution
    ///
    /// | Strategy                | Mechanism                        |
    /// |-------------------------|----------------------------------|
    /// | `DirectTcp`             | `TcpStream::connect`             |
    /// | `Ipv6Direct`            | `TcpStream::connect`             |
    /// | `PortMapped`            | `TcpStream::connect`             |
    /// | `TcpHolePunch`          | `race_accept_or_connect` (below) |
    /// | `TcpRelay`              | reserved for Phase 3             |
    ///
    /// All `DirectTcp` / `Ipv6Direct` / `PortMapped` candidates are each
    /// spawned as an independent task. `TcpHolePunch` candidates are
    /// bundled into a single hole-punch task (they share one shadow listener).
    ///
    /// The first task to report success wins. All remaining tasks are
    /// cancelled via `JoinSet::shutdown`.
    pub async fn connect(
        peer_candidates: &[WireCandidate],
        our_listener_addr: Option<SocketAddr>,
    ) -> Result<StrategyResult, ConnectionError> {
        if peer_candidates.is_empty() {
            return Err(ConnectionError::NoCandidates);
        }

        // ── Build strategy list ──
        let mut simple = Vec::new();  // DirectTcp / Ipv6Direct / PortMapped
        let mut punch = Vec::new();   // TcpHolePunch

        for c in peer_candidates {
            if let Ok(addr) = c.address.parse::<SocketAddr>() {
                match c.candidate_type {
                    0 => simple.push(Strategy::DirectTcp { peer: addr }),
                    5 => simple.push(Strategy::Ipv6Direct { peer: addr }),
                    4 => simple.push(Strategy::PortMapped { peer: addr }),
                    1 | 2 => punch.push(addr),
                    3 => {
                        // Relay — reserved for Phase 3.
                        tracing::debug!(target = %addr, "relay candidate ignored (Phase 3)");
                    }
                    _ => {}
                }
            }
        }

        let total = simple.len() + if punch.is_empty() { 0 } else { 1 };
        if total == 0 {
            return Err(ConnectionError::NoCandidates);
        }

        // ── Race everything in parallel ──
        let deadline = Instant::now() + OVERALL_TIMEOUT;
        let mut set = JoinSet::new();

        // Spawn a connect task for each simple (direct-TCP) strategy.
        for s in simple {
            set.spawn(run_simple(s));
        }

        // Spawn a single hole-punch task that races accept vs all srflx
        // candidates (they share a shadow listener).
        if !punch.is_empty() {
            let addrs = punch;
            let listener = our_listener_addr;
            set.spawn(async move { run_hole_punch(&addrs, listener).await });
        }

        // Collect results. First success wins; log failures but keep going.
        let mut last_error = ConnectionError::AllFailed(total);
        while let Ok(Some(result)) = time::timeout_at(time::Instant::from(deadline), set.join_next()).await {
            match result {
                // Task returned Ok value (a strategy result or error).
                Ok(task_result) => {
                    match task_result {
                        // Strategy succeeded.
                        Ok(strategy_result) => {
                            set.shutdown().await;
                            return Ok(strategy_result);
                        }
                        // Strategy returned an error.
                        Err(e) => {
                            last_error = e;
                            tracing::debug!(error = %last_error, "connection attempt failed");
                        }
                    }
                }
                // Task panicked.
                Err(_join_err) => {}
            }
        }

        Err(last_error)
    }
}

// ─── Per-strategy runners ───────────────────────────────────────────────────

/// Run a single direct-TCP strategy and return the result.
///
/// Wrapped in a `Result<_, ConnectionError>` so the join-set machinery
/// can distinguish strategy failure from task panics.
async fn run_simple(s: Strategy) -> Result<StrategyResult, ConnectionError> {
    let peer = s.peer_addr();
    let start = Instant::now();
    tracing::debug!(target = %peer, strategy = %s.name(), "connecting");

    let stream = tcp_connect_timeout(peer, STRATEGY_TIMEOUT).await?;

    tracing::info!(
        target = %peer,
        strategy = %s.name(),
        latency = ?start.elapsed(),
        "connected"
    );
    Ok(StrategyResult {
        stream,
        remote_addr: peer,
        role: Role::Initiator,
        strategy_name: s.name(),
        latency: start.elapsed(),
    })
}

/// Run the hole-punch race: bind a shadow listener and simultaneously
/// try to connect to all given srflx candidates.
///
/// The first to succeed (accept incoming or connect outgoing) wins.
async fn run_hole_punch(
    peer_candidates: &[SocketAddr],
    our_listener_addr: Option<SocketAddr>,
) -> Result<StrategyResult, ConnectionError> {
    let start = Instant::now();

    let result = race_accept_or_connect(peer_candidates, our_listener_addr).await?;

    tracing::info!(
        peer = %result.remote_addr,
        role = ?result.role,
        latency = ?start.elapsed(),
        "hole punch succeeded"
    );
    Ok(StrategyResult {
        stream: result.stream,
        remote_addr: result.remote_addr,
        role: result.role,
        strategy_name: "srflx",
        latency: start.elapsed(),
    })
}

// ─── Internal: Race Accept vs Connect ───────────────────────────────────────

/// True TCP hole punch: race an incoming accept against outgoing connects.
async fn race_accept_or_connect(
    peer_candidates: &[SocketAddr],
    our_listener_addr: Option<SocketAddr>,
) -> Result<HolePunchResult, ConnectionError> {
    if peer_candidates.is_empty() {
        return Err(ConnectionError::NoCandidates);
    }

    let peer_candidates = peer_candidates.to_vec();

    match our_listener_addr {
        None => connect_sequential(&peer_candidates).await,
        Some(addr) => {
            let std = std::net::TcpListener::bind(addr)?;
            std.set_nonblocking(true)?;
            let listener = TcpListener::from_std(std)?;

            let accept = async {
                let (stream, peer) = time::timeout(OVERALL_TIMEOUT, listener.accept())
                    .await
                    .map_err(|_| ConnectionError::TimedOut(OVERALL_TIMEOUT))?
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
        match time::timeout(STRATEGY_TIMEOUT, TcpStream::connect(addr)).await {
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

pub fn extract_candidates_from_invite(
    address_hint: &str,
    candidates: &[WireCandidate],
) -> Vec<WireCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    if let Ok(addr) = address_hint.parse::<SocketAddr>() {
        seen.insert(addr);
        result.push(WireCandidate {
            address: addr.to_string(),
            candidate_type: 1,
        });
    }

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

    #[test]
    fn test_address_hint_only() {
        let c = extract_candidates_from_invite("1.2.3.4:12345", &[]);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].address, "1.2.3.4:12345");
        assert_eq!(c[0].candidate_type, 1);
    }

    #[test]
    fn test_with_structured_candidates() {
        let candidates = vec![
            WireCandidate { address: "192.168.1.5:54321".into(), candidate_type: 0 },
            WireCandidate { address: "5.6.7.8:9876".into(), candidate_type: 1 },
        ];
        let c = extract_candidates_from_invite("1.2.3.4:12345", &candidates);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].candidate_type, 1);
        assert_eq!(c[1].candidate_type, 0);
        assert_eq!(c[2].candidate_type, 1);
    }

    #[test]
    fn test_strategy_names() {
        assert_eq!(Strategy::DirectTcp { peer: "0.0.0.0:0".parse().unwrap() }.name(), "host");
        assert_eq!(Strategy::Ipv6Direct { peer: "0.0.0.0:0".parse().unwrap() }.name(), "ipv6");
        assert_eq!(Strategy::PortMapped { peer: "0.0.0.0:0".parse().unwrap() }.name(), "port-mapped");
        assert_eq!(Strategy::TcpHolePunch { peer: "0.0.0.0:0".parse().unwrap() }.name(), "srflx");
        assert_eq!(Strategy::TcpRelay { peer: "0.0.0.0:0".parse().unwrap() }.name(), "relay");
    }

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", ConnectionError::NoCandidates), "no candidates supplied");
        assert_eq!(format!("{}", ConnectionError::AllFailed(3)), "all 3 strategy(ies) failed");
    }
}
