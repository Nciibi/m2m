/// M2M — Local Address Discovery
///
/// Discovers local interface addresses by probing well-known public hosts
/// with UDP sockets. Each probe reveals which local IP the kernel would
/// use as the source address for that outbound path.
///
/// This module owns NO protocol logic — it only looks up local IPs. The
/// STUN, port-mapping, and connection-manager modules each call into here
/// when they need to know the local interface(s) to bind or advertise.
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};

#[cfg(test)]
use std::net::Ipv6Addr;

// ─── IPv4 Host Candidates ──────────────────────────────────────────────────

/// Gather local non-loopback IPv4 addresses that can reach the internet.
///
/// Uses a UDP socket trick that works across all major OSes without
/// requiring external crate dependencies for interface enumeration:
/// connecting to a public IP tells us which local IP the kernel would
/// use as the source.
pub fn gather_host_candidates() -> Vec<SocketAddr> {
    let mut candidates: Vec<SocketAddr> = Vec::new();
    let mut seen_ips: HashSet<IpAddr> = HashSet::new();

    let probes: &[(&str, u16)] = &[
        ("8.8.8.8", 80u16),
        ("1.1.1.1", 443u16),
        ("208.67.222.222", 53u16),
    ];

    for &(ip, port) in probes {
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            let addr_str = format!("{}:{}", ip, port);
            if socket.connect(&addr_str).is_ok() {
                if let Ok(local) = socket.local_addr() {
                    let local_ip = local.ip();
                    if !local_ip.is_loopback()
                        && !local_ip.is_unspecified()
                        && !local_ip.is_multicast()
                        && seen_ips.insert(local_ip)
                    {
                        candidates.push(local);
                    }
                }
            }
        }
    }

    candidates
}

// ─── IPv6 Host Candidates ──────────────────────────────────────────────────

/// Gather local non-loopback global-unicast IPv6 addresses.
///
/// Probes against well-known IPv6 DNS servers. This discovers:
/// - Global unicast addresses (directly routable without NAT)
/// - Unique local addresses (fc00::/7, site-local, lower priority)
///
/// Link-local addresses (fe80::/10) are excluded — they are not routable
/// beyond the local link.
pub fn gather_ipv6_candidates() -> Vec<SocketAddr> {
    let mut candidates: Vec<SocketAddr> = Vec::new();
    let mut seen_ips: HashSet<IpAddr> = HashSet::new();

    let probes: &[(&str, u16)] = &[
        ("2001:4860:4860::8888", 53), // Google DNS
        ("2606:4700:4700::1111", 53), // Cloudflare DNS
        ("2620:fe::fe", 53),          // Quad9 DNS
    ];

    for &(ip, port) in probes {
        if let Ok(socket) = std::net::UdpSocket::bind("[::]:0") {
            let addr_str = format!("[{}]:{}", ip, port);
            if socket.connect(&addr_str).is_ok() {
                if let Ok(local) = socket.local_addr() {
                    let local_ip = local.ip();
                    if !local_ip.is_loopback()
                        && !local_ip.is_unspecified()
                        && !local_ip.is_multicast()
                        && !ipv6_is_link_local(local_ip)
                        && seen_ips.insert(local_ip)
                    {
                        candidates.push(local);
                    }
                }
            }
        }
    }

    candidates
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Returns `true` if `ip` is an IPv6 link-local address (fe80::/10).
pub fn ipv6_is_link_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V6(v6) => {
            let o = v6.octets();
            o[0] == 0xfe && (o[1] & 0xc0) == 0x80
        }
        IpAddr::V4(_) => false,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod local_addr_tests {
    use super::*;

    #[test]
    fn test_ipv6_link_local_detection() {
        let fe80 = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);
        assert!(ipv6_is_link_local(IpAddr::V6(fe80)));

        let global = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
        assert!(!ipv6_is_link_local(IpAddr::V6(global)));

        let v4 = "1.2.3.4".parse::<IpAddr>().unwrap();
        assert!(!ipv6_is_link_local(v4));
    }

    #[test]
    fn test_host_candidate_gathering() {
        let candidates = gather_host_candidates();
        assert!(candidates.len() <= 10, "sanity: shouldn't find dozens of IPs");
    }

    #[test]
    fn test_ipv6_candidate_gathering_no_panic() {
        let candidates = gather_ipv6_candidates();
        // May be empty if the host has no IPv6 connectivity — that's fine.
        assert!(candidates.len() <= 10);
    }
}
