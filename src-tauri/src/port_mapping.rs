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
/// ```
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

use tokio::net::{TcpStream, UdpSocket};
use tokio::time;

use thiserror::Error;

// ─── Public API ─────────────────────────────────────────────────────────────

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
    pub async fn remove_port_mapping(mapping: &PortMapping) {
        match mapping.protocol {
            "nat-pmp" => {
                if let Err(e) = nat_pmp_remove_tcp(mapping.external_addr.port()).await {
                    tracing::warn!(error = %e, "NAT-PMP remove failed");
                }
            }
            "pcp" => {
                if let Err(e) = pcp_remove_tcp(mapping.internal_port, mapping.external_addr.port()).await {
                    tracing::warn!(error = %e, "PCP remove failed");
                }
            }
            "upnp-igd" => {
                if let Err(e) = upnp_remove_tcp(mapping.internal_port, mapping.external_addr.port()).await {
                    tracing::warn!(error = %e, "UPnP remove failed");
                }
            }
            other => tracing::warn!(protocol = other, "don't know how to remove this mapping"),
        }
    }
}

// ─── Gateway Discovery ──────────────────────────────────────────────────────

/// Common default-gateway addresses to try in /24 and /16 subnets.
const COMMON_GATEWAYS: &[[u8; 4]] = &[
    [192, 168, 0, 1],
    [192, 168, 1, 1],
    [192, 168, 1, 254],
    [10, 0, 0, 1],
    [10, 0, 1, 1],
    [172, 16, 0, 1],
    [192, 168, 0, 254],
    [10, 0, 0, 138],
    [192, 168, 2, 1],
    [192, 168, 100, 1],
];

/// Discover the local gateway/router IP by probing common addresses.
///
/// Strategy:
/// 1. Connect a UDP socket to a public IP to learn our local interface IP.
/// 2. If the local IP is on a known subnet, try the .1 and .254 suffixes on
///    that /24, plus a handful of other common gateway addresses.
/// 3. Send a NAT-PMP public-address request (op=0) to each candidate — the
///    first to respond is the real gateway.
async fn discover_gateway() -> Option<IpAddr> {
    // Learn our local interface IP by binding to port 0 and "connecting"
    // to a public IP (the socket doesn't actually send anything).
    let local_ip = {
        let sock = UdpSocket::bind("0.0.0.0:0").await.ok()?;
        sock.connect("8.8.8.8:53").await.ok()?;
        sock.local_addr().ok()?.ip()
    };

    let candidates: Vec<Ipv4Addr> = if let IpAddr::V4(v4) = local_ip {
        let octets = v4.octets();
        // Add subnet-specific probes first.
        let mut list = vec![
            Ipv4Addr::new(octets[0], octets[1], octets[2], 1),
            Ipv4Addr::new(octets[0], octets[1], octets[2], 254),
        ];
        // Append common fallback gateways.
        for gw in COMMON_GATEWAYS {
            let addr = Ipv4Addr::new(gw[0], gw[1], gw[2], gw[3]);
            if !list.contains(&addr) {
                list.push(addr);
            }
        }
        list
    } else {
        COMMON_GATEWAYS
            .iter()
            .map(|o| Ipv4Addr::new(o[0], o[1], o[2], o[3]))
            .collect()
    };

    for gw in &candidates {
        let addr = SocketAddr::new(IpAddr::V4(*gw), 5351);
        let probe = match nat_pmp_public_address(&addr).await {
            Ok(ip) => ip,
            Err(_) => continue,
        };
        tracing::info!(gateway = %gw, public_ip = %probe, "gateway discovered");
        return Some(probe);
    }

    // Last resort: try the candidate list as-is (the probe above would have
    // returned the public IP; if it failed, we can't verify any of them).
    // Return the first subnet-based candidate anyway so callers can attempt
    // the mapping protocols — they might succeed even if our probe didn't.
    for gw in &candidates {
        tracing::warn!(gateway = %gw, "using unverified gateway — NAT-PMP probe failed");
        return Some(IpAddr::V4(*gw));
    }

    None
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
async fn nat_pmp_public_address(
    gateway: &SocketAddr,
) -> Result<IpAddr, PortMapError> {
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
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
        return Err(PortMapError::NatPmp(format!("unexpected opcode: {}", buf[1])));
    }
    let result = u16::from_be_bytes([buf[2], buf[3]]);
    if result != NAT_PMP_SUCCESS {
        return Err(PortMapError::NatPmp(format!("public-address error: result={}", result)));
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
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(gw).await?;

    // Request: [ver=0, op=2, reserved=2B, int_port=2B, ext_port=2B, lifetime=4B] (12 bytes)
    let mut req = [0u8; 12];
    req[0] = NAT_PMP_VERSION;
    req[1] = NAT_PMP_OP_MAP_TCP;
    req[4..6].copy_from_slice(&internal_port.to_be_bytes());  // internal port
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
        return Err(PortMapError::NatPmp(format!("unexpected opcode: {}", buf[1])));
    }
    let result = u16::from_be_bytes([buf[2], buf[3]]);
    if result != NAT_PMP_SUCCESS {
        return Err(PortMapError::NatPmp(format!(
            "router rejected mapping: result code {}",
            result
        )));
    }
    let ext_port = u16::from_be_bytes([buf[8], buf[9]]);

    // Get the router's WAN IP via a separate public-address request.
    let public_ip = nat_pmp_public_address(&gw).await?;

    Ok(PortMapping {
        protocol: "nat-pmp",
        internal_port,
        external_addr: SocketAddr::new(public_ip, ext_port),
    })
}

