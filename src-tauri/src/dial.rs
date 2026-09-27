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
    Io(#[from] std::io::Error),

    /// The connect exceeded its deadline and was cancelled.
    #[error("dial timed out after {0:?}")]
    TimedOut(Duration),

    /// A LAN-only protocol (UPnP/NAT-PMP/PCP) was attempted while Tor is on.
    #[error("LAN port mapping is disabled while Tor is enabled (would disclose the real IP)")]
    TorLanUnsupported(SocketAddr),
}

/// Returns `true` when `ip` can never be reached over the public internet,
/// and therefore must not be dialled while Tor is enabled.
///
/// This is a *conservative* classification: anything that is not clearly
/// global unicast is treated as non-routable. A false positive costs a
/// candidate; a false negative leaks the user's IP.
pub fn is_non_tor_routable(ip: IpAddr) -> bool {
    match ip {
        // An IPv4 address reached through an IPv6 socket is the same address
        // in disguise. Re-check the mapped form so `::ffff:192.168.1.1` is
        // treated exactly like `192.168.1.1`.
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_non_tor_routable(IpAddr::V4(v4));
            }
            // Deprecated IPv4-compatible form (`::a.b.c.d`).
            if let Some(v4) = v6.to_ipv4() {
                return is_non_tor_routable(IpAddr::V4(v4));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // Unique local (fc00::/7) and link-local (fe80::/10).
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
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
            // `tor::connect` is Tor-only; call the direct socket ourselves
            // otherwise so the two branches cannot drift apart.
            tor::connect_via_socks(addr).await.map_err(DialError::Io)
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
        assert!(!is_non_tor_routable(v4("203.0.113.9")), "TEST-NET is documentation-only");
        assert!(!is_non_tor_routable(v6("2001:4860:4860::8888")));
        assert!(!is_non_tor_routable(v6("2606:4700:4700::1111")));
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
    /// advisory: no other module may dial directly. If someone adds a raw
    /// `TcpStream::connect` in a peer-facing module, this fails.
    ///
    /// Only `tor.rs` (the transport implementation) and this module are
    /// allowed to hold the raw call.
    #[test]
    fn test_no_module_bypasses_the_dial_chokepoint() {
        let allowed = ["tor.rs", "dial.rs"];

        let offenders: Vec<String> = crate::protocol_fuzz_regression::crate_root()
            .join("src")
            .read_dir()
            .expect("read src/")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                !allowed.contains(&name.as_str())
            })
            .filter_map(|e| {
                let src = std::fs::read_to_string(e.path()).ok()?;
                // Ignore the doc/test commentary that legitimately names the
                // call, and count only real code uses.
                let real: String = src
                    .lines()
                    .filter(|l| {
                        let t = l.trim();
                        !t.starts_with("//") && !t.starts_with("///") && !t.starts_with("|")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if real.contains("TcpStream::connect") {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        assert!(
            offenders.is_empty(),
            "these modules bypass the Tor-aware dial chokepoint — \
             use crate::dial::dial() instead: {offenders:?}"
        );
    }
}
