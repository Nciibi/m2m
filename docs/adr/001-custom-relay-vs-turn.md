# ADR 001: Custom TCP Relay Instead of Full TURN (RFC 5766)

**Status**: Accepted  
**Date**: 2026-06-28  

## Context

M2M needed a relay fallback for symmetric NAT scenarios where direct TCP hole punching fails. The options were: implement full TURN (RFC 5766), or build a lightweight custom TCP relay protocol.

## Decision

Build a **custom lightweight TCP relay** instead of implementing full TURN.

## Rationale

- **TCP-only**: M2M uses TCP transport exclusively. Full TURN is UDP-oriented and requires HMAC-SHA1 for authentication, adding crypto dependencies M2M doesn't otherwise need.
- **Forward secrecy by construction**: The relay never sees plaintext — M2M's XChaCha20-Poly1305 runs on top, so the relay is a dumb byte pipe.
- **Simplicity**: The custom protocol has 4 message types (REGISTER, CONNECT, KEEPALIVE, ERROR/REGISTERED/CONNECTED/PONG) vs. TURN's Allocate/Refresh/Send/ChannelData lifecycle.
- **No new crypto**: Uses length-prefixed framing (same as M2M's wire protocol) and the relay server has no crypto dependencies.

## Consequences

- Incompatible with standard TURN servers (must run the custom relay-server example or implement one).
- Simpler codebase (relay.rs is ~570 lines vs. a full TURN stack which would be 2000+).
- Users who want to run their own relay can deploy the provided `examples/relay-server.rs`.
