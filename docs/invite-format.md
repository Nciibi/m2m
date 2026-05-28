# M2M — Invite Link Format

> **Version**: 0.1.0 | **Status**: Draft | **Last Updated**: 2026-05-28

## 1. Purpose

An invite link allows one user to share their connection details with another.
It must be:
- **Signed** (tamper-evident via Ed25519)
- **Expiring** (short-lived, prevents stale invites)
- **Minimal** (contains only what's needed to connect)
- **Safe** (no private keys or secrets)
- **Shareable** (base64url-encoded string, copy-pasteable)

## 2. Invite Data Structure

```
InvitePayload {
    version:       u8,           // Protocol version (0x01)
    identity_pub:  [u8; 32],     // Ed25519 public key of inviter
    address_hint:  String,       // IP:port or hostname:port (max 256 chars)
    created_at:    u64,          // Unix timestamp (seconds)
    expires_at:    u64,          // Unix timestamp (seconds)
    nonce:         [u8; 16],     // Random nonce for uniqueness
    flags:         u8,           // Bit flags (one-time use, etc.)
}
```

### 2.1 Flags

| Bit | Name | Description |
|-----|------|-------------|
| 0 | ONE_TIME | Invite can be used only once |
| 1 | LISTENER | Inviter is the TCP listener (peer should connect) |
| 2-7 | Reserved | Must be 0 |

## 3. Signed Invite

```
SignedInvite {
    payload:   InvitePayload,        // The data above
    signature: [u8; 64],             // Ed25519 signature over serialized payload
}
```

Signature is computed over the MessagePack-serialized payload bytes.

## 4. Serialized Format

The final invite string is:

```
m2m://<base64url(msgpack(SignedInvite))>
```

- Prefix `m2m://` identifies the protocol.
- Base64url encoding (RFC 4648, no padding) for safe copy-paste.
- Total max length: 512 characters.

## 5. Validation Rules

1. Decode base64url. Reject if invalid encoding.
2. Deserialize MessagePack. Reject if malformed.
3. Check `version` field. Reject if unsupported.
4. Check `expires_at > now()`. Reject if expired.
5. Check `created_at <= now() + 5min`. Reject if created in the far future (clock skew tolerance).
6. Check `expires_at - created_at <= 24h`. Reject if expiry window too large.
7. Verify Ed25519 signature over serialized payload. Reject if invalid.
8. Validate `address_hint` format. Reject if malformed.
9. If ONE_TIME flag set, check if invite nonce was already consumed. Reject if reused.

## 6. Security Properties

- **Tamper-evidence**: Any modification invalidates the Ed25519 signature.
- **Expiry**: Time-bounded to prevent indefinite invite accumulation.
- **No secrets**: Contains only public key and connection hint.
- **One-time option**: Prevents invite reuse after first successful connection.
- **No replay**: Random nonce ensures each invite is unique.

## 7. Example

```
m2m://pGd2ZXJzaW9uAWtp...base64url...
```

The user copies this string and shares it via a secure out-of-band channel
(e.g., Signal, in-person, encrypted email).
