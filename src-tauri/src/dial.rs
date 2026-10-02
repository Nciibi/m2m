//! M2M — Outbound Dial Chokepoint
//!
//! **Every outbound TCP connection in M2M MUST go through [`dial`] or
//! [`dial_with_timeout`] in this module.** No other module may call
//! `tokio::net::TcpStream::connect` directly.
//!
//! ## Why this module exists
//!
//! The Tor integration originally lived only in [`crate::tor`], whose
//! `connect()` was invoked from exactly two live call sites. Every actual
//! peer-connection path — the Happy-Eyeballs connection manager, the relay
//! client, the DHT, UPnP/PCP/NAT-PMP port mapping, and reconnect — called
//! `TcpStream::connect` directly. Enabling Tor therefore routed almost no
//! traffic through the proxy, while the UI advertised that "all outgoing
//! connections use Tor". For a tool whose stated users are journalists and
//! people under coercion, a privacy feature that silently does nothing is
//! worse than no feature at all.
//!
//! Centralising the decision makes the property *structural*: a call site
//! that uses this module is Tor-correct by construction, and a call site that
//! does not is visible in review as a single `grep TcpStream::connect`.
//! `tests::test_no_module_bypasses_the_dial_chokepoint` enforces that at
//! test time.
//!
//! ## Leak-prevention policy
//!
//! When Tor is enabled, M2M must not emit a single packet to a
//! non-Tor-routable address. Three classes of address are therefore refused
//! outright rather than attempted:
//!
//! * **Loopback / unspecified / multicast** — a Tor circuit cannot reach
//!   them, and attempting one leaks the attempt locally.
//! * **Private and link-local ranges** (`10/8`, `172.16/12`, `192.168/16`,
//!   `169.254/16`, `100.64/10` CGNAT, `fc00::/7`, `fe80::/10`) — these
//!   identify the user's LAN or ISP. Connecting to them while Tor is on is
//!   both useless (Tor cannot route there) and a direct IP leak.
//! * **IPv4-mapped/compatible IPv6 forms of the above** — the same address
//!   in a different notation must not slip past the check.
//!
//! Refusing is a *hard error*, never a silent fallback to a direct connect.
//! A silent fallback would reintroduce exactly the bug this module exists to
//! prevent.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::net::TcpStream;

use crate::tor;

/// Default timeout applied by [`dial`] when a caller does not supply one.
///
/// Every outbound connect in M2M is to an untrusted or unreliable address, so
/// a bounded wait is required; an unbounded `TcpStream::connect` can hang a
/// spawned strategy task until the process exits.
pub const DEFAULT_DIAL_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum DialError {
    /// Tor is enabled and the destination is not reachable over the Tor
    /// network. Refused rather than attempted — attempting it would either
    /// leak the user's IP or be meaningless.
    #[error(
        "refusing to connect to non-Tor-routable address {0} while Tor is enabled \
         (this would bypass the Tor proxy and leak your IP)"
    )]
    NonTorRoutable(SocketAddr),

    /// The TCP connect (direct or via SOCKS5) failed.
    #[error("dial failed: {0}")]
    Dial(String),

    /// The underlying socket operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The connect exceeded its deadline and was cancelled.
    #[error("dial timed out after {0:?}")]
    TimedOut(Duration),

    /// A LAN-only protocol (UPnP/NAT-PMP/PCP) was attempted while Tor is on.
    #[error("LAN port mapping is disabled while Tor is enabled (would disclose the real IP)")]
    TorLanUnsupported(SocketAddr),

    /// An outbound UDP query (STUN / PCP / NAT-PMP / SSDP) was attempted
    /// while Tor is on. Tor has no UDP transport, so the query would go out
    /// directly from the real address — the one class of leak the TCP
    /// chokepoint could not catch.
    #[error(
        "outbound UDP query blocked while Tor is enabled (STUN, PCP, NAT-PMP and \
         SSDP would go out from your real address and leak your IP)"
    )]
    TorUdpUnsupported,
}

