use crate::local_addr;
use crate::stun;
/// M2M — Candidate Module
///
/// ICE-Lite candidate types and gathering logic.
/// Provides structured network candidates (host, server-reflexive)
/// with prioritization for ICE-Lite connectivity establishment.
use serde::{Deserialize, Serialize};

// ─── Candidate Types ────────────────────────────────────────────────────────

/// Type of network candidate, matching ICE RFC 8445 terminology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CandidateType {
    /// A candidate obtained by binding to a local port on a local interface.
    Host = 0,
    /// A candidate whose address is obtained from a STUN server (server-reflexive).
    /// This is the public IP:port as seen by the STUN server.
    ServerReflexive = 1,
    /// A candidate whose address is obtained from a peer (peer-reflexive).
    PeerReflexive = 2,
    /// A candidate obtained from a TURN relay server.
    Relay = 3,
    /// A candidate obtained from binding to an IPv6 interface.
    /// IPv6 global unicast addresses are typically directly routable,
    /// making this the most reliable path after IPv4 LAN.
    Ipv6 = 5,
}

impl std::fmt::Display for CandidateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CandidateType::Host => write!(f, "host"),
            CandidateType::ServerReflexive => write!(f, "srflx"),
            CandidateType::PeerReflexive => write!(f, "prflx"),
            CandidateType::Relay => write!(f, "relay"),
            CandidateType::Ipv6 => write!(f, "ipv6"),
        }
    }
}

/// A network candidate that can be used for peer-to-peer connectivity.
///
/// Follows ICE candidate semantics with type-based priority:
///   Host candidates: highest priority (direct path)
///   Server-reflexive: medium priority (NAT traversal)
///   Relay: lowest priority (fallback)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCandidate {
    /// IP:port address of this candidate.
    pub address: String,
    /// Candidate type.
    pub candidate_type: CandidateType,
    /// Priority (computed from type preference + local pref).
    /// Higher = more preferred.
    pub priority: u32,
    /// Foundation: used for ICE candidate pairing (same foundation = same base).
    pub foundation: String,
    /// Base address (the local socket this candidate was derived from).
    pub base_address: Option<String>,
}

/// Combined network diagnostics for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkDiagnostics {
    pub candidates: Vec<NetworkCandidate>,
    pub nat_type: stun::NatType,
    pub stun_servers: Vec<stun::StunServerHealth>,
    pub connectivity: stun::ConnectivityStatus,
}

// ─── Priority Computation ───────────────────────────────────────────────────

/// ICE candidate priority formula (RFC 8445 §5.1.2.1):
///   priority = (2^24)*type_pref + (2^8)*local_pref + (2^0)*component_id
///
/// Type preferences:
///   Host: 126
///   IPv6: 115   (below LAN host, above srflx)
///   Peer-Reflexive: 110
///   Server-Reflexive: 100
///   Port-mapped: 95  (UPnP/NAT-PMP/PCP — documented for reference)
///   Relay: 0
const TYPE_PREF_HOST: u32 = 126;
const TYPE_PREF_IPV6: u32 = 115;
const TYPE_PREF_PRFLX: u32 = 110;
const TYPE_PREF_SRFLX: u32 = 100;
const TYPE_PREF_RELAY: u32 = 0;

fn compute_priority(candidate_type: CandidateType, local_pref: u32) -> u32 {
    let type_pref = match candidate_type {
        CandidateType::Host => TYPE_PREF_HOST,
        CandidateType::Ipv6 => TYPE_PREF_IPV6,
        CandidateType::PeerReflexive => TYPE_PREF_PRFLX,
        CandidateType::ServerReflexive => TYPE_PREF_SRFLX,
        CandidateType::Relay => TYPE_PREF_RELAY,
    };
    (type_pref << 24) | ((local_pref & 0xFF) << 8) | 1 // component_id = 1 for RTP/RTCP, 1 for our single stream
}

// ─── Candidate Gathering ────────────────────────────────────────────────────

/// Gather all host candidates by probing local interfaces.
/// Returns a list of `NetworkCandidate` with type=Host, sorted by priority.
pub fn gather_host_candidates() -> Vec<NetworkCandidate> {
    let addrs = local_addr::gather_host_candidates();
    let total = addrs.len();
    let mut candidates: Vec<NetworkCandidate> = addrs
        .into_iter()
        .enumerate()
        .map(|(i, addr)| {
            let local_pref = ((total - i) * 10) as u32; // Prefer earlier entries
            NetworkCandidate {
                address: addr.to_string(),
                candidate_type: CandidateType::Host,
                priority: compute_priority(CandidateType::Host, local_pref),
                foundation: format!("host-{}", i),
                base_address: Some(addr.to_string()),
            }
        })
        .collect();

    // Sort by priority descending
    candidates.sort_by_key(|c| std::cmp::Reverse(c.priority));
    candidates
}