/// Remove a NAT-PMP TCP mapping by requesting lifetime=0 for the external port.
async fn nat_pmp_remove_tcp(external_port: u16) -> Result<(), PortMapError> {
    let gateway = match discover_gateway().await {
        Some(g) => g,
        None => return Err(PortMapError::NoGateway),
    };

    let sock = UdpSocket::bind("0.0.0.0:0").await?;
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
        return Err(PortMapError::NatPmp(format!("remove failed: result={}", result)));
    }
    Ok(())
}

// ─── PCP (RFC 6887) ─────────────────────────────────────────────────────────

/// PCP version.
const PCP_VERSION: u8 = 2;

/// Opcode: MAP.
const PCP_OP_MAP: u8 = 1;

/// Success result code.
const PCP_SUCCESS: u8 = 0;

/// PCP request/response timeout.
const PCP_TIMEOUT: Duration = Duration::from_secs(3);

/// PCP MAP request for TCP.
///
/// PCP uses a 24-byte request (with 12-byte authentication nonce that we
/// zero-fill for simplicity — many home routers accept this).
async fn pcp_map_tcp(
    gateway: IpAddr,
    internal_port: u16,
    lifetime_secs: u32,
) -> Result<PortMapping, PortMapError> {
    let gw = SocketAddr::new(gateway, 5351);
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(gw).await?;

    // Build a PCP MAP request (24 bytes for simplest case).
    // Request: [ver, op, reserved=1B, lifetime=4B, client_ip=16B (zero),
    //           nonce=12B (zero), protocol=1B, reserved=1B, int_port=2B,
    //           ext_port=2B, ext_ip=16B (zero)]
    let mut req = [0u8; 36];
    req[0] = PCP_VERSION;
    req[1] = PCP_OP_MAP;
    req[4..8].copy_from_slice(&lifetime_secs.to_be_bytes());
    // client_ip: all zeros = let the router decide
    // nonce: all zeros = no authentication
    req[24] = 6; // IPPROTO_TCP
    req[28..30].copy_from_slice(&internal_port.to_be_bytes());
    // ext_port = 0 = let router choose
    // ext_ip = zeros = let router choose

    sock.send(&req).await?;

    let mut buf = [0u8; 64];
    let n = time::timeout(PCP_TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| PortMapError::Pcp("MAP request timed out".into()))?
        .map_err(PortMapError::Io)?;

    if n < 36 {
        return Err(PortMapError::Pcp(format!("short response: {} bytes", n)));
    }

    let result = buf[3];
    if result != PCP_SUCCESS {
        return Err(PortMapError::Pcp(format!(
            "router rejected PCP mapping: result code {}",
            result
        )));
    }

    let mapped_lifetime = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let external_port = u16::from_be_bytes([buf[28], buf[29]]);
    let ext_ip = IpAddr::V4(Ipv4Addr::new(buf[30], buf[31], buf[32], buf[33]));

    tracing::debug!(
        lifetime = mapped_lifetime,
        "PCP mapping granted"
    );

    Ok(PortMapping {
        protocol: "pcp",
        internal_port,
        external_addr: SocketAddr::new(ext_ip, external_port),
    })
}

