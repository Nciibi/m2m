//! M2M — the single error type crossing the IPC boundary
//!
//! ## Why this exists
//!
//! The crate defines 14 `thiserror` enums — `CryptoError`, `SessionError`,
//! `NetworkError`, `ProtocolError`, `StorageError`, `StunError`,
//! `PortMapError`, `HolePunchError`, `DhtError`, `RelayError`, `TorError`,
//! `IdentityError`, `GroupError`, `LanDiscoveryError` — and wires them together
//! with `#[from]` chains so a failure keeps its identity all the way down. The
//! quality of that taxonomy is the best thing about the backend's error
//! handling.
//!
//! Then the command layer destroyed it. All 109 `#[tauri::command]` functions
//! returned `Result<T, String>`, and 309 `map_err(|e| format!("...: {e}"))` calls
//! flattened every chain into prose on the last step. The frontend received
//! `"network error: crypto error: ..."` or `"send failed"` and could not tell a
//! replayed frame from a disk-full from a validation failure. Error *kinds*
//! were lost, so no caller could make a principled retry decision, and the only
//! structured code in the whole system was a bare string sentinel
//! (`"CANNOT_REACH"`, in `connect_family_member`) that the frontend had to
//! string-match.
//!
//! This type is the fix, and it is deliberately thin: a stable machine-readable
//! `code` plus the human-readable `message` the user should see. The 14 enums
//! stay where they are and keep their own variants; each gets a `From` impl that
//! maps it to a code, so nothing above the boundary has to know the taxonomy to
//! be useful.
//!
//! The `code` is a `&'static str` rather than an enum so adding a code does not
//! change the wire format, and so the frontend can exhaustively switch on a
//! string union without a generated binding. Codes are namespaced by
//! subsystem (`crypto.*`, `network.*`, …) so a caller can act on a family
//! without enumerating every variant.
//!
//! ## What is deliberately lost
//!
//! The full variant chain. `"network error: crypto error: key derivation
//! failed"` is more informative for a bug report than `crypto.key_derivation`,
//! and `AppError` keeps the whole rendered chain in `message` — so nothing is
//! actually lost, it just stops being the *only* thing available. The `code`
//! is what code branches on; the `message` is what the user reads and what gets
//! logged.

use serde::Serialize;

/// A command failure, as seen by the frontend.
///
/// Serialises to `{ "code": "crypto.integrity", "message": "..." }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppError {
    /// Stable, machine-readable identifier. Namespaced by subsystem.
    ///
    /// Treat as opaque-but-stable: new codes will be added, existing ones are
    /// not renamed.
    pub code: &'static str,
    /// Human-readable text, safe to show a user. Carries the full underlying
    /// error chain so a bug report still has the detail.
    pub message: String,
}

impl AppError {
    /// Build an error with an explicit code.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// A validation / bad-input failure. The default for anything the caller
    /// got wrong.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }

    /// A policy refusal: the operation was well-formed but is not permitted in
    /// the current configuration. Distinct from `invalid` because the remedy is
    /// "change a setting", not "fix the request" — e.g. Tor refusing a
    /// non-routable address.
    pub fn blocked(message: impl Into<String>) -> Self {
        Self::new("blocked", message)
    }

    /// The requested peer is not connected.
    pub fn not_connected(message: impl Into<String>) -> Self {
        Self::new("not_connected", message)
    }

    /// Persistence failed.
    pub fn storage(message: impl Into<String>) -> Self {
        Self::new("storage", message)
    }

    /// The vault is locked, so key material is unavailable.
    ///
    /// Separate from `not_connected` because the remedy is different: unlock
    /// first, then retry. Worth distinguishing — the UI can offer to unlock.
    pub fn vault_locked(message: impl Into<String>) -> Self {
        Self::new("vault_locked", message)
    }

