/// M2M — NAT Port Mapping Module
///
/// Unified interface for programmatic NAT port mapping via:
/// - **PCP** (Port Control Protocol, RFC 6887) — newest, most capable
/// - **NAT-PMP** (NAT Port Mapping Protocol, RFC 6886) — simple, Apple-originated
/// - **UPnP IGD** (Internet Gateway Device) — most widely supported on consumer routers
///
/// All three protocols let a device behind a NAT ask the router to forward an
/// external port to an internal one. This module tries them in order (newest
/// first) and returns the first successful mapping.
///
/// ## Architecture
///
/// ```text
/// PortMapper::add_port_mapping(internal_port, lifetime)
///        │
///        ▼
///   Discover gateway (router IP on LAN)
///        │
///        ▼
///   Try PCP ──→ success? ──→ return
///        │
///        no
///        ▼
///   Try NAT-PMP ──→ success? ──→ return
///        │
///        no
///        ▼
///   Try UPnP IGD ──→ success? ──→ return
///        │
///        no
///        ▼
///   Err(AllFailed)
/// ```
///
/// The returned `PortMapping` can be used by the Connection Manager as a
/// ServerReflexive-quality candidate. It is also stored separately so the
/// mapping can be refreshed or removed later.
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use std::sync::Arc;

use tokio::time;

use thiserror::Error;

// ─── Public API ─────────────────────────────────────────────────────────────

/// Maximum bytes read from a UPnP IGD HTTP response (256 KiB).
///
/// A real device description is a few KB and a SOAP response is smaller still,
/// so 256 KiB is generous. It exists because every length in these responses —
/// `Content-Length`, each chunk size — is chosen by whatever host answered our
/// SSDP probe, and each was previously used directly as a `vec![]` length.
/// Rust's allocation-failure handler calls `abort()`, and this binary is built
/// with `panic = "abort"`, so an oversized declared length was a process kill.
const MAX_HTTP_BODY: usize = 256 * 1024;

/// Per-read timeout when talking to a UPnP IGD (5s).
///
/// The `Connection: close` path already had this; the chunked-transfer path
/// had no timeout at all, so a hostile responder could stall the task
/// indefinitely part-way through a body.
const UPNP_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// A successful NAT port mapping from one of the three protocols.
#[derive(Debug, Clone)]
pub struct PortMapping {
    /// Which protocol created the mapping.
    /// One of "pcp", "nat-pmp", "upnp-igd".
    pub protocol: &'static str,
    /// The internal port we bound on this machine.
    pub internal_port: u16,
    /// The public (WAN) IP and port the router forwards to us.
    /// This is what remote peers connect to.
    pub external_addr: SocketAddr,
    /// The lifetime the router granted, in seconds.
    /// Renewal should happen at ~75% of this interval.
    pub lifetime_secs: u32,
}

/// Errors from port-mapping attempts.
#[derive(Debug, Error)]
pub enum PortMapError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no router/gateway found on the local network")]
    NoGateway,
    #[error("PCP mapping failed: {0}")]
    Pcp(String),
    #[error("NAT-PMP mapping failed: {0}")]
    NatPmp(String),
    #[error("UPnP IGD mapping failed: {0}")]
    Upnp(String),
    #[error("all three mapping protocols (PCP, NAT-PMP, UPnP IGD) failed")]
    AllFailed,
    #[error(
        "port mapping skipped: Tor is enabled, and PCP / NAT-PMP / SSDP are UDP \
         queries that would go out from your real address"
    )]
    TorUnsupported,
}

/// Map a LAN-only dial failure onto a port-mapping error.
///
/// Port mapping talks to the user's own router, so under Tor it is refused
/// outright: the request is meaningless (Tor cannot reach a LAN gateway) and
/// is a direct IP-disclosure primitive. The distinct error lets the invite
/// path report *why* no port-mapped candidate appeared.
fn pm_dial_err(e: crate::dial::DialError) -> PortMapError {
    match e {
        crate::dial::DialError::TimedOut(_) => PortMapError::NoGateway,
        crate::dial::DialError::Io(e) => PortMapError::Io(e),
        crate::dial::DialError::NonTorRoutable(a) => {
            PortMapError::Upnp(format!("{a} is not a local gateway address"))
        }
        crate::dial::DialError::TorLanUnsupported(_) => PortMapError::AllFailed,
        // The UDP protocols (PCP / NAT-PMP / SSDP) now go through the UDP
        // chokepoint too, so the Tor refusal reaches them.
        crate::dial::DialError::TorUdpUnsupported => PortMapError::TorUnsupported,
        crate::dial::DialError::Dial(msg) => PortMapError::Upnp(msg),
    }
}

/// Unified port-mapping facade.
///
/// Tries PCP → NAT-PMP → UPnP IGD and returns the first mapping that
/// the router accepted. `AllFailed` means the router supports none of them.
pub struct PortMapper;

impl PortMapper {
    /// Attempt to create a TCP port mapping on the NAT gateway.
    ///
    /// * `internal_port` — the local TCP port we are listening on.
    /// * `lifetime_secs` — requested mapping lifetime (the router may grant less).
    ///
    /// On success the caller should store the returned `PortMapping` and call
    /// `remove_port_mapping` on shutdown or when the mapping is no longer needed.
    pub async fn add_port_mapping(
        internal_port: u16,
        lifetime_secs: u32,
    ) -> Result<PortMapping, PortMapError> {
        let gateway = match discover_gateway().await {
            Some(gw) => gw,
            None => {
                tracing::warn!("cannot discover gateway — port mapping unavailable");
                return Err(PortMapError::NoGateway);
            }
        };

        // ── PCP (newest, most capable) ──
        match pcp_map_tcp(gateway, internal_port, lifetime_secs).await {
            Ok(m) => {
                tracing::info!(protocol = "pcp", external = %m.external_addr, "PCP mapping created");
                return Ok(m);
            }
            Err(e) => tracing::debug!(error = %e, "PCP failed, falling back"),
        }

        // ── NAT-PMP ──
        match nat_pmp_map_tcp(gateway, internal_port, lifetime_secs).await {
            Ok(m) => {
                tracing::info!(protocol = "nat-pmp", external = %m.external_addr, "NAT-PMP mapping created");
                return Ok(m);
            }
            Err(e) => tracing::debug!(error = %e, "NAT-PMP failed, falling back"),
        }

        // ── UPnP IGD (most compatible) ──
        match upnp_map_tcp(internal_port, lifetime_secs).await {
            Ok(m) => {
                tracing::info!(protocol = "upnp-igd", external = %m.external_addr, "UPnP IGD mapping created");
                return Ok(m);
            }
            Err(e) => tracing::debug!(error = %e, "UPnP IGD failed"),
        }

        Err(PortMapError::AllFailed)
    }

    /// Remove a port mapping that was previously created.
    ///
    /// Best-effort — logs failures but does not propagate them to the caller
    /// (the mapping will eventually expire on the router anyway).
    #[expect(dead_code, reason = "Reserved for cleanup on shutdown")]
    pub async fn remove_port_mapping(mapping: &PortMapping) {
        match mapping.protocol {
            "nat-pmp" => {
                if let Err(e) = nat_pmp_remove_tcp(mapping.external_addr.port()).await {
                    tracing::warn!(error = %e, "NAT-PMP remove failed");
                }
            }
            "pcp" => {
                if let Err(e) =
                    pcp_remove_tcp(mapping.internal_port, mapping.external_addr.port()).await
                {
                    tracing::warn!(error = %e, "PCP remove failed");
                }
            }
            "upnp-igd" => {
                if let Err(e) =
                    upnp_remove_tcp(mapping.internal_port, mapping.external_addr.port()).await
                {
                    tracing::warn!(error = %e, "UPnP remove failed");
                }
            }
            other => tracing::warn!(protocol = other, "don't know how to remove this mapping"),
        }
    }

