/// M2M — Candidate Module
///
/// ICE-Lite candidate types and gathering logic.
/// Provides structured network candidates (host, server-reflexive)
/// with prioritization for ICE-Lite connectivity establishment.
use serde::{Deserialize, Serialize};
use crate::local_addr;
use crate::stun;

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
///   IPv6: 115   (below LAN host, above srflx — IPv6 is routable but may have higher latency)
///   Peer-Reflexive: 110
///   Server-Reflexive: 100
///   Port-mapped: 95  (UPnP/NAT-PMP/PCP — reliably forwarded but via NAT)
///   Relay: 0
const TYPE_PREF_HOST: u32 = 126;
const TYPE_PREF_IPV6: u32 = 115;
const TYPE_PREF_PRFLX: u32 = 110;
const TYPE_PREF_SRFLX: u32 = 100;
#[allow(dead_code)]
const TYPE_PREF_PORT_MAPPED: u32 = 95;
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
    candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
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

    candidates.sort_by(|a, b| b.priority.cmp(&a.priority));
    candidates
}

/// Gather server-reflexive candidates from STUN results.
/// Maps each STUN result to a candidate with type=ServerReflexive.
pub fn gather_reflexive_candidates(
    multi_result: &stun::StunMultiResult,
) -> Vec<NetworkCandidate> {
    let base = local_addr::gather_host_candidates()
        .first()
        .map(|a| a.to_string());

    multi_result
        .results
        .iter()
        .filter_map(|result| {
            let addr_str = result.public_addr.to_string();
            // Deduplicate: skip if same address from different servers
            // (consensus means they're all the same anyway)
            base.as_ref().map(|host| {
                let local_pref = if multi_result.consensus { 100 } else { 80 };
                NetworkCandidate {
                    address: addr_str.clone(),
                    candidate_type: CandidateType::ServerReflexive,
                    priority: compute_priority(CandidateType::ServerReflexive, local_pref),
                    foundation: format!("srflx-{}", addr_str),
                    base_address: Some(host.clone()),
                }
            })
        })
        .collect()
}

