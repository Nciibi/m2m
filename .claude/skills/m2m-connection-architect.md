---
name: m2m-connection-architect
description: >
  Complete reference for M2M's connection strategy architecture, NAT traversal
  implementations, module boundaries, and design decisions. Use this when
  working on any networking, connection-management, or NAT-traversal feature.
---

# M2M Connection Architect Skill

You are building a **zero-trust P2P encrypted messenger** in Rust. The
connection layer uses a **Happy Eyeballs parallel race** across multiple
NAT-traversal strategies. This skill documents the architecture, the
trade-offs, and the common pitfalls.

## Core architecture

The Connection Manager (`hole_punch.rs`) races all strategies concurrently:

```
JoinSet::spawn(strategy_1)  // DirectTcp (host)
JoinSet::spawn(strategy_2)  // Ipv6Direct
JoinSet::spawn(strategy_3)  // PortMapped (UPnP/NAT-PMP/PCP / manual forward)
JoinSet::spawn(strategy_4)  // TcpHolePunch (srflx candidates)

join_next() → first success wins → shutdown() cancels all others
```

**Never use sequential phases.** Sequential phases sum timeouts (~30s).
Parallel racing determines the winner by network latency, not list
position — same pattern as RFC 8305 Happy Eyeballs.

## Module boundary rule

Each module owns exactly **one mechanism**:

| Module | Owns |
|--------|------|
| `local_addr.rs` | Local interface discovery via UDP probes |
| `stun.rs` | Pure RFC 8489 STUN only |
| `port_mapping.rs` | PCP → NAT-PMP → UPnP IGD behind `PortMapper` facade |
| `candidate.rs` | ICE candidate types (Host=0, Srflx=1, Prflx=2, Relay=3, PortMapped=4, Ipv6=5), RFC 8445 priority, gathering orchestrator |
| `hole_punch.rs` | Connection Manager, Happy Eyeballs race, TCP hole punch |

Do NOT leak STUN protocol logic into candidate gathering, or vice versa.

## TCP hole punch pattern

```rust
tokio::select! {
    stream = listener.accept()  => /* peer connected to us → Responder */
    stream = connect(candidates) => /* we connected to peer → Initiator */
}
```

Create a shadow listener with `SO_REUSEADDR` on the main listener's port.
Both sides run this race simultaneously. Works for restricted-cone NATs.

## Port mapping protocol order

Try **PCP → NAT-PMP → UPnP IGD**. First success wins, log failures as debug:

- PCP: 50-byte binary packet, external port at offset 32-33
- NAT-PMP: 12-byte request, external port at offset **10-11** (NOT 8-9)
- UPnP IGD: SSDP → LOCATION → XML description → SOAP AddPortMapping

## Common gotchas

- **PCP request is 50 bytes** (24 header + 26 body), not 36.
- **NAT-PMP external port** is at bytes 10-11, not 8-9.
- **Gateway IP ≠ WAN IP.** Use NAT-PMP public-address request or system routing table (`/proc/net/route` on Linux, `route -n get default` on macOS, `route print 0.0.0.0` on Windows).
- **UDP prelude doesn't help TCP.** TCP and UDP NAT mappings are independent.
- **`JoinSet::join_next()` returns `Option<Result<T, JoinError>>`.** Handle both layers.
- **`time::timeout` with `TcpStream::connect`** needs pre-parsed `SocketAddr` to help type inference.
- **`IpAddr` lacks `is_link_local()`** — write a helper: `v6.octets()[0] == 0xfe && (v6.octets()[1] & 0xc0) == 0x80`.

## Connection lifecycle

```
Invite: STUN → port mapping → host/IPv6 candidates → manual forwards
        → merge, dedup → sign invite

Connect: extract candidates → build Strategy list → JoinSet::spawn all
         → race → first wins → shutdown() rest → handshake → session

Disconnect: read fails → remove from state → must re-invite
```