    /// Spawn a background task that automatically renews a port mapping
    /// before the router's lifetime expires.
    ///
    /// The renewal fires at 75% of the mapping's `lifetime_secs` and retries
    /// up to 3 times with exponential backoff before giving up (the mapping
    /// will be re-created on the next invite anyway).
    ///
    /// Returns a handle that can be used to cancel the renewal loop (e.g. on
    /// app shutdown).
    #[expect(dead_code, reason = "Reserved for automatic mapping renewal")]
    pub fn spawn_renewal(mapping: Arc<PortMapping>) -> tokio::sync::watch::Sender<()> {
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(());

        // Compute renewal interval as 75% of the granted lifetime.
        let interval = Duration::from_secs((mapping.lifetime_secs as f64 * 0.75) as u64);

        // Don't bother renewing if the lifetime is ridiculously short.
        if interval < Duration::from_secs(30) {
            tracing::warn!(
                lifetime = mapping.lifetime_secs,
                "mapping lifetime too short for automatic renewal"
            );
            return cancel_tx;
        }

        let mapping = mapping.clone();
        tokio::spawn(async move {
            loop {
                // Wait for the renewal interval or cancellation.
                tokio::select! {
                    _ = time::sleep(interval) => {}
                    _ = cancel_rx.changed() => {
                        tracing::debug!("port mapping renewal cancelled");
                        return;
                    }
                }

                // Renew with up to 3 retries.
                let mut retries = 0u32;
                loop {
                    tracing::info!(
                        protocol = mapping.protocol,
                        external = %mapping.external_addr,
                        attempt = retries + 1,
                        "renewing port mapping"
                    );

                    let result = match mapping.protocol {
                        "nat-pmp" => {
                            // NAT-PMP: re-request with the same internal port.
                            let gw = match discover_gateway().await {
                                Some(g) => g,
                                None => {
                                    tracing::warn!("cannot discover gateway for NAT-PMP renewal");
                                    break;
                                }
                            };
                            nat_pmp_map_tcp(gw, mapping.internal_port, mapping.lifetime_secs).await
                        }
                        "pcp" => {
                            let gw = match discover_gateway().await {
                                Some(g) => g,
                                None => {
                                    tracing::warn!("cannot discover gateway for PCP renewal");
                                    break;
                                }
                            };
                            pcp_map_tcp(gw, mapping.internal_port, mapping.lifetime_secs).await
                        }
                        "upnp-igd" => {
                            upnp_map_tcp(mapping.internal_port, mapping.lifetime_secs).await
                        }
                        other => {
                            tracing::warn!(
                                protocol = other,
                                "don't know how to renew this mapping"
                            );
                            break;
                        }
                    };

                    match result {
                        Ok(_) => {
                            tracing::info!(
                                protocol = mapping.protocol,
                                "port mapping renewed successfully"
                            );
                            break;
                        }
                        Err(e) => {
                            retries += 1;
                            if retries >= 3 {
                                tracing::error!(
                                    error = %e,
                                    "port mapping renewal failed after 3 retries"
                                );
                                break;
                            }
                            tracing::warn!(error = %e, retry = retries, "renewal attempt failed, retrying");
                            time::sleep(Duration::from_secs(2u64.pow(retries))).await;
                        }
                    }
                }
            }
        });

        cancel_tx
    }
}

// ─── Gateway Discovery ──────────────────────────────────────────────────────

/// Discover the default gateway using the system routing table.
///
/// Strategy (tried in order):
/// 1. **Linux** — parse `/proc/net/route` (no process spawning needed).
/// 2. **macOS** — run `route -n get default` and parse the output.
/// 3. **Windows** — run `route print 0.0.0.0` and parse the output.
/// 4. **Fallback** — probe common gateway addresses (last resort).
///
/// Returns the gateway's LAN IP address.
///
/// Only an address that has actually been shown to be a router is returned —
/// see `discover_gateway_fallback` for why the unverified guess it used to hand
/// back is no longer usable.
async fn discover_gateway() -> Option<IpAddr> {
    // ── Strategy 1: Linux /proc/net/route ──
    // Format (header + one line per route):
    //   Iface   Destination  Gateway      Flags ...
    //   eth0    00000000     0123A8C0     ...
    // The default route has Destination=00000000.
    // The gateway is in hex, reversed byte order.
    if let Ok(route) = std::fs::read_to_string("/proc/net/route") {
        for line in route.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 3 && fields[1] == "00000000" {
                if let Ok(gw) = parse_hex_ipv4(fields[2]) {
                    tracing::debug!(gateway = %gw, source = "/proc/net/route");
                    return Some(IpAddr::V4(gw));
                }
            }
        }
    }

    // ── Strategy 2: macOS / BSD `route -n get default` ──
    // Output line: "gateway: 192.168.1.1"
    if let Ok(output) = std::process::Command::new("route")
        .args(["-n", "get", "default"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("gateway:") {
                    if let Ok(ip) = val.trim().parse::<IpAddr>() {
                        tracing::debug!(gateway = %ip, source = "route -n get default");
                        return Some(ip);
                    }
                }
            }
        }
    }

    // ── Strategy 3: Windows `route print 0.0.0.0` ──
    // Output lines look like:
    //   0.0.0.0          0.0.0.0    192.168.1.1    192.168.1.5     25
    if cfg!(target_os = "windows") {
        if let Ok(output) = std::process::Command::new("route")
            .args(["print", "0.0.0.0"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let fields: Vec<&str> = line.split_whitespace().collect();
                    if fields.len() >= 3 && fields[0] == "0.0.0.0" {
                        if let Ok(ip) = fields[2].parse::<IpAddr>() {
                            tracing::debug!(gateway = %ip, source = "route print");
                            return Some(ip);
                        }
                    }
                }
            }
        }
    }

    // ── Strategy 4: Fallback (probe common gateways via NAT-PMP) ──
    //
    // The fallback can only ever produce a *proved* router or nothing. Handing
    // the guess through here would put a LAN-only protocol on the wire to an
    // address we know nothing about (F4) — see `discover_gateway_fallback`.
    match discover_gateway_fallback().await? {
        GatewayCandidate::Verified(ip) => Some(ip),
        GatewayCandidate::Unverified(ip) => {
            // Nothing answered a NAT-PMP request at any candidate, so we have no
            // evidence that *any* of them is a router. PCP and NAT-PMP are LAN
            // protocols: sending them to a guess means sending them to an internet
            // host (or to nothing at all), and `add_port_mapping` would then report
            // a mapping built from an address nobody verified. Fail closed.
            tracing::warn!(
                gateway = %ip,
                "no router answered a NAT-PMP probe; refusing to send LAN port-mapping \
                 requests to an unverified address"
            );
            None
        }
    }
}

/// How much is known about an address returned by the fallback discovery path.
///
/// `Option<IpAddr>` could not carry this: the old code returned a guess on the
/// same path as a probe-confirmed router, and every caller treated both as a
/// gateway to send PCP / NAT-PMP datagrams to.
#[derive(Debug, Clone, Copy)]
enum GatewayCandidate {
    /// The address answered a NAT-PMP public-address request, which only a
    /// NAT-capable router on this LAN does.
    Verified(IpAddr),
    /// Nothing proved this address is a router — it was derived from our own
    /// subnet or from a hard-coded list. Must never be used as a mapping target.
    Unverified(IpAddr),
}

/// Parse an IPv4 address from `/proc/net/route` hex format.
///
/// The gateway is stored as a little‑endian hex string without leading "0x".
/// Example: `0123A8C0` → `192.168.1.1`
fn parse_hex_ipv4(hex: &str) -> Result<Ipv4Addr, ()> {
    let val = u32::from_str_radix(hex, 16).map_err(|_| ())?;
    let octets = val.to_le_bytes();
    Ok(Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]))
}