/// Gather IPv6 host candidates.
///
/// Discovers local global-unicast IPv6 addresses. These are directly routable
/// on the IPv6 internet without NAT (most residential ISPs already provide
/// IPv6 connectivity), making this a high-reliability path.
pub fn gather_ipv6_candidates() -> Vec<NetworkCandidate> {
    let addrs = local_addr::gather_ipv6_candidates();
    let total = addrs.len();
    let mut candidates: Vec<NetworkCandidate> = addrs
        .into_iter()
        .enumerate()
        .map(|(i, addr)| {
            let local_pref = ((total - i) * 10) as u32;
            NetworkCandidate {
                address: addr.to_string(),
                candidate_type: CandidateType::Ipv6,
                priority: compute_priority(CandidateType::Ipv6, local_pref),
                foundation: format!("ipv6-{}", i),
                base_address: Some(addr.to_string()),
            }
        })
        .collect();

    candidates.sort_by_key(|c| std::cmp::Reverse(c.priority));
    candidates
}

/// Gather server-reflexive candidates from STUN results.
///
/// Publishes **at most one** candidate, and only when
/// [`stun::StunMultiResult::consensus_addr`] is populated — i.e. when
/// [`stun::MIN_CONSENSUS_SERVERS`] independent servers agreed
/// (`consensus`) or one IP held a strict majority of them, and only when that
/// address passes [`stun::is_publishable_public_addr`].
///
/// # What this replaces
///
/// This used to iterate `multi_result.results` — *every* server's answer — and
/// emit a candidate for each. That published attacker-chosen addresses: one
/// rogue STUN server, or one DNS-hijacked hostname already sitting in the
/// user's list, contributed an arbitrary `host:port` to the candidate set. That
/// set flows into `state.candidates`, into invite candidate lists (including
/// one-time, shareable invite links) and into the **plaintext**
/// `HandshakeInit` / `HandshakeResponse` frames. A peer that picks the injected
/// candidate connects to the attacker, who is then first in the path for a
/// handshake whose opening frames are unauthenticated by construction — so the
/// attacker sees the X3DH bundle and the identity key, and can sit in the
/// middle of a session the user believes is direct.
///
/// # Cost
///
/// When the servers disagree without a strict majority, or only one answers,
/// this returns an empty list: no server-reflexive candidate is published at
/// all. Peers then connect via the host/IPv6/relay candidates or by dialling
/// the listening port directly. That is the intended direction — withholding an
/// address beats publishing a wrong one.
pub fn gather_reflexive_candidates(multi_result: &stun::StunMultiResult) -> Vec<NetworkCandidate> {
    // The consensus address is the only value that survived the quorum and
    // address-validation checks in `stun`. Everything in `results` is
    // uncorroborated per-server output and must not reach the wire.
    let consensus = match multi_result.consensus_addr {
        Some(addr) => addr,
        None => {
            tracing::warn!(
                responding = multi_result.responding_servers,
                total = multi_result.total_servers,
                "no STUN consensus address — publishing no server-reflexive candidate"
            );
            return Vec::new();
        }
    };

    // Re-validated here rather than trusted from the aggregator: this is the
    // last point before the address is written into an invite, and the caller
    // cannot see whether the `StunMultiResult` it holds came from the network
    // or was constructed by hand.
    if !stun::is_publishable_public_addr(&consensus) {
        tracing::warn!(
            addr = %consensus,
            "STUN consensus address is not globally routable — publishing no \
             server-reflexive candidate"
        );
        return Vec::new();
    }

    let base = match local_addr::gather_host_candidates().first() {
        Some(a) => a.to_string(),
        None => return Vec::new(),
    };

    let addr_str = consensus.to_string();
    vec![NetworkCandidate {
        address: addr_str.clone(),
        candidate_type: CandidateType::ServerReflexive,
        // Corroborated address, so full local preference. There is only one
        // such address, so the old "demote when there is no consensus" branch
        // has nothing left to demote.
        priority: compute_priority(CandidateType::ServerReflexive, 100),
        foundation: format!("srflx-{}", addr_str),
        base_address: Some(base),
    }]
}
