# M2M — Protocol Specification

> **Version**: 0.1.0 (Protocol Version 1)  
> **Status**: Draft  
> **Last Updated**: 2026-05-28

## 1. Protocol Versioning

Every M2M packet begins with a protocol version byte.

- `0x01` = Protocol Version 1 (this document)
- `0x00`, `0xFE`, `0xFF` are reserved
- No fallback to older versions (prevents downgrade attacks)

## 2. Transport Framing

Length-prefixed framing over TCP:

```
┌────────────┬──────────┬──────────────────┐
│ Length (4B) │ Ver (1B) │ Payload (var)    │
│ u32 BE     │ u8       │                  │
└────────────┴──────────┴──────────────────┘
```

- **Length**: size of Version + Payload (big-endian u32), excludes itself
- **Max frame**: 16 MiB | **Max text msg**: 64 KiB | **Max file chunk**: 256 KiB
- **Max handshake msg**: 4 KiB | **Min frame**: 2 bytes

## 3. Packet Types

| Type | Name | Phase |
|------|------|-------|
| 0x01 | HandshakeInit | Handshake |
| 0x02 | HandshakeResponse | Handshake |
| 0x03 | HandshakeComplete | Handshake |
| 0x10 | EncryptedMessage | Established |
| 0x11 | FileTransferRequest | Established |
| 0x12 | FileTransferChunk | Established |
| 0x13 | FileTransferComplete | Established |
| 0x14 | FileTransferAccept | Established |
| 0x15 | FileTransferReject | Established |
| 0x20 | Heartbeat | Any |
| 0x21 | HeartbeatAck | Any |
| 0x30 | Disconnect | Any |
| 0x31 | Error | Any |

Unknown types → Error packet + close connection.

## 4. Handshake Protocol

X25519 DH authenticated by Ed25519 identity keys:

1. **Init**: initiator sends ephemeral_pub + identity_pub + timestamp + signature
2. **Response**: responder sends same fields
3. **Key derivation**: `shared = X25519(my_eph, peer_eph)`, `session_key = HKDF(shared, context)`
4. **Complete**: initiator sends encrypted verification proof

## 5. Encrypted Message Format

```
nonce (24B) | counter (8B, u64 BE) | ciphertext (XChaCha20-Poly1305)
```

AAD = packet_type || counter. Counter must be monotonically increasing.

## 6. Heartbeat: every 30s, timeout 10s, 1 retry.

## 7. Disconnect: reason enum, then close TCP.

## 8. Error Codes

| Code | Name |
|------|------|
| 0x0001 | UnknownPacketType |
| 0x0002 | FrameTooLarge |
| 0x0003 | HandshakeFailed |
| 0x0004 | DecryptionFailed |
| 0x0005 | InvalidSequence |
| 0x0006 | SessionExpired |
| 0x0007 | RateLimitExceeded |
| 0x0008 | VersionMismatch |
| 0x0009 | InternalError |
| 0x000A | InvalidSignature |