/// Fallback gateway discovery: probe common addresses (last resort).
///
/// 1. Determine the local interface IP by binding a UDP socket.
/// 2. Try `.1` and `.254` on the same /24 subnet — **only when that address is
///    private**, because a publicly-addressed host would otherwise have us probe
///    `.1` of its own public /24.
/// 3. Append a list of well-known gateway addresses.
/// 4. Send a NAT-PMP public-address request to each — the first to
///    respond is confirmed as the real gateway.
/// 5. If nothing responded, return the first candidate *marked unverified*.
///
/// **F4 — why the responder's address, not its reply, is returned.** Step 4's
/// probe carries the router's *external* address in the reply body
/// (`nat_pmp_public_address`). The old code returned that value as "the
/// gateway", so the caller sent PCP and NAT-PMP — LAN protocols, port 5351 — to
/// a public internet address. What has been proved to be a router is the
/// address we *probed*, so that is what comes back.
///
/// **F4 — why step 5 is unusable.** The candidate in step 5 is `.1` of whatever
/// subnet we guessed, with zero evidence behind it. It is returned wrapped in
/// `Unverified` so `discover_gateway` can refuse it instead of turning it into a
/// mapping attempt against an address that was never shown to be a router.
async fn discover_gateway_fallback() -> Option<GatewayCandidate> {
    // Learn our local interface IP.
    let local_ip = {
        let sock = crate::dial::bind_udp_for_external_query().await.ok()?;
        sock.connect("8.8.8.8:53").await.ok()?;
        sock.local_addr().ok()?.ip()
    };

    let common: &[[u8; 4]] = &[
        [192, 168, 0, 1],
        [192, 168, 1, 1],
        [192, 168, 1, 254],
        [10, 0, 0, 1],
        [10, 0, 1, 1],
        [172, 16, 0, 1],
        [192, 168, 0, 254],
        [10, 0, 0, 138],
    ];

    let candidates: Vec<Ipv4Addr> = if let IpAddr::V4(v4) = local_ip {
        let octets = v4.octets();
        let mut list: Vec<Ipv4Addr> = Vec::new();
        // `sock.connect("8.8.8.8:53")` is a connect-only trick to pick a source
        // address, and on a host with a public interface it yields the *public*
        // IP. Sibling-derived candidates are therefore only meaningful when the
        // host is private; otherwise `.1` of that /24 is an arbitrary internet
        // address and probing it sends a LAN-protocol datagram off-LAN.
        if v4.is_private() {
            list.push(Ipv4Addr::new(octets[0], octets[1], octets[2], 1));
            list.push(Ipv4Addr::new(octets[0], octets[1], octets[2], 254));
        } else {
            tracing::debug!(
                local_ip = %v4,
                "not deriving sibling gateway candidates: source address is not private"
            );
        }
        for gw in common {
            let a = Ipv4Addr::new(gw[0], gw[1], gw[2], gw[3]);
            if !list.contains(&a) {
                list.push(a);
            }
        }
        list
    } else {
        common
            .iter()
            .map(|o| Ipv4Addr::new(o[0], o[1], o[2], o[3]))
            .collect()
    };

    for gw in &candidates {
        let addr = SocketAddr::new(IpAddr::V4(*gw), 5351);
        if let Ok(probe) = nat_pmp_public_address(&addr).await {
            tracing::info!(gateway = %gw, public_ip = %probe, "gateway discovered via NAT-PMP probe");
            // The *probed* address is the router. `probe` is the external address
            // the router reported and is not a valid LAN mapping target.
            return Some(GatewayCandidate::Verified(IpAddr::V4(*gw)));
        }
    }

    // Nothing responded: hand back the first guess, explicitly marked unverified
    // so the caller refuses to send it a mapping request.
    let guess = candidates.first().copied().map(IpAddr::V4);
    tracing::warn!("no gateway answered a NAT-PMP probe");
    guess.map(GatewayCandidate::Unverified)
}

// ─── NAT-PMP (RFC 6886) ─────────────────────────────────────────────────────

/// NAT-PMP version.
const NAT_PMP_VERSION: u8 = 0;

/// Opcode: public-address request.
const NAT_PMP_OP_PUBADDR: u8 = 0;

/// Opcode: map TCP port.
const NAT_PMP_OP_MAP_TCP: u8 = 2;

/// Response flag.
const NAT_PMP_RESP: u8 = 128;

/// Success result code.
const NAT_PMP_SUCCESS: u16 = 0;

/// NAT-PMP request/response timeout.
const NAT_PMP_TIMEOUT: Duration = Duration::from_secs(3);

/// Send a NAT-PMP public-address request and return the WAN IP.
async fn nat_pmp_public_address(gateway: &SocketAddr) -> Result<IpAddr, PortMapError> {
    let sock = crate::dial::bind_udp_for_external_query()
        .await
        .map_err(pm_dial_err)?;
    sock.connect(gateway).await?;

    // Request: [version=0, op=0] (2 bytes)
    let req = [NAT_PMP_VERSION, NAT_PMP_OP_PUBADDR];
    sock.send(&req).await?;

    // Response: [ver, 128|0, result=2B, epoch=4B, public_ip=4B] (12 bytes)
    let mut buf = [0u8; 12];
    let n = time::timeout(NAT_PMP_TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| PortMapError::NatPmp("public-address request timed out".into()))?
        .map_err(PortMapError::Io)?;

    if n < 12 {
        return Err(PortMapError::NatPmp(format!("short response: {} bytes", n)));
    }
    if buf[1] != NAT_PMP_RESP | NAT_PMP_OP_PUBADDR {
        return Err(PortMapError::NatPmp(format!(
            "unexpected opcode: {}",
            buf[1]
        )));
    }
    let result = u16::from_be_bytes([buf[2], buf[3]]);
    if result != NAT_PMP_SUCCESS {
        return Err(PortMapError::NatPmp(format!(
            "public-address error: result={}",
            result
        )));
    }
    let ip_bytes: [u8; 4] = [buf[8], buf[9], buf[10], buf[11]];
    Ok(IpAddr::V4(Ipv4Addr::from(ip_bytes)))
}

/// Request a TCP port mapping via NAT-PMP.
///
/// Returns the external (WAN) address of the mapping.
async fn nat_pmp_map_tcp(
    gateway: IpAddr,
    internal_port: u16,
    lifetime_secs: u32,
) -> Result<PortMapping, PortMapError> {
    let gw = SocketAddr::new(gateway, 5351);
    let sock = crate::dial::bind_udp_for_external_query()
        .await
        .map_err(pm_dial_err)?;
    sock.connect(gw).await?;

    // Request: [ver=0, op=2, reserved=2B, int_port=2B, ext_port=2B, lifetime=4B] (12 bytes)
    let mut req = [0u8; 12];
    req[0] = NAT_PMP_VERSION;
    req[1] = NAT_PMP_OP_MAP_TCP;
    req[4..6].copy_from_slice(&internal_port.to_be_bytes()); // internal port
                                                             // external port = 0 means "let the router choose"
    req[8..12].copy_from_slice(&lifetime_secs.to_be_bytes());
    sock.send(&req).await?;

    // Response: [ver, 130, result=2B, epoch=4B, int_port=2B, ext_port=2B, lifetime=4B] (16 bytes)
    let mut buf = [0u8; 16];
    let n = time::timeout(NAT_PMP_TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| PortMapError::NatPmp("map request timed out".into()))?
        .map_err(PortMapError::Io)?;
    if n < 16 {
        return Err(PortMapError::NatPmp(format!("short response: {} bytes", n)));
    }
    if buf[1] != NAT_PMP_RESP | NAT_PMP_OP_MAP_TCP {
        return Err(PortMapError::NatPmp(format!(
            "unexpected opcode: {}",
            buf[1]
        )));
    }
    let result = u16::from_be_bytes([buf[2], buf[3]]);
    if result != NAT_PMP_SUCCESS {
        return Err(PortMapError::NatPmp(format!(
            "router rejected mapping: result code {}",
            result
        )));
    }
    let ext_port = u16::from_be_bytes([buf[10], buf[11]]);
    let mapped_lifetime = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);

    // Get the router's WAN IP via a separate public-address request.
    let public_ip = nat_pmp_public_address(&gw).await?;

    tracing::debug!(
        ext_port = ext_port,
        lifetime = mapped_lifetime,
        "NAT-PMP mapping granted"
    );

    Ok(PortMapping {
        protocol: "nat-pmp",
        internal_port,
        external_addr: SocketAddr::new(public_ip, ext_port),
        lifetime_secs: mapped_lifetime,
    })
}