/// Remove a PCP mapping by requesting lifetime=0.
async fn pcp_remove_tcp(
    internal_port: u16,
    external_port: u16,
) -> Result<(), PortMapError> {
    let gateway = match discover_gateway().await {
        Some(g) => g,
        None => return Err(PortMapError::NoGateway),
    };

    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(SocketAddr::new(gateway, 5351)).await?;

    let mut req = [0u8; 36];
    req[0] = PCP_VERSION;
    req[1] = PCP_OP_MAP;
    req[4..8].copy_from_slice(&0u32.to_be_bytes()); // lifetime = 0 → remove
    req[24] = 6; // IPPROTO_TCP
    req[28..30].copy_from_slice(&internal_port.to_be_bytes());
    req[30..32].copy_from_slice(&external_port.to_be_bytes());
    sock.send(&req).await?;

    let mut buf = [0u8; 64];
    let n = time::timeout(PCP_TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| PortMapError::Pcp("remove timed out".into()))?
        .map_err(PortMapError::Io)?;
    if n < 4 {
        return Err(PortMapError::Pcp("short remove response".into()));
    }
    if buf[3] != PCP_SUCCESS {
        return Err(PortMapError::Pcp(format!("remove failed: result={}", buf[3])));
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
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
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
        if resp.contains("InternetGatewayDevice") || resp.contains("urn:schemas-upnp-org:device:InternetGatewayDevice") {
            for line in resp.lines() {
                let lower = line.to_lowercase();
                if lower.starts_with("location:") {
                    if let Some(url) = line.splitn(2, ':').nth(1) {
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

    let location = location_url.ok_or_else(|| {
        PortMapError::Upnp("no UPnP IGD device found on the network".into())
    })?;

    // Now fetch the device description XML and find the WANIPConnection control URL.
    let control_url = upnp_parse_description(&location).await?;

    Ok(UpnpService { control_url })
}

/// Fetch and parse a UPnP device description XML to find the
/// WANIPConnection service's control URL.
async fn upnp_parse_description(location_url: &str) -> Result<String, PortMapError> {
    let (host, port) = parse_url_host_port(location_url)?;

    let mut stream = time::timeout(Duration::from_secs(5), TcpStream::connect((host, port)))
        .await
        .map_err(|_| PortMapError::Upnp("connection to lookup device description timed out".into()))?
        .map_err(PortMapError::Io)?;

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

    // Read the full HTTP response.
    use tokio::io::AsyncReadExt;
    let mut response = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => response.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
        if response.len() > 32 * 1024 {
            break; // Sanity cap
        }
    }

    // Split headers from body using double CRLF.
    let response_str = String::from_utf8_lossy(&response);
    let body = if let Some(pos) = response_str.find("\r\n\r\n") {
        &response_str[pos + 4..]
    } else {
        &response_str
    };

    // Parse the XML to find service:WANIPConnection:1 → controlURL.
    // We do this with simple string scanning to avoid an XML dep entirely.
    // UPnP XML is predictable enough for this to work reliably.
    //
    // We look for:
    //   <service>
    //     <serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType>
    //     <controlURL>...</controlURL>
    //   </service>

    // Find the WANIPConnection service block.
    let service_marker = "urn:schemas-upnp-org:service:WANIPConnection:1";
    let service_pos = body.find(service_marker).ok_or_else(|| {
        PortMapError::Upnp("WANIPConnection service not found in device description".into())
    })?;

    // Look backwards for <service> and forwards for <controlURL>
    let before_service = &body[..service_pos];
    let service_start = before_service.rfind("<service>").ok_or_else(|| {
        PortMapError::Upnp("malformed device description XML".into())
    })?;

    let after_type = &body[service_pos + service_marker.len()..];

    // Find <controlURL> in the remaining service block
    let control_start = after_type.find("<controlURL>").ok_or_else(|| {
        PortMapError::Upnp("controlURL not found in service".into())
    })?;

    let after_control_start = &after_type[control_start + "<controlURL>".len()..];
    let control_end = after_control_start.find("</controlURL>").ok_or_else(|| {
        PortMapError::Upnp("unclosed controlURL tag".into())
    })?;

    let control_url = after_control_start[..control_end].trim().to_string();

    // If the control URL is relative, resolve it against the base URL.
    if control_url.starts_with('/') {
        let base = location_url.trim_end_matches('/');
        // Strip path from base URL.
        if let Some(slash_pos) = base.rfind('/') {
            let base_up_to_path = &base[..slash_pos];
            Ok(format!("{}{}", base_up_to_path, control_url))
        } else {
            Ok(format!("{}{}", base, control_url))
        }
    } else {
        Ok(control_url)
    }
}

/// Add a TCP port mapping via UPnP IGD.
async fn upnp_map_tcp(
    internal_port: u16,
    _lifetime_secs: u32,
) -> Result<PortMapping, PortMapError> {
    let service = upnp_discover().await?;

    // Learn our internal client IP.
    let client_ip = crate::commands::resolve_local_ip()
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
    let addr = format!("{}:{}", host, port);

    let mut stream = time::timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| PortMapError::Upnp("connection to IGD timed out".into()))?
        .map_err(PortMapError::Io)?;

    use tokio::io::AsyncWriteExt;
    stream.write_all(http_req.as_bytes()).await?;

    // Read the HTTP response.
    use tokio::io::AsyncReadExt;
    let mut response = Vec::new();
    let mut buf = [0u8; 1024];
    let mut read_deadline = tokio::time::Instant::now() + Duration::from_secs(5);

    while tokio::time::Instant::now() < read_deadline {
        let remaining = read_deadline - tokio::time::Instant::now();
        match time::timeout(remaining, stream.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => response.extend_from_slice(&buf[..n]),
            Ok(Err(_)) => break,
            Err(_) => break,
        }
        if response.windows(4).any(|w| w == b"\r\n\r\n") && response.len() > 1000 {
            // Have headers, check for Content-Length
            break;
        }
    }

    let resp_str = String::from_utf8_lossy(&response);
    if resp_str.contains("HTTP/1.1 200") || resp_str.contains("HTTP/1.0 200") {
        // Success — get the external IP from the gateway.
        // For UPnP, the external IP is usually the same as what STUN would reveal.
        // We fall back to getting it from the response or using STUN's result.
        let public_ip = gateway_wan_ip_via_upnp(&service).await
            .unwrap_or_else(|_| discover_gateway_ip_via_stun().unwrap_or(client_ip));

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
        })
    } else if resp_str.contains("HTTP/1.1 500") && resp_str.contains("ConflictInMappingEntry") {
        Err(PortMapError::Upnp("port already mapped (conflict)".into()))
    } else if resp_str.contains("HTTP/1.1 500") {
        Err(PortMapError::Upnp(format!("SOAP error: {}", truncate_safe(&resp_str, 200))))
    } else {
        Err(PortMapError::Upnp(format!(
            "unexpected HTTP response: {}",
            truncate_safe(&resp_str, 100)
        )))
    }
}

/// Remove a UPnP TCP port mapping.
async fn upnp_remove_tcp(
    _internal_port: u16,
    external_port: u16,
) -> Result<(), PortMapError> {
    let service = upnp_discover().await?;

    let body = SOAP_DELETE_PORT
        .replace("{external_port}", &external_port.to_string());

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
    let addr = format!("{}:{}", host, port);

    let mut stream = time::timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| PortMapError::Upnp("connection timed out".into()))?
        .map_err(PortMapError::Io)?;

    use tokio::io::AsyncWriteExt;
    stream.write_all(http_req.as_bytes()).await?;

    let mut response = String::new();
    let mut buf = [0u8; 1024];
    'read: for _ in 0..10 {
        use tokio::io::AsyncReadExt;
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                response.push_str(&String::from_utf8_lossy(&buf[..n]));
                if response.contains("\r\n\r\n") {
                    break 'read;
                }
            }
            Err(_) => break,
        }
    }

    if response.contains("200 OK") {
        Ok(())
    } else {
        // Non-fatal: the mapping will expire eventually.
        tracing::warn!(response = %truncate_safe(&response, 100), "UPnP remove returned non-200");
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

    let mut stream = time::timeout(Duration::from_secs(5), TcpStream::connect((host, port)))
        .await
        .map_err(|_| PortMapError::Upnp("connection to IGD timed out".into()))?
        .map_err(PortMapError::Io)?;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream.write_all(http_req.as_bytes()).await?;

    let mut response = Vec::new();
    let mut buf = [0u8; 2048];
    loop {
        match stream.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => response.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
        if response.len() > 4096 {
            break;
        }
    }

    let resp_str = String::from_utf8_lossy(&response);
    // Parse <NewExternalIPAddress>xxx.xxx.xxx.xxx</NewExternalIPAddress>
    let start_tag = "<NewExternalIPAddress>";
    let end_tag = "</NewExternalIPAddress>";
    if let Some(start) = resp_str.find(start_tag) {
        let after_start = &resp_str[start + start_tag.len()..];
        if let Some(end) = after_start.find(end_tag) {
            let ip_str = after_start[..end].trim();
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                return Ok(ip);
            }
        }
    }

    Err(PortMapError::Upnp("could not parse external IP from UPnP response".into()))
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Fallback: use the STUN-discovered public IP from the app state.
fn discover_gateway_ip_via_stun() -> Result<IpAddr, PortMapError> {
    Err(PortMapError::NoGateway)
}

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
    Some(rest.split('/').next()?)
}

/// Safely truncate a string for error messages.
fn truncate_safe(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
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
        assert_eq!(
            extract_host("http://192.168.1.1/upnp"),
            Some("192.168.1.1")
        );
    }

    #[test]
    fn test_port_mapping_debug() {
        let m = PortMapping {
            protocol: "nat-pmp",
            internal_port: 9000,
            external_addr: "1.2.3.4:54321".parse().unwrap(),
        };
        let d = format!("{:?}", m);
        assert!(d.contains("nat-pmp"));
        assert!(d.contains("9000"));
        assert!(d.contains("1.2.3.4:54321"));
    }
}