    /// The passphrase did not meet the strength policy.
    pub fn weak_passphrase(message: impl Into<String>) -> Self {
        Self::new("weak_passphrase", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

// ── Conversions from the existing taxonomy ───────────────────────────────────
//
// Each `From` maps one enum to a code family. Keeping the mapping in one place
// is the point: it is the only place that needs to know the whole taxonomy, and
// it is a `match` per variant rather than 309 scattered `format!` calls.

macro_rules! from_error {
    ($ty:ty, $default:literal, { $($variant:ident => $code:literal),* $(,)? }) => {
        impl From<$ty> for AppError {
            fn from(e: $ty) -> Self {
                let code = match e {
                    $(<$ty>::$variant { .. } => $code,)*
                };
                AppError::new(code, e.to_string())
            }
        }
    };
}

from_error!(crate::crypto::CryptoError, "crypto.error", {
    DecryptionFailed => "crypto.decryption_failed",
    EncryptionFailed => "crypto.encryption_failed",
    SignatureInvalid => "crypto.signature_invalid",
    KeyDerivationFailed => "crypto.key_derivation_failed",
    NonceReuse => "crypto.nonce_reuse",
    DoubleRatchetError => "crypto.ratchet",
    MaxSkippedKeysExceeded => "crypto.too_many_skipped_keys",
    InputTooLarge => "crypto.input_too_large",
    RandomnessUnavailable => "crypto.randomness_unavailable",
    InvalidKeyLength => "crypto.invalid_key_length",
    X3DHFailed => "crypto.x3dh_failed",
    PrekeySignatureInvalid => "crypto.prekey_signature_invalid",
    InitFailed => "crypto.init_failed",
});

from_error!(crate::session::SessionError, "session.error", {
    HandshakeFailed => "session.handshake_failed",
    ReplayDetected => "session.replay_detected",
    NotEstablished => "session.not_established",
    Io => "session.io",
    Network => "session.network",
    Serialization => "session.serialization",
    Crypto => "session.crypto",
    NotConnected => "session.not_connected",
});

from_error!(crate::network::NetworkError, "network.error", {
    Io => "network.io",
    Timeout => "network.timeout",
    ConnectionRefused => "network.refused",
    ConnectionReset => "network.reset",
    FrameTooLarge => "network.frame_too_large",
    RateLimited => "network.rate_limited",
    Protocol => "network.protocol",
    InvalidAddress => "network.invalid_address",
    NotConnected => "network.not_connected",
});

from_error!(crate::protocol::ProtocolError, "protocol.error", {
    SerializationError => "protocol.serialization",
    DeserializationError => "protocol.deserialization",
    InvalidPacketType => "protocol.invalid_packet_type",
    ReservedVersion => "protocol.reserved_version",
    UnsupportedVersion => "protocol.unsupported_version",
    FrameTooLarge => "protocol.frame_too_large",
    InvalidSignature => "protocol.invalid_signature",
    InvalidNonce => "protocol.invalid_nonce",
});

from_error!(crate::storage::StorageError, "storage.error", {
    Database => "storage.database",
    NotFound => "storage.not_found",
    InvalidData => "storage.invalid_data",
    PathError => "storage.path",
    Crypto => "storage.crypto",
    Serialization => "storage.serialization",
    Busy => "storage.busy",
    Constraint => "storage.constraint",
});

from_error!(crate::stun::StunError, "stun.error", {
    Io => "stun.io",
    Timeout => "stun.timeout",
    InvalidResponse => "stun.invalid_response",
    NoMappedAddress => "stun.no_mapped_address",
    AllServersFailed => "stun.all_servers_failed",
    DnsError => "stun.dns",
    TransactionIdMismatch => "stun.transaction_id_mismatch",
    TorBlocked => "stun.blocked_by_tor",
});

from_error!(crate::port_mapping::PortMapError, "portmap.error", {
    Io => "portmap.io",
    NoGateway => "portmap.no_gateway",
    Pcp => "portmap.pcp",
    NatPmp => "portmap.nat_pmp",
    Upnp => "portmap.upnp",
    AllFailed => "portmap.all_failed",
    TorUnsupported => "portmap.blocked_by_tor",
});

from_error!(crate::hole_punch::ConnectionError, "holepunch.error", {
    NoCandidates => "holepunch.no_candidates",
    AllFailed => "holepunch.all_failed",
    TimedOut => "holepunch.timeout",
    Io => "holepunch.io",
    Relay => "holepunch.relay",
});

from_error!(crate::dht::DhtError, "dht.error", {
    Io => "dht.io",
    Timeout => "dht.timeout",
    BadResponse => "dht.bad_response",
    NotFound => "dht.not_found",
    Protocol => "dht.protocol",
    Disabled => "dht.disabled",
});

from_error!(crate::relay::RelayError, "relay.error", {
    Config => "relay.config",
    Io => "relay.io",
    Timeout => "relay.timeout",
    Protocol => "relay.protocol",
    NotConnected => "relay.not_connected",
    ServerError => "relay.server_error",
});

from_error!(crate::tor::TorError, "tor.error", {
    Config => "tor.config",
    Io => "tor.io",
    NotEnabled => "tor.not_enabled",
    ProxyUnreachable => "tor.proxy_unreachable",
});

from_error!(crate::identity::IdentityError, "identity.error", {
    NoIdentity => "identity.missing",
    InvalidKeyLength => "identity.invalid_key_length",
    InvalidSignature => "identity.invalid_signature",
    MalformedInvite => "identity.malformed_invite",
    InviteExpired => "identity.invite_expired",
    InviteTooLong => "identity.invite_too_long",
    AddressHintTooLong => "identity.address_hint_too_long",
    InviteValidityTooLarge => "identity.invalidity_too_large",
    InviteAlreadyConsumed => "identity.invite_consumed",
    InviteReplay => "identity.invite_replay",
    Serialization => "identity.serialization",
    KeyGeneration => "identity.key_generation",
    VersionMismatch => "identity.version_mismatch",
    Fingerprint => "identity.fingerprint",
});

impl From<String> for AppError {
    /// A bare `String` from a command body that has not been given a code yet.
    ///
    /// Exists so the mechanical conversion of the 109 commands is a one-line
    /// change per signature rather than a rewrite of every `Err(...)` and
    /// `map_err` in every body. Code it as you touch it: the `invalid_input`
    /// default is a placeholder, not a decision.
    fn from(s: String) -> Self {
        AppError::invalid(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::invalid(s)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("io", e.to_string())
    }
}