/// Remove a NAT-PMP TCP mapping by requesting lifetime=0 for the external port.
async fn nat_pmp_remove_tcp(external_port: u16) -> Result<(), PortMapError> {
    let gateway = match discover_gateway().await {
        Some(g) => g,
        None => return Err(PortMapError::NoGateway),
    };

    let sock = crate::dial::bind_udp_for_external_query()
        .await
        .map_err(pm_dial_err)?;
    sock.connect(SocketAddr::new(gateway, 5351)).await?;

    let mut req = [0u8; 12];
    req[0] = NAT_PMP_VERSION;
    req[1] = NAT_PMP_OP_MAP_TCP;
    req[6..8].copy_from_slice(&external_port.to_be_bytes()); // external port
    req[8..12].copy_from_slice(&0u32.to_be_bytes()); // lifetime = 0 → remove
    sock.send(&req).await?;

    let mut buf = [0u8; 16];
    let n = time::timeout(NAT_PMP_TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| PortMapError::NatPmp("remove request timed out".into()))?
        .map_err(PortMapError::Io)?;

    if n < 16 {
        return Err(PortMapError::NatPmp("short remove response".into()));
    }
    let result = u16::from_be_bytes([buf[2], buf[3]]);
    if result != NAT_PMP_SUCCESS {
        return Err(PortMapError::NatPmp(format!(
            "remove failed: result={}",
            result
        )));
    }
    Ok(())
}

// ─── PCP (RFC 6887) ─────────────────────────────────────────────────────────

/// PCP version (RFC 6887).
const PCP_VERSION: u8 = 2;

/// Opcode: MAP.
const PCP_OP_MAP: u8 = 1;

/// Success result code.
const PCP_SUCCESS: u8 = 0;

/// PCP request/response timeout.
const PCP_TIMEOUT: Duration = Duration::from_secs(3);

/// Build an RFC 6887 §12.1 compliant MAP request for the given internal port.
///
/// Packet layout (24‑byte header + 26‑byte MAP body = 50 bytes total):
///
/// ```text
///  Bytes    Field
///  0        Version (2)
///  1        Opcode (1 = MAP)
///  2-3      Reserved
///  4-7      Requested Lifetime
///  8-23     Client IP (16 bytes, zero = let router decide)
///  24-26    Reserved (MAP body)
///  27       Protocol (6 = TCP)
///  28-29    Reserved
///  30-31    Internal Port (big-endian)
///  32-33    Suggested External Port (0 = let router choose)
///  34-49    Requested External IP (16 bytes, zero = any)
/// ```
const PCP_MAP_REQUEST_SIZE: usize = 50;
/// PCP MAP request field offsets (0‑based from start of packet).
const PCP_OFF_OP: usize = 1;
const PCP_OFF_RESULT: usize = 3;
const PCP_OFF_LIFETIME: usize = 4;
/// PCP header size: 24 bytes (RFC 6887 §7.3).
const PCP_HEADER_SIZE: usize = 24;
// Constants used by future PCP features (ECHO REQUEST, THIRD PARTY, etc.).
#[expect(dead_code, reason = "Reserved for future PCP features")]
const PCP_OFF_CLIENT_IP: usize = 8;
#[expect(dead_code, reason = "Reserved for future PCP features")]
const PCP_OFF_BODY_RESERVED: usize = 24;
#[expect(dead_code, reason = "Reserved for future PCP features")]
const PCP_OFF_RESERVED2: usize = 28;
const PCP_OFF_PROTOCOL: usize = 27;
const PCP_OFF_INT_PORT: usize = 30;
const PCP_OFF_EXT_PORT: usize = 32;
const PCP_OFF_EXT_IP: usize = 34;

fn build_pcp_map_request(
    lifetime_secs: u32,
    internal_port: u16,
    external_port: u16,
) -> [u8; PCP_MAP_REQUEST_SIZE] {
    let mut req = [0u8; PCP_MAP_REQUEST_SIZE];
    req[0] = PCP_VERSION;
    req[PCP_OFF_OP] = PCP_OP_MAP;
    req[PCP_OFF_LIFETIME..PCP_OFF_LIFETIME + 4].copy_from_slice(&lifetime_secs.to_be_bytes());
    req[PCP_OFF_PROTOCOL] = 6; // IPPROTO_TCP
    req[PCP_OFF_INT_PORT..PCP_OFF_INT_PORT + 2].copy_from_slice(&internal_port.to_be_bytes());
    req[PCP_OFF_EXT_PORT..PCP_OFF_EXT_PORT + 2].copy_from_slice(&external_port.to_be_bytes());
    // Body reserved, client IP, reserved2, and external IP are already zeroed.
    req
}

/// PCP MAP request (RFC 6887 §12.1).
///
/// Builds a 50‑byte MAP request packet. The 96‑bit authentication nonce
/// is zero‑filled (implicitly — the `[]` initialiser sets all bytes to
/// zero). Many residential routers accept this; enterprise gateways may
/// require a proper nonce exchange (ECHO REQUEST / ECHO RESPONSE).
async fn pcp_map_tcp(
    gateway: IpAddr,
    internal_port: u16,
    lifetime_secs: u32,
) -> Result<PortMapping, PortMapError> {
    let gw = SocketAddr::new(gateway, 5351);
    let sock = crate::dial::bind_udp_for_external_query()
        .await
        .map_err(pm_dial_err)?;
    sock.connect(gw).await?;

    let req = build_pcp_map_request(lifetime_secs, internal_port, 0);
    sock.send(&req).await?;

    let mut buf = [0u8; PCP_MAP_REQUEST_SIZE];
    let n = time::timeout(PCP_TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| PortMapError::Pcp("MAP request timed out".into()))?
        .map_err(PortMapError::Io)?;

    if n < PCP_MAP_REQUEST_SIZE {
        return Err(PortMapError::Pcp(format!(
            "short response: {} bytes (expected {})",
            n, PCP_MAP_REQUEST_SIZE
        )));
    }

    let result = buf[PCP_OFF_RESULT];
    if result != PCP_SUCCESS {
        return Err(PortMapError::Pcp(format!(
            "router rejected PCP mapping: result code {}",
            result
        )));
    }

    let mapped_lifetime = u32::from_be_bytes([
        buf[PCP_OFF_LIFETIME],
        buf[PCP_OFF_LIFETIME + 1],
        buf[PCP_OFF_LIFETIME + 2],
        buf[PCP_OFF_LIFETIME + 3],
    ]);
    let external_port = u16::from_be_bytes([buf[PCP_OFF_EXT_PORT], buf[PCP_OFF_EXT_PORT + 1]]);
    let ext_ip = IpAddr::V4(Ipv4Addr::new(
        buf[PCP_OFF_EXT_IP],
        buf[PCP_OFF_EXT_IP + 1],
        buf[PCP_OFF_EXT_IP + 2],
        buf[PCP_OFF_EXT_IP + 3],
    ));

    tracing::debug!(
        lifetime = mapped_lifetime,
        external = %SocketAddr::new(ext_ip, external_port),
        "PCP mapping granted"
    );

    Ok(PortMapping {
        protocol: "pcp",
        internal_port,
        external_addr: SocketAddr::new(ext_ip, external_port),
        lifetime_secs: mapped_lifetime,
    })
}

/// Remove a PCP mapping by requesting lifetime=0.
async fn pcp_remove_tcp(internal_port: u16, external_port: u16) -> Result<(), PortMapError> {
    let gateway = match discover_gateway().await {
        Some(g) => g,
        None => return Err(PortMapError::NoGateway),
    };

    let sock = crate::dial::bind_udp_for_external_query()
        .await
        .map_err(pm_dial_err)?;
    sock.connect(SocketAddr::new(gateway, 5351)).await?;

    // Lifetime = 0 signals deletion.
    let req = build_pcp_map_request(0, internal_port, external_port);
    sock.send(&req).await?;

    let mut buf = [0u8; PCP_MAP_REQUEST_SIZE];
    let n = time::timeout(PCP_TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| PortMapError::Pcp("remove timed out".into()))?
        .map_err(PortMapError::Io)?;
    if n < PCP_HEADER_SIZE {
        return Err(PortMapError::Pcp("short remove response".into()));
    }
    if buf[PCP_OFF_RESULT] != PCP_SUCCESS {
        return Err(PortMapError::Pcp(format!(
            "remove failed: result={}",
            buf[PCP_OFF_RESULT]
        )));
    }
    Ok(())
}

// ─── UPnP IGD ───────────────────────────────────────────────────────────────

/// SSDP multicast address for UPnP device discovery.
const SSDP_ADDR: &str = "239.255.255.250:1900";

/// SSDP M-SEARCH discovery request body.
const SSDP_MSEARCH: &[u8] = b"M-SEARCH * HTTP/1.1\r\n\
    HOST: 239.255.255.250:1900\r\n\
    MAN: \"ssdp:discover\"\r\n\
    MX: 3\r\n\
    ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\
    \r\n";