/// Returns `true` when `ip` can never be reached over the public internet,
/// and therefore must not be dialled while Tor is enabled.
///
/// This is a *conservative* classification: anything that is not clearly
/// global unicast is treated as non-routable. A false positive costs a
/// candidate; a false negative leaks the user's IP.
pub fn is_non_tor_routable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V6(v6) => {
            // Loopback / unspecified / multicast are checked FIRST, before the
            // IPv4-compatibility conversions below. `::1` is an
            // "IPv4-compatible" address that `to_ipv4()` reports as
            // `0.0.0.1` — not itself loopback — so converting first would let
            // the IPv6 loopback through as routable.
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return true;
            }
            // An IPv4 address reached through an IPv6 socket is the same
            // address in disguise. Re-check the mapped form so
            // `::ffff:192.168.1.1` is treated exactly like `192.168.1.1`.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_non_tor_routable(IpAddr::V4(v4));
            }
            // Deprecated IPv4-compatible form (`::a.b.c.d`).
            if let Some(v4) = v6.to_ipv4() {
                return is_non_tor_routable(IpAddr::V4(v4));
            }
            // Unique local (fc00::/7) and link-local (fe80::/10).
            (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation()
                // Shared address space (CGNAT, RFC 6598) — identifies the ISP.
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
                // Benchmarking (RFC 2544).
                || (v4.octets()[0] == 198 && (v4.octets()[1] & 0xfe) == 18)
                || v4.is_private()
                || v4.is_link_local()
        }
    }
}

/// Returns `true` when the given address may be dialled in the current
/// transport mode.
///
/// Under Tor this is `!is_non_tor_routable(ip)`. With Tor off every address
/// is permitted, including LAN peers, which is the intended P2P behaviour.
pub fn is_dialable(addr: SocketAddr) -> bool {
    !tor::is_enabled() || !is_non_tor_routable(addr.ip())
}

/// Filter the candidate list for advertisement in a **plaintext** handshake frame.
///
/// # Why this exists
///
/// `HandshakeInit` and `HandshakeResponse` are written through
/// [`crate::network::write_frame`], which is *not* encrypted — they precede
/// session establishment, so there is no key to encrypt them with. Every
/// candidate in that frame is therefore readable by the destination peer, by
/// the Tor exit, and by every AS on the path between them.
///
/// The list being published is built from [`crate::local_addr::gather_host_candidates`]
/// (the host's real LAN address), `gather_ipv6_candidates` (global unicast
/// IPv6), and the STUN server-reflexive address (the host's public IP as seen
/// by a third party). Under Tor, publishing any of them makes the routing
/// decorative: the peer can note the address and simply connect to it directly
/// on a later attempt, and none of those connections touch Tor.
///
/// So under Tor we advertise **no** directly-routable candidates. Relay
/// candidates are kept: they carry a `relay_id` rather than an address the
/// peer can dial, and they are the only path that works for a Tor session
/// anyway.
///
/// With Tor off this is the identity function, so direct P2P and LAN
/// connectivity is unchanged.
///
/// # Safety note
///
/// This only withholds information. `Session::our_candidates` is recorded but
/// never dialled — the connection is the one the handshake is running over —
/// so an empty list cannot break an established session.
pub fn filter_advertised_candidates(
    candidates: Vec<crate::protocol::WireCandidate>,
) -> Vec<crate::protocol::WireCandidate> {
    if !tor::is_enabled() {
        return candidates;
    }

    let kept: Vec<_> = candidates
        .into_iter()
        .filter(|c| {
            // Relay candidates are identified by carrying a relay id. Anything
            // else is a directly-routable address and is withheld under Tor.
            c.relay_id.is_some()
        })
        .collect();

    if !kept.is_empty() {
        tracing::debug!(
            advertised = kept.len(),
            "Tor enabled — advertising relay candidates only, withholding direct addresses"
        );
    } else {
        tracing::info!(
            "Tor enabled — no candidates advertised; peers must use a relay to reach us"
        );
    }
    kept
}

