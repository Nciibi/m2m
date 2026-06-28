# ADR 003: MessagePack Instead of Protocol Buffers

**Status**: Accepted  
**Date**: 2026-06-28  

## Context

M2M needs a binary serialization format for its wire protocol. Options: MessagePack (via rmp-serde), Protocol Buffers (via prost), or flatbuffers.

## Decision

Use **MessagePack** via `rmp-serde`.

## Rationale

- **Zero schema compilation**: MessagePack works directly with Rust `#[derive(Serialize, Deserialize)]` from serde. No `.proto` files, no code generation step, no build.rs complexity.
- **Compact**: Binary format is typically 60-70% of JSON size for M2M's message shapes.
- **Self-describing**: Unlike protobuf, MessagePack messages can be decoded without the schema. This aids debugging and makes backward-compatible field additions trivial (`#[serde(default)]`).
- **Backward compat**: M2M's protocol v2 adds X3DH fields with `#[serde(default)]` — old peers that don't know about them simply see defaults. No schema migration needed.
- **Lighter dependency**: `rmp-serde` is a pure Rust crate with no C compilation.

## Consequences

- Slightly larger wire size than protobuf for the same data (MessagePack encodes field names, protobuf uses field numbers). Not a concern — M2M already runs on TCP with 16 MiB max frame size.
- No built-in schema evolution documentation (no .proto files to read). M2M compensates with thorough protocol documentation in `docs/protocol-spec.md`.