/// UPnP action template for `AddPortMapping`.
const SOAP_ADD_PORT: &str = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:AddPortMapping xmlns:u="urn:schemas-upnp-org:service:WANIPConnection:1">
      <NewRemoteHost></NewRemoteHost>
      <NewExternalPort>{external_port}</NewExternalPort>
      <NewProtocol>TCP</NewProtocol>
      <NewInternalPort>{internal_port}</NewInternalPort>
      <NewInternalClient>{internal_client}</NewInternalClient>
      <NewEnabled>1</NewEnabled>
      <NewPortMappingDescription>M2M Messenger</NewPortMappingDescription>
      <NewLeaseDuration>{lease_duration}</NewLeaseDuration>
    </u:AddPortMapping>
  </s:Body>
</s:Envelope>"#;

/// UPnP action template for `DeletePortMapping`.
const SOAP_DELETE_PORT: &str = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:DeletePortMapping xmlns:u="urn:schemas-upnp-org:service:WANIPConnection:1">
      <NewRemoteHost></NewRemoteHost>
      <NewExternalPort>{external_port}</NewExternalPort>
      <NewProtocol>TCP</NewProtocol>
    </u:DeletePortMapping>
  </s:Body>
</s:Envelope>"#;

/// Container for UPnP service URLs discovered via SSDP.
struct UpnpService {
    /// The control URL for the WANIPConnection service.
    control_url: String,
}

/// Discover the UPnP IGD device and return its WANIPConnection control URL.
///
/// Steps:
/// 1. Send SSDP M-SEARCH multicast.
/// 2. Parse the `LOCATION` header from the first IGD response.
/// 3. Fetch the device description XML.
/// 4. Extract the WANIPConnection service's `controlURL`.
async fn upnp_discover() -> Result<UpnpService, PortMapError> {
    let sock = crate::dial::bind_udp_for_external_query()
        .await
        .map_err(pm_dial_err)?;
    sock.set_broadcast(true)?;
    let ssdp_addr: SocketAddr = SSDP_ADDR
        .parse()
        .map_err(|_| PortMapError::Upnp("invalid SSDP address".into()))?;

    sock.send_to(SSDP_MSEARCH, ssdp_addr).await?;

    // Collect responses for 3 seconds.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
    let mut location_url: Option<String> = None;

    while tokio::time::Instant::now() < deadline {
        let mut buf = [0u8; 2048];
        let remaining = deadline - tokio::time::Instant::now();
        if remaining.is_zero() {
            break;
        }

        let (n, _src) = match time::timeout(remaining, sock.recv_from(&mut buf)).await {
            Ok(Ok(r)) => r,
            _ => break,
        };

        let resp = String::from_utf8_lossy(&buf[..n]);

        // Look for the LOCATION header which points to the device description XML.
        if resp.contains("InternetGatewayDevice")
            || resp.contains("urn:schemas-upnp-org:device:InternetGatewayDevice")
        {
            for line in resp.lines() {
                let lower = line.to_lowercase();
                if lower.starts_with("location:") {
                    if let Some(url) = line.split_once(':').map(|x| x.1) {
                        location_url = Some(url.trim().to_string());
                        break;
                    }
                }
            }
            if location_url.is_some() {
                break;
            }
        }
    }

    let location = location_url
        .ok_or_else(|| PortMapError::Upnp("no UPnP IGD device found on the network".into()))?;

    // ── SSRF guard: only follow a LOCATION that points at a private gateway ──
    //
    // `LOCATION` is a header in an SSDP reply, and *anything* on the LAN can
    // send one. Following it verbatim turns M2M into a request forwarder: a
    // hostile host answering our `M-SEARCH` with
    // `LOCATION: http://169.254.169.254/latest/meta-data/` makes us issue a
    // GET to the cloud metadata service, and the connection outcome (or a
    // distinguishable error) is a working internal port-scan primitive.
    //
    // UPnP IGD devices are, by definition, on the local network: RFC 6742
    // expects the control URL to live on the gateway. So restricting the
    // destination to private/loopback/link-local addresses costs nothing in
    // practice and removes the whole class of attack.
    let location = validate_upnp_location(&location)?;

    // Now fetch the device description XML and find the WANIPConnection control URL.
    let control_url = upnp_parse_description(&location).await?;

    Ok(UpnpService { control_url })
}

/// Maximum size of a `LOCATION` URL we will act on.
const MAX_UPNP_LOCATION_LEN: usize = 512;

/// Validate an SSDP `LOCATION` header before connecting to it.
///
/// Requires a plain `http://` URL whose host is a literal private, loopback or
/// link-local IP — i.e. something that could plausibly be the local gateway.
/// Everything else (HTTPS, a hostname, a public or documentation address) is
/// rejected outright rather than resolved, so no DNS lookup is performed on an
/// attacker-supplied name.
fn validate_upnp_location(raw: &str) -> Result<String, PortMapError> {
    let url = raw.trim();
    if url.is_empty() {
        return Err(PortMapError::Upnp("empty UPnP LOCATION header".into()));
    }
    if url.len() > MAX_UPNP_LOCATION_LEN {
        return Err(PortMapError::Upnp("UPnP LOCATION header too long".into()));
    }
    // Only plain HTTP. An IGD never needs TLS, and accepting `https` would mean
    // trusting a certificate on a device that has no identity story.
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| PortMapError::Upnp("UPnP LOCATION must be a plain http:// URL".into()))?;
    // Authority ends at the first '/', '?' or '#'.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(PortMapError::Upnp("UPnP LOCATION has no host".into()));
    }
    // Reject userinfo (`user@host`) outright — a classic parser-confusion trick.
    if authority.contains('@') {
        return Err(PortMapError::Upnp(
            "UPnP LOCATION must not contain userinfo".into(),
        ));
    }
    // IPv6 literals are bracketed: [::1]:80
    let (host, _port) = if let Some(after_bracket) = authority.strip_prefix('[') {
        let end = after_bracket
            .find(']')
            .ok_or_else(|| PortMapError::Upnp("malformed IPv6 in UPnP LOCATION".into()))?;
        (&after_bracket[..end], &after_bracket[end + 1..])
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h, p),
            _ => (authority, ""),
        }
    };

    // Must be a literal IP. A hostname is rejected rather than resolved: a
    // DNS name is attacker-controlled and could point anywhere, including at
    // rebind-style tricks.
    let ip: IpAddr = host.parse().map_err(|_| {
        PortMapError::Upnp(format!(
            "UPnP LOCATION host '{host}' is not a literal IP address; refusing to \
             resolve an attacker-supplied name"
        ))
    })?;

    // Must look like a local gateway.
    let is_local = match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                // Unique local address, fc00::/7.
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // Link-local, fe80::/10.
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    };
    if !is_local {
        return Err(PortMapError::Upnp(format!(
            "UPnP LOCATION host {ip} is not a local-network address; refusing to \
             fetch a device description from a non-gateway host"
        )));
    }

    // ── Cloud instance-metadata deny list ──
    //
    // These addresses ARE link-local, so the check above lets them through — and
    // they are precisely the prize for an SSRF: a signed `LOCATION` pointing at
    // the metadata service turns M2M into a credential-exfiltration primitive on
    // any cloud-hosted instance. Blocked explicitly.
    //
    // * 169.254.169.254 — AWS IMDS, GCP, Azure, DigitalOcean, OpenStack
    // * fd00:ec2::254     — AWS IMDS over IPv6
    // * 100.100.100.200  — Alibaba Cloud
    // * 192.0.0.192      — Oracle Cloud
    // * metadata.google.internal resolves to 169.254.169.254, and hostnames are
    //   already refused above, so no name-based entry is needed.
    const METADATA_ADDRS: &[(std::net::Ipv4Addr, std::net::Ipv6Addr)] = &[
        (
            std::net::Ipv4Addr::new(169, 254, 169, 254),
            std::net::Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254),
        ),
        (
            std::net::Ipv4Addr::new(100, 100, 100, 200),
            std::net::Ipv6Addr::UNSPECIFIED,
        ),
        (
            std::net::Ipv4Addr::new(192, 0, 0, 192),
            std::net::Ipv6Addr::UNSPECIFIED,
        ),
    ];
    for (v4, v6) in METADATA_ADDRS {
        let hit = match ip {
            IpAddr::V4(a) => a == *v4,
            IpAddr::V6(a) => *v6 != std::net::Ipv6Addr::UNSPECIFIED && a == *v6,
        };
        if hit {
            return Err(PortMapError::Upnp(format!(
                "UPnP LOCATION host {ip} is a cloud instance-metadata endpoint; \
                 refusing to fetch it (SSRF / credential-exfiltration guard)"
            )));
        }
    }

    Ok(url.to_string())
}