/// Connect to `addr`, honouring the Tor setting, with [`DEFAULT_DIAL_TIMEOUT`].
///
/// This is the entry point every outbound peer connection should use.
pub async fn dial(addr: SocketAddr) -> Result<TcpStream, DialError> {
    dial_with_timeout(addr, DEFAULT_DIAL_TIMEOUT).await
}

/// Connect to `addr` with an explicit deadline, honouring the Tor setting.
///
/// # Behaviour
///
/// * **Tor enabled** — `addr` is first checked with
///   [`is_non_tor_routable`]. If it is non-routable the call returns
///   [`DialError::NonTorRoutable`] and **no packet is sent**. Otherwise the
///   connection is made through the local SOCKS5 proxy, so the destination
///   sees the Tor exit's address, never the user's.
/// * **Tor disabled** — a plain direct TCP connect, which is the intended
///   behaviour for LAN / high-speed P2P links.
///
/// In both cases the returned stream has `TCP_NODELAY` set, matching what the
/// pre-existing call sites did inline.
pub async fn dial_with_timeout(
    addr: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream, DialError> {
    // Refuse BEFORE opening any socket. Under Tor this must not degrade into
    // a direct connect: that is the exact failure mode this module fixes.
    if tor::is_enabled() && is_non_tor_routable(addr.ip()) {
        tracing::warn!(
            target = %addr,
            "Tor is enabled — refusing non-Tor-routable destination \
             (LAN/CGNAT/loopback candidate dropped rather than leaking the real IP)"
        );
        return Err(DialError::NonTorRoutable(addr));
    }

    let connect = async {
        if tor::is_enabled() {
            // `connect_via_socks` is unconditional; the branch above this is
            // what selects it, so the two cannot silently diverge.
            tor::connect_via_socks(addr)
                .await
                .map_err(|e| DialError::Dial(e.to_string()))
        } else {
            TcpStream::connect(addr).await.map_err(DialError::Io)
        }
    };

    let stream = tokio::time::timeout(timeout, connect)
        .await
        .map_err(|_| DialError::TimedOut(timeout))??;

    // Latency matters for a chat protocol; Nagle would batch small frames.
    let _ = stream.set_nodelay(true);
    Ok(stream)
}

/// Is Tor currently enabled? Thin re-export so call sites that need to make
/// transport decisions (candidate filtering, invite generation) do not have
/// to import [`crate::tor`] directly.
pub fn tor_enabled() -> bool {
    tor::is_enabled()
}

/// Dial an address that only makes sense on the local network, and **refuse
/// when Tor is enabled**.
///
/// Used by the LAN-only protocols — UPnP/IGD, NAT-PMP and PCP — which talk to
/// the user's own router. Two reasons these must not run under Tor:
///
/// 1. **They are meaningless over Tor.** Tor cannot reach `192.168.0.1`, so
///    the attempt would simply fail.
/// 2. **They are an IP-disclosure primitive.** Requesting a port mapping tells
///    the ISP's upstream NAT — and therefore anyone watching it — that this
///    host wants to be reachable inbound. Combined with a Tor-routed peer
///    connection it also defeats the point of Tor: the peer reaches you
///    directly, so the exit node's IP is irrelevant.
///
/// Returning an error (rather than silently skipping) lets callers surface
/// "port forwarding was skipped because Tor is enabled" to the user, instead
/// of quietly leaving them without a direct path.
pub async fn dial_lan_only(addr: SocketAddr, timeout: Duration) -> Result<TcpStream, DialError> {
    if tor::is_enabled() {
        tracing::warn!(
            target = %addr,
            "skipping LAN port mapping because Tor is enabled \
             (requesting a port mapping would disclose the real IP)"
        );
        return Err(DialError::TorLanUnsupported(addr));
    }
    dial_with_timeout(addr, timeout).await
}

/// True when LAN port mapping (UPnP/NAT-PMP/PCP) may be attempted.
///
/// Invite generation uses this to omit port-mapped candidates entirely under
/// Tor, rather than producing an invite whose candidates are all unreachable.
pub fn lan_port_mapping_allowed() -> bool {
    !tor::is_enabled()
}

// ─── UDP chokepoint ───────────────────────────────────────────────────────────
//
// `dial_with_timeout` above is the single outbound TCP path, and
// `test_no_module_bypasses_the_dial_chokepoint` keeps it that way. There was
// no equivalent for UDP, which is where the remaining IP-disclosure
// primitives live: a STUN Binding Request carries the sender's source address
// to a third party by construction, and PCP / NAT-PMP / SSDP talk to the
// user's own router. All of them bypassed the TCP guard entirely, so enabling
// Tor did not stop M2M from disclosing the user's real address — five
// production call sites reached `stun::discover_public_addrs` unguarded.
//
// `bind_udp_for_external_query` is the corresponding seam: it binds the
// ephemeral socket those protocols need and refuses under Tor. A bound UDP
// socket sends nothing on its own, so binding is safe; the *send* is the
// disclosure, which is why the guard belongs on the function that exists
// solely to feed an off-host query.
//
// This is deliberately narrower than the TCP guard: a datagram to a peer
// address we have already chosen to talk to needs no special handling.

/// Bind an ephemeral UDP socket for a query that will leave this host.
///
/// Refuses while Tor is enabled. STUN, PCP, NAT-PMP and SSDP all fall in this
/// category: each is either an explicit request to a third party that resolves
/// to this machine's real address, or a request to the local router that
/// discloses that the host wants to be reachable inbound.
pub async fn bind_udp_for_external_query() -> Result<tokio::net::UdpSocket, DialError> {
    if tor::is_enabled() {
        return Err(DialError::TorUdpUnsupported);
    }
    // Try IPv4 first; on an IPv6-only network fall back to the wildcard v6
    // address, mirroring what the STUN client did inline before.
    match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => Ok(s),
        Err(_) => tokio::net::UdpSocket::bind("[::]:0")
            .await
            .map_err(DialError::Io),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod dial_tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn v4(s: &str) -> IpAddr {
        IpAddr::V4(s.parse::<Ipv4Addr>().unwrap())
    }
    fn v6(s: &str) -> IpAddr {
        IpAddr::V6(s.parse::<Ipv6Addr>().unwrap())
    }

    #[test]
    fn loopback_unspecified_multicast_are_non_routable() {
        assert!(is_non_tor_routable(v4("127.0.0.1")));
        assert!(is_non_tor_routable(v4("127.255.255.254")));
        assert!(is_non_tor_routable(v4("0.0.0.0")));
        assert!(is_non_tor_routable(v4("224.0.0.1")));
        assert!(is_non_tor_routable(v4("255.255.255.255")));
        assert!(is_non_tor_routable(v6("::1")));
        assert!(is_non_tor_routable(v6("::")));
        assert!(is_non_tor_routable(v6("ff02::1")));
    }

    #[test]
    fn lan_and_isp_ranges_are_non_routable() {
        // The exact ranges a LAN discovery / UPnP candidate would carry.
        assert!(is_non_tor_routable(v4("192.168.1.5")));
        assert!(is_non_tor_routable(v4("10.0.0.1")));
        assert!(is_non_tor_routable(v4("172.16.0.1")));
        assert!(is_non_tor_routable(v4("172.31.255.255")));
        // CGNAT — identifies the ISP's subscriber network.
        assert!(is_non_tor_routable(v4("100.64.0.1")));
        // Link-local, incl. the cloud metadata endpoint.
        assert!(is_non_tor_routable(v4("169.254.169.254")));
        // IPv6 ULA + link-local.
        assert!(is_non_tor_routable(v6("fc00::1")));
        assert!(is_non_tor_routable(v6("fd12:3456::1")));
        assert!(is_non_tor_routable(v6("fe80::1")));
    }

    #[test]
    fn public_addresses_are_routable() {
        assert!(!is_non_tor_routable(v4("8.8.8.8")));
        assert!(!is_non_tor_routable(v4("1.1.1.1")));
        assert!(!is_non_tor_routable(v4("45.33.32.156")));
        assert!(
            !is_non_tor_routable(v4("172.32.0.1")),
            "just outside 172.16/12"
        );
        assert!(
            !is_non_tor_routable(v4("100.128.0.1")),
            "just outside 100.64/10"
        );
        assert!(!is_non_tor_routable(v6("2001:4860:4860::8888")));
        assert!(!is_non_tor_routable(v6("2606:4700:4700::1111")));
    }

    /// Documentation / benchmarking ranges are not globally routable. M2M
    /// never needs to dial them, and a hostile bootstrap or DHT record that
    /// points at one should be dropped rather than attempted.
    #[test]
    fn documentation_ranges_are_non_routable() {
        assert!(is_non_tor_routable(v4("192.0.2.1")), "TEST-NET-1");
        assert!(is_non_tor_routable(v4("198.51.100.1")), "TEST-NET-2");
        assert!(is_non_tor_routable(v4("203.0.113.1")), "TEST-NET-3");
        assert!(
            is_non_tor_routable(v4("198.18.0.1")),
            "benchmarking, RFC 2544"
        );
    }

    /// `::1` is an "IPv4-compatible" address that `to_ipv4()` maps to
    /// `0.0.0.1`. The loopback check must run before that conversion or the
    /// IPv6 loopback is misclassified as a routable public address.
    #[test]
    fn ipv6_loopback_is_not_misclassified_via_ipv4_compat_form() {
        assert!(is_non_tor_routable(v6("::1")), "IPv6 loopback");
        assert!(is_non_tor_routable(v6("::")));
    }

    /// The critical bypass: a private address dressed up in IPv6 notation must
    /// be classified exactly like its bare IPv4 form.
    #[test]
    fn ipv4_mapped_and_compatible_forms_do_not_bypass() {
        assert!(is_non_tor_routable(v6("::ffff:192.168.1.5")));
        assert!(is_non_tor_routable(v6("::ffff:127.0.0.1")));
        assert!(is_non_tor_routable(v6("::192.168.1.5")));
        assert!(!is_non_tor_routable(v6("::ffff:8.8.8.8")));
    }

    #[test]
    fn dialable_depends_on_tor_mode() {
        let lan: SocketAddr = "192.168.1.5:1234".parse().unwrap();
        let public: SocketAddr = "8.8.8.8:1234".parse().unwrap();

        tor::set_enabled(false);
        assert!(is_dialable(lan), "LAN peers are legitimate with Tor off");
        assert!(is_dialable(public));

        tor::set_enabled(true);
        assert!(!is_dialable(lan), "LAN peers must be refused under Tor");
        assert!(is_dialable(public));

        tor::set_enabled(false);
    }

    /// With Tor enabled, a non-routable destination must fail *without* any
    /// network I/O. If this test needs a Tor daemon it means the guard is
    /// being applied after the connect instead of before it.
    #[tokio::test]
    async fn tor_mode_refuses_lan_address_before_any_io() {
        tor::set_enabled(true);
        // A black-hole address that would hang if we actually dialled it.
        let lan: SocketAddr = "192.0.2.1:9".parse().unwrap();
        let err = dial_with_timeout(lan, Duration::from_secs(30))
            .await
            .expect_err("LAN address must be refused under Tor");
        assert!(
            matches!(err, DialError::NonTorRoutable(_)),
            "expected NonTorRoutable, got {err:?}"
        );
        tor::set_enabled(false);
    }

    #[tokio::test]
    async fn direct_mode_still_reaches_loopback() {
        // A live listener proves the non-Tor path is genuinely functional and
        // not accidentally short-circuited by the guard.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tor::set_enabled(false);
        let stream = dial_with_timeout(addr, Duration::from_secs(5))
            .await
            .expect("loopback must remain dialable with Tor off");
        drop(stream);
    }

    /// The property that makes this module load-bearing rather than
    /// advisory: no module may dial a TCP socket directly. If someone adds a
    /// raw `TcpStream::connect` in a peer-facing module, this test fails.
    ///
    /// Only `tor.rs` (the transport implementation) and this module are
    /// permitted to hold the raw call.
    #[test]
    fn test_no_module_bypasses_the_dial_chokepoint() {
        /// Modules allowed to contain the raw socket call.
        const ALLOWED: &[&str] = &["tor.rs", "dial.rs"];

        let src_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders: Vec<String> = Vec::new();

        let mut stack = vec![src_root.clone()];
        while let Some(dir) = stack.pop() {
            let entries =
                std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    // `fuzz/` holds a separate crate with its own deps; skip it.
                    if path.file_name().and_then(|s| s.to_str()) != Some("fuzz") {
                        stack.push(path);
                    }
                    continue;
                }
                if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                    continue;
                }
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                if ALLOWED.contains(&name.as_str()) {
                    continue;
                }
                let Ok(src) = std::fs::read_to_string(&path) else {
                    continue;
                };
                // Strip comment lines so prose describing the chokepoint (and
                // the doc tables in hole_punch.rs) does not trip the check.
                let code: String = src
                    .lines()
                    .filter(|l| {
                        let t = l.trim_start();
                        !t.starts_with("//")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if code.contains("TcpStream::connect") {
                    offenders.push(
                        path.strip_prefix(&src_root)
                            .unwrap_or(&path)
                            .display()
                            .to_string(),
                    );
                }
            }
        }

        offenders.sort();
        assert!(
            offenders.is_empty(),
            "these modules bypass the Tor-aware dial chokepoint — \
             use crate::dial::dial() instead: {offenders:?}"
        );
    }

    fn wc(
        address: &str,
        candidate_type: u8,
        relay_id: Option<&str>,
    ) -> crate::protocol::WireCandidate {
        crate::protocol::WireCandidate {
            address: address.to_string(),
            candidate_type,
            relay_id: relay_id.map(str::to_string),
        }
    }

    /// The handshake frame carrying these candidates is *plaintext* — it
    /// precedes session establishment, so there is no key to encrypt it with.
    /// Advertising a directly-routable address under Tor therefore hands the
    /// peer, the Tor exit and every AS on the path a way to bypass the proxy
    /// on a later attempt.
    #[test]
    fn advertised_candidates_withhold_direct_addresses_under_tor() {
        crate::tor::set_enabled(true);

        let lan = wc("192.168.1.57:52341", 0, None);
        let ipv6 = wc("[2001:db8::1]:52341", 5, None);
        let srflx = wc("203.0.113.9:40000", 1, None);
        let relay = wc("relay.invalid:0", 3, Some("abc123"));

        let kept = filter_advertised_candidates(vec![lan.clone(), ipv6, srflx, relay.clone()]);

        assert_eq!(
            kept.len(),
            1,
            "only the relay candidate may survive under Tor, got {kept:?}"
        );
        assert_eq!(kept[0].relay_id.as_deref(), Some("abc123"));

        crate::tor::set_enabled(false);
    }

    /// With Tor off this must be the identity function, or direct P2P and LAN
    /// connectivity break for every user who has not enabled Tor.
    #[test]
    fn advertised_candidates_are_unchanged_without_tor() {
        crate::tor::set_enabled(false);
        let all = vec![
            wc("192.168.1.57:52341", 0, None),
            wc("203.0.113.9:40000", 1, None),
            wc("relay.invalid:0", 3, Some("abc123")),
        ];
        let kept = filter_advertised_candidates(all.clone());
        assert_eq!(kept.len(), all.len());
        for (a, b) in all.iter().zip(kept.iter()) {
            assert_eq!(a.address, b.address);
        }
    }

    /// An all-direct set under Tor must yield an empty list rather than
    /// panicking or silently keeping something.
    #[test]
    fn advertised_candidates_can_be_empty_under_tor() {
        crate::tor::set_enabled(true);
        let kept = filter_advertised_candidates(vec![wc("198.51.100.4:1", 0, None)]);
        assert!(kept.is_empty());
        crate::tor::set_enabled(false);
    }
}