/// Read an HTTP response body from a stream, handling both Content-Length
/// and Transfer-Encoding: chunked, plus plain `Connection: close` fallback.
///
/// Returns the response status code and body bytes.
async fn read_http_response_body<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<(u16, Vec<u8>), PortMapError> {
    use tokio::io::AsyncReadExt;

    let mut buf = [0u8; 4096];
    let mut header_bytes = Vec::with_capacity(2048);

    // ── Read until headers are complete (double CRLF or double LF) ──
    loop {
        let n = time::timeout(Duration::from_secs(5), reader.read(&mut buf))
            .await
            .map_err(|_| PortMapError::Upnp("HTTP header read timed out".into()))?
            .map_err(PortMapError::Io)?;
        if n == 0 {
            break;
        }
        header_bytes.extend_from_slice(&buf[..n]);
        let hdrs = String::from_utf8_lossy(&header_bytes);
        if hdrs.contains("\r\n\r\n") || hdrs.contains("\n\n") {
            break;
        }
        if header_bytes.len() > 8192 {
            return Err(PortMapError::Upnp("HTTP headers too large".into()));
        }
    }

    let hdrs = String::from_utf8_lossy(&header_bytes);

    // ── Parse status line ──
    // "HTTP/1.1 200 OK\r\n..."
    let status = hdrs
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);

    // Find header/body boundary.
    let body_start = if let Some(pos) = hdrs.find("\r\n\r\n") {
        pos + 4
    } else if let Some(pos) = hdrs.find("\n\n") {
        pos + 2
    } else {
        0
    };

    // Helper to get a header value (case-insensitive).
    let get_header = |name: &str| -> Option<String> {
        let lower_name = name.to_lowercase();
        for line in hdrs.lines() {
            let lower_line = line.to_lowercase();
            if lower_line.starts_with(&lower_name) {
                if let Some(val) = line.split_once(':').map(|x| x.1) {
                    return Some(val.trim().to_string());
                }
            }
        }
        None
    };

    let mut body: Vec<u8> = header_bytes[body_start..].to_vec();

    // ── Determine how to read the body ──
    if let Some(te) = get_header("Transfer-Encoding") {
        if te.to_lowercase().contains("chunked") {
            // Chunked transfer encoding.
            loop {
                let mut line_buf = Vec::with_capacity(128);
                loop {
                    let mut byte = [0u8; 1];
                    let got = time::timeout(UPNP_READ_TIMEOUT, reader.read(&mut byte))
                        .await
                        .map_err(|_| PortMapError::Upnp("HTTP chunk header read timed out".into()))?
                        .map_err(PortMapError::Io)?;
                    if got == 0 {
                        break;
                    }
                    if byte[0] == b'\n' {
                        break;
                    }
                    if byte[0] != b'\r' {
                        line_buf.push(byte[0]);
                    }
                }
                if line_buf.is_empty() {
                    break;
                }
                let chunk_size_str = String::from_utf8_lossy(&line_buf);
                let chunk_size = usize::from_str_radix(chunk_size_str.trim(), 16)
                    .map_err(|_| PortMapError::Upnp("invalid chunk size".into()))?;
                if chunk_size == 0 {
                    break; // End of chunks
                }
                // The chunk size comes from an unauthenticated HTTP response
                // and was previously used directly as an allocation length, so
                // a hostile "router" could ask for a 68 GB buffer. Rust's
                // allocation-failure path aborts the process, and this binary
                // is built with `panic = "abort"`.
                if chunk_size > MAX_HTTP_BODY {
                    return Err(PortMapError::Upnp(format!(
                        "chunk size {chunk_size} exceeds the {MAX_HTTP_BODY} byte limit"
                    )));
                }
                if body.len() + chunk_size > MAX_HTTP_BODY {
                    return Err(PortMapError::Upnp(format!(
                        "response body exceeds the {MAX_HTTP_BODY} byte limit"
                    )));
                }
                let mut chunk = vec![0u8; chunk_size];
                let mut read_total = 0;
                while read_total < chunk_size {
                    // Bounded like every other read here — the chunked path
                    // previously had no timeout at all, so a hostile peer could
                    // stall the task forever mid-body.
                    let n = time::timeout(UPNP_READ_TIMEOUT, reader.read(&mut chunk[read_total..]))
                        .await
                        .map_err(|_| PortMapError::Upnp("HTTP chunk read timed out".into()))?
                        .map_err(PortMapError::Io)?;
                    if n == 0 {
                        break;
                    }
                    read_total += n;
                }
                body.extend_from_slice(&chunk);
                // Consume trailing CRLF.
                let mut trail = [0u8; 2];
                let _ = time::timeout(UPNP_READ_TIMEOUT, reader.read(&mut trail)).await;
            }
            return Ok((status, body));
        }
    }

    if let Some(cl) = get_header("Content-Length") {
        // `Content-Length` is attacker-chosen (the responder to our SSDP probe
        // is whatever answers first) and was used directly as an allocation
        // length: `Content-Length: 500000000` meant a 500 MB `vec![]`, whose
        // allocation-failure path aborts the process under `panic = "abort"`.
        let remaining: usize = cl.parse().unwrap_or(0);
        if remaining > MAX_HTTP_BODY || body.len() + remaining > MAX_HTTP_BODY {
            return Err(PortMapError::Upnp(format!(
                "Content-Length {remaining} exceeds the {MAX_HTTP_BODY} byte limit"
            )));
        }
        let to_read = remaining.saturating_sub(body.len());
        let mut rest = vec![0u8; to_read];
        let mut read_total = 0;
        while read_total < to_read {
            let n = time::timeout(UPNP_READ_TIMEOUT, reader.read(&mut rest[read_total..]))
                .await
                .map_err(|_| PortMapError::Upnp("HTTP body read timed out".into()))?
                .map_err(PortMapError::Io)?;
            if n == 0 {
                break;
            }
            read_total += n;
        }
        body.extend_from_slice(&rest[..read_total]);
    } else {
        // No Content-Length and no chunked — read until connection close.
        let mut chunk = vec![0u8; 4096];
        loop {
            let n = time::timeout(Duration::from_secs(5), reader.read(&mut chunk))
                .await
                .map_err(|_| PortMapError::Upnp("HTTP body read timed out".into()))?
                .map_err(PortMapError::Io)?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
            if body.len() > MAX_HTTP_BODY {
                break;
            }
        }
    }

    Ok((status, body))
}

/// Extract the text content of an XML tag, ignoring whitespace, line breaks,
/// and namespace prefixes.
///
/// Searches for `<localName>` or `<ns:localName>…</...localName>` in `xml`.
/// Returns the trimmed content between the opening and closing tags.
fn extract_xml_tag(xml: &str, tag_name: &str) -> Option<String> {
    // Match opening tag: <tagName> or <ns:tagName> or <tagName > (with attributes).
    let _open_patterns = [
        format!("<{}>", tag_name),
        format!("<{} ", tag_name), // with attribute(s)
        format!("<{}:", tag_name), // wait, that's backward — ns:tag, not tag:ns
    ];
    // Actually, namespace prefix is prefix:tag, so we need <prefix:tagName>
    // Let me use a different approach: find </tagName> and work backwards.
    // Or: find any tag that ends with 'tagName' in the closing.

    // Simpler: look for <...tag_name followed by > or space or :>
    // This handles: <controlURL>, <u:controlURL>, <controlURL xmlns="...">
    let start_marker = format!("{}>", tag_name);
    let close_marker = format!("</{}>", tag_name);
    let also_close = format!("</{}:", tag_name); // Namespaced closing: </ns:tagName>

    // Find start by searching for "tag_name>"
    if let Some(start) = xml.find(&start_marker) {
        // Rewind to find the opening '<'
        let _open_begin = xml[..start].rfind('<')?;
        let content_start = start + start_marker.len();
        let remaining = &xml[content_start..];

        // Find closing tag.
        let close_pos = remaining
            .find(&close_marker)
            .or_else(|| remaining.find(&also_close))?;
        let content = remaining[..close_pos].trim();
        return Some(content.to_string());
    }

    None
}

/// Fetch and parse a UPnP device description XML to find the
/// WANIPConnection service's control URL.
async fn upnp_parse_description(location_url: &str) -> Result<String, PortMapError> {
    let sock_addr: SocketAddr = location_url.parse().or_else(|_| {
        let (host, port) = parse_url_host_port(location_url)?;
        format!("{}:{}", host, port)
            .parse()
            .map_err(|e| PortMapError::Upnp(format!("invalid socket address: {e}")))
    })?;

    let mut stream = crate::dial::dial_lan_only(sock_addr, Duration::from_secs(5))
        .await
        .map_err(pm_dial_err)?;

    // Determine the path from the URL for the GET request.
    let path = location_url
        .splitn(4, '/')
        .nth(3)
        .map(|p| format!("/{}", p))
        .unwrap_or_else(|| "/".to_string());

    let get_req = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Accept: text/xml\r\n\
         Connection: close\r\n\
         \r\n",
        path,
        extract_host(location_url).unwrap_or("localhost")
    );

    use tokio::io::AsyncWriteExt;
    stream.write_all(get_req.as_bytes()).await?;

    let (_status, body_bytes) = read_http_response_body(&mut stream).await?;
    let body = String::from_utf8_lossy(&body_bytes);

    // Find the WANIPConnection service and extract its controlURL.
    // Use the robust XML tag extractor which handles whitespace, multiline,
    // and namespace prefixes like <ns:controlURL>.
    extract_xml_tag(&body, "serviceType")
        .and_then(|t| {
            if t.contains("WANIPConnection") {
                Some(())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            PortMapError::Upnp("WANIPConnection service not found in device description".into())
        })?;

    let control_url = extract_xml_tag(&body, "controlURL").ok_or_else(|| {
        PortMapError::Upnp("controlURL not found in WANIPConnection service".into())
    })?;

    // Resolve relative URLs against the base URL.
    let resolved = if control_url.starts_with('/') {
        let base = location_url.trim_end_matches('/');
        if let Some(slash_pos) = base.rfind('/') {
            format!("{}{}", &base[..slash_pos], control_url)
        } else {
            format!("{}{}", base, control_url)
        }
    } else {
        control_url
    };

    // ── Validate hop 2 as strictly as hop 1 ──
    //
    // `LOCATION` is validated by `validate_upnp_location`, but the `controlURL`
    // *inside the fetched document* was returned verbatim and then used as the
    // POST target. Anything on the LAN can answer our SSDP probe, so a hostile
    // host could return a legitimate-looking
    // `LOCATION: http://192.168.1.1/desc.xml` whose document contains
    //
    //     <controlURL>http://169.254.169.254/latest/meta-data/</controlURL>
    //
    // and M2M would POST a SOAP body straight at the cloud metadata service.
    // The metadata deny-list in `validate_upnp_location` was bypassable, because
    // it only ever applied to the first hop.
    //
    // `dial_lan_only` does not help: it refuses only under Tor and performs no
    // locality check.
    validate_upnp_location(&resolved).map_err(|e| {
        PortMapError::Upnp(format!(
            "device description returned an unsafe controlURL ({e}) — refusing to POST to it"
        ))
    })
}

/// Add a TCP port mapping via UPnP IGD.
async fn upnp_map_tcp(
    internal_port: u16,
    _lifetime_secs: u32,
) -> Result<PortMapping, PortMapError> {
    let service = upnp_discover().await?;

    // Learn our internal client IP.
    let client_ip = crate::commands::util::resolve_local_ip()
        .ok_or_else(|| PortMapError::Upnp("cannot determine local IP".into()))?;

    // Build the SOAP AddPortMapping request.
    let body = SOAP_ADD_PORT
        .replace("{external_port}", &internal_port.to_string())
        .replace("{internal_port}", &internal_port.to_string())
        .replace("{internal_client}", &client_ip.to_string())
        .replace("{lease_duration}", &_lifetime_secs.to_string());

    // POST to control URL with SOAPAction header.
    let content_type = "text/xml; charset=\"utf-8\"";
    let soap_action = "\"urn:schemas-upnp-org:service:WANIPConnection:1#AddPortMapping\"";

    let http_req = format!(
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         SOAPAction: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        service.control_url,
        extract_host(&service.control_url).unwrap_or("localhost"),
        content_type,
        body.len(),
        soap_action,
        body
    );

    // Parse the host and port from the control URL.
    let (host, port) = parse_url_host_port(&service.control_url)?;
    let sock_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| PortMapError::Upnp(format!("invalid socket address: {e}")))?;

    let mut stream = crate::dial::dial_lan_only(sock_addr, Duration::from_secs(5))
        .await
        .map_err(pm_dial_err)?;

    use tokio::io::AsyncWriteExt;
    stream.write_all(http_req.as_bytes()).await?;

    // Read the full HTTP response using the robust reader.
    let (status_code, response_body) = read_http_response_body(&mut stream).await?;
    let resp_str = String::from_utf8_lossy(&response_body);

    if status_code == 200 {
        // Success — get the external IP from the gateway via UPnP
        // GetExternalIPAddress, or fall back to the local IP.
        let public_ip = gateway_wan_ip_via_upnp(&service).await.unwrap_or(client_ip);

        tracing::info!(
            internal = internal_port,
            external = internal_port,
            public = %public_ip,
            "UPnP port mapping established"
        );

        Ok(PortMapping {
            protocol: "upnp-igd",
            internal_port,
            external_addr: SocketAddr::new(public_ip, internal_port),
            lifetime_secs: _lifetime_secs,
        })
    } else if status_code == 500 && resp_str.contains("ConflictInMappingEntry") {
        Err(PortMapError::Upnp("port already mapped (conflict)".into()))
    } else if status_code == 500 {
        Err(PortMapError::Upnp(format!(
            "SOAP error: {}",
            truncate_safe(&resp_str, 200)
        )))
    } else {
        Err(PortMapError::Upnp(format!(
            "unexpected HTTP status {}: {}",
            status_code,
            truncate_safe(&resp_str, 100)
        )))
    }
}

/// Remove a UPnP TCP port mapping.
async fn upnp_remove_tcp(_internal_port: u16, external_port: u16) -> Result<(), PortMapError> {
    let service = upnp_discover().await?;

    let body = SOAP_DELETE_PORT.replace("{external_port}", &external_port.to_string());

    let content_type = "text/xml; charset=\"utf-8\"";
    let soap_action = "\"urn:schemas-upnp-org:service:WANIPConnection:1#DeletePortMapping\"";

    let http_req = format!(
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         SOAPAction: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        service.control_url,
        extract_host(&service.control_url).unwrap_or("localhost"),
        content_type,
        body.len(),
        soap_action,
        body
    );

    let (host, port) = parse_url_host_port(&service.control_url)?;
    let sock_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| PortMapError::Upnp(format!("invalid socket address: {e}")))?;

    let mut stream = crate::dial::dial_lan_only(sock_addr, Duration::from_secs(5))
        .await
        .map_err(pm_dial_err)?;

    use tokio::io::AsyncWriteExt;
    stream.write_all(http_req.as_bytes()).await?;

    let (status_code, _body) = read_http_response_body(&mut stream).await?;

    if status_code == 200 {
        Ok(())
    } else {
        // Non-fatal: the mapping will expire eventually.
        tracing::warn!(status = status_code, "UPnP remove returned non-200");
        Ok(())
    }
}

/// Get the WAN IP via UPnP `GetExternalIPAddress`.
async fn gateway_wan_ip_via_upnp(service: &UpnpService) -> Result<IpAddr, PortMapError> {
    let soap_body = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
            s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetExternalIPAddress xmlns:u="urn:schemas-upnp-org:service:WANIPConnection:1"/>
  </s:Body>
</s:Envelope>"#;

    let content_type = "text/xml; charset=\"utf-8\"";
    let soap_action = "\"urn:schemas-upnp-org:service:WANIPConnection:1#GetExternalIPAddress\"";

    let http_req = format!(
        "POST {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         SOAPAction: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        service.control_url,
        extract_host(&service.control_url).unwrap_or("localhost"),
        content_type,
        soap_body.len(),
        soap_action,
        soap_body
    );

    let (host, port) = parse_url_host_port(&service.control_url)?;
    let sock_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e| PortMapError::Upnp(format!("invalid socket address: {e}")))?;

    let mut stream = crate::dial::dial_lan_only(sock_addr, Duration::from_secs(5))
        .await
        .map_err(pm_dial_err)?;

    use tokio::io::AsyncWriteExt;
    stream.write_all(http_req.as_bytes()).await?;

    let (_status, body_bytes) = read_http_response_body(&mut stream).await?;
    let body = String::from_utf8_lossy(&body_bytes);

    if let Some(ip_str) = extract_xml_tag(&body, "NewExternalIPAddress") {
        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            return Ok(ip);
        }
    }

    Err(PortMapError::Upnp(
        "could not parse external IP from UPnP response".into(),
    ))
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Parse a URL like `http://192.168.1.1:5000/ctl/conn` into
/// `(host, port)`.
fn parse_url_host_port(url: &str) -> Result<(&str, u16), PortMapError> {
    // Strip http:// or https:// prefix.
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    // Split on first '/' to get host:port part.
    let host_port = rest.split('/').next().unwrap_or(rest);
    let host = host_port.split(':').next().unwrap_or(host_port);
    let port: u16 = host_port
        .split(':')
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(5000);

    Ok((host, port))
}

/// Extract just the host from a URL for the Host header.
fn extract_host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    rest.split('/').next()
}

/// Safely truncate a string for error messages.
/// Char-boundary safe: never panics on multi-byte UTF-8 input (H2).
fn truncate_safe(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod port_mapping_tests {
    use super::*;

    #[test]
    fn test_parse_url_host_port_simple() {
        let (host, port) = parse_url_host_port("http://192.168.1.1:5000/ctl/conn").unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 5000);
    }

    #[test]
    fn test_parse_url_host_port_default_port() {
        let (host, port) = parse_url_host_port("http://192.168.1.1/upnp").unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 5000);
    }

    #[test]
    fn test_parse_url_host_port_no_path() {
        let (host, port) = parse_url_host_port("192.168.1.1:49152").unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 49152);
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("http://192.168.1.1:5000/ctl/conn"),
            Some("192.168.1.1:5000")
        );
        assert_eq!(extract_host("http://192.168.1.1/upnp"), Some("192.168.1.1"));
    }

    #[test]
    fn test_port_mapping_debug() {
        let m = PortMapping {
            protocol: "nat-pmp",
            internal_port: 9000,
            external_addr: "1.2.3.4:54321".parse().unwrap(),
            lifetime_secs: 3600,
        };
        let d = format!("{:?}", m);
        assert!(d.contains("nat-pmp"));
        assert!(d.contains("9000"));
        assert!(d.contains("1.2.3.4:54321"));
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod upnp_security_tests {
    use super::*;

    /// A hostile LAN host can answer our SSDP probe with any `LOCATION` it
    /// likes. Following it verbatim turned M2M into a request forwarder — most
    /// sharply against the cloud metadata endpoint at 169.254.169.254, whose
    /// connection outcome is also a port-scan oracle.
    #[test]
    fn test_ssrf_blocks_cloud_metadata_endpoint() {
        let err = validate_upnp_location("http://169.254.169.254/latest/meta-data/")
            .expect_err("the cloud metadata endpoint must be refused");
        assert!(
            err.to_string().contains("metadata"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_ssrf_blocks_public_and_documentation_addresses() {
        for url in [
            "http://8.8.8.8/desc.xml",
            "http://1.1.1.1/",
            "http://203.0.113.10/rootDesc.xml",
            "http://198.51.100.7/desc.xml",
        ] {
            assert!(
                validate_upnp_location(url).is_err(),
                "{url} is not a gateway and must be refused"
            );
        }
    }

    /// A hostname is refused rather than resolved: DNS is attacker-controlled
    /// and could point anywhere.
    #[test]
    fn test_ssrf_blocks_hostnames() {
        for url in [
            "http://evil.example.com/desc.xml",
            "http://metadata.google.internal/computeMetadata/v1/",
            "http://localhost/desc.xml",
            "http://router.local/desc.xml",
        ] {
            assert!(
                validate_upnp_location(url).is_err(),
                "{url} must be refused (hostname, not a literal gateway IP)"
            );
        }
    }

    #[test]
    fn test_ssrf_blocks_https_and_parser_confusion() {
        for url in [
            "https://192.168.1.1/desc.xml", // TLS on a device with no identity story
            "ftp://192.168.1.1/desc.xml",   // wrong scheme
            "file:///etc/passwd",           // local file read
            "http://user@8.8.8.8/desc.xml", // userinfo confusion
            "http://192.168.1.1@8.8.8.8/",  // userinfo confusion
            "http://",                      // no host
            "",                             // empty
        ] {
            assert!(
                validate_upnp_location(url).is_err(),
                "{url:?} must be refused"
            );
        }
    }

    /// The guard must not break real IGDs — they are always on the local
    /// network, which is the whole point of UPnP.
    #[test]
    fn test_ssrf_allows_real_gateways() {
        for url in [
            "http://192.168.1.1:5000/rootDesc.xml",
            "http://10.0.0.1/rootDesc.xml",
            "http://172.16.0.1:80/upnp/desc.xml",
            "http://127.0.0.1:49000/rootDesc.xml",
            "http://[fe80::1]:80/desc.xml",
            "http://[fd00::1]/desc.xml",
        ] {
            assert!(
                validate_upnp_location(url).is_ok(),
                "{url} is a legitimate gateway and must be allowed"
            );
        }
    }

    #[test]
    fn test_ssrf_rejects_overlong_location() {
        let long = format!("http://192.168.1.1/{}", "a".repeat(MAX_UPNP_LOCATION_LEN));
        assert!(validate_upnp_location(&long).is_err());
    }

    /// Every declared length in a UPnP HTTP response is chosen by whatever
    /// answered the probe. They must never reach `vec![]` unbounded, because an
    /// allocation failure aborts the process under `panic = "abort"`.
    #[tokio::test]
    async fn test_http_body_bounds_are_enforced() {
        // (a) A hostile Content-Length.
        let mut evil = Vec::new();
        evil.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 500000000\r\n\r\n");
        evil.extend_from_slice(b"x");
        let mut rd = &evil[..];
        let err = read_http_response_body(&mut rd)
            .await
            .expect_err("an oversized Content-Length must be refused");
        assert!(err.to_string().contains("exceeds"), "unexpected: {err}");

        // (b) A hostile chunk size in a chunked response.
        //
        // Driven over a duplex stream with the headers and the chunk header
        // written separately, because the reader's header loop reads up to
        // 4096 bytes at a time: if the chunk header arrives in the same read it
        // is already part of the buffered body and the chunk-size path is never
        // reached. The streaming case is the one that matters.
        let (mut wr, mut rd2) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let _ = wr
                .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                .await;
            let _ = wr.flush().await;
            // Give the reader a moment to finish the header pass, then send an
            // absurd chunk size.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let _ = wr.write_all(b"100000000\r\n").await; // 268 MB
            let _ = wr.flush().await;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), wr.write_all(b"AAAA"))
                .await;
        });
        let err2 = read_http_response_body(&mut rd2)
            .await
            .expect_err("an oversized chunk size must be refused");
        assert!(err2.to_string().contains("exceeds"), "unexpected: {err2}");

        // (c) A legitimate body still works, so the guard is not over-tight.
        let mut good = Vec::new();
        good.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
        let mut rd3 = &good[..];
        let (status, body) = read_http_response_body(&mut rd3).await.unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, b"hello");
    }

    /// The bound is a compile-time invariant, so a bad edit fails the build
    /// rather than silently shipping a limit that is too small (breaking real
    /// gateways) or too large (restoring the amplification it exists to stop).
    const _: () = assert!(MAX_HTTP_BODY > 0);
    const _: () = assert!(MAX_HTTP_BODY <= 1024 * 1024);
}
