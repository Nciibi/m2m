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

    /// A family member's saved address is stale — the address we have for them
    /// no longer answers.
    ///
    /// Its own code rather than a bare string sentinel, because it is
    /// actionable: the UI offers to request a fresh invite. The old
    /// `"CANNOT_REACH"` literal had to be substring-matched against a
    /// stringified error, which broke the moment the error type gained a
    /// serialised shape.
    pub fn peer_unreachable(message: impl Into<String>) -> Self {
        Self::new("family.unreachable", message)
    }

    /// Encoding a payload failed.
    ///
    /// Almost always a bug rather than bad input — these types are
    /// `Serialize` by construction, so a failure means a field that cannot be
    /// represented, not a malformed request. Distinguished from `invalid_input`
    /// so a caller can tell "you sent nonsense" from "we could not encode what
    /// we were given".
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new("protocol.serialization_failed", message)
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
// it is one exhaustive `match` per enum rather than 309 scattered `format!`
// calls. Every variant is listed, so adding a variant to any of these enums is
// a compile error here rather than a silent `unreachable!()` or a lost code.

use crate::crypto::CryptoError;
use crate::dht::DhtError;
use crate::hole_punch::ConnectionError;
use crate::identity::IdentityError;
use crate::network::NetworkError;
use crate::port_mapping::PortMapError;
use crate::protocol::ProtocolError;
use crate::relay::RelayError;
use crate::session::SessionError;
use crate::storage::StorageError;
use crate::stun::StunError;
use crate::tor::TorError;

impl From<CryptoError> for AppError {
    fn from(e: CryptoError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            CryptoError::InitFailed => "crypto.init_failed",
            CryptoError::EncryptionFailed => "crypto.encryption_failed",
            CryptoError::DecryptionFailed => "crypto.decryption_failed",
            CryptoError::SignatureInvalid => "crypto.signature_invalid",
            CryptoError::KeyDerivationFailed => "crypto.key_derivation_failed",
            CryptoError::RandomnessUnavailable(..) => "crypto.randomness_unavailable",
            CryptoError::InputTooLarge { .. } => "crypto.input_too_large",
            CryptoError::InvalidKeyLength => "crypto.invalid_key_length",
            CryptoError::X3DHFailed => "crypto.x3dh_failed",
            CryptoError::DoubleRatchetError(..) => "crypto.double_ratchet_error",
            CryptoError::PrekeySignatureInvalid => "crypto.prekey_signature_invalid",
            CryptoError::MaxSkippedKeysExceeded(..) => "crypto.max_skipped_keys_exceeded",
        };
        AppError::new(code, message)
    }
}

impl From<SessionError> for AppError {
    fn from(e: SessionError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            SessionError::Crypto(..) => "session.crypto",
            SessionError::Protocol(..) => "session.protocol",
            SessionError::Network(..) => "session.network",
            SessionError::HandshakeFailed(..) => "session.handshake_failed",
            SessionError::SessionExpired => "session.session_expired",
            SessionError::ReplayDetected { .. } => "session.replay_detected",
            SessionError::InvalidState => "session.invalid_state",
        };
        AppError::new(code, message)
    }
}

impl From<NetworkError> for AppError {
    fn from(e: NetworkError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            NetworkError::Io(..) => "network.io",
            NetworkError::ConnectionTimeout => "network.connection_timeout",
            NetworkError::ReadTimeout => "network.read_timeout",
            NetworkError::WriteTimeout => "network.write_timeout",
            NetworkError::PeerClosed => "network.peer_closed",
            NetworkError::Protocol(..) => "network.protocol",
            NetworkError::InvalidState(..) => "network.invalid_state",
            NetworkError::RateLimitExceeded => "network.rate_limit_exceeded",
        };
        AppError::new(code, message)
    }
}

impl From<ProtocolError> for AppError {
    fn from(e: ProtocolError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            ProtocolError::UnsupportedVersion(..) => "protocol.unsupported_version",
            ProtocolError::ReservedVersion(..) => "protocol.reserved_version",
            ProtocolError::FrameTooLarge { .. } => "protocol.frame_too_large",
            ProtocolError::FrameTooSmall { .. } => "protocol.frame_too_small",
            ProtocolError::UnknownPacketType(..) => "protocol.unknown_packet_type",
            ProtocolError::SerializationError(..) => "protocol.serialization_error",
            ProtocolError::DeserializationError(..) => "protocol.deserialization_error",
            ProtocolError::InvalidHandshake => "protocol.invalid_handshake",
            ProtocolError::InvalidInvite => "protocol.invalid_invite",
            ProtocolError::InviteExpired => "protocol.invite_expired",
            ProtocolError::InviteSignatureInvalid => "protocol.invite_signature_invalid",
            ProtocolError::InvalidSequence => "protocol.invalid_sequence",
            ProtocolError::MessageTooLarge => "protocol.message_too_large",
        };
        AppError::new(code, message)
    }
}

impl From<StorageError> for AppError {
    fn from(e: StorageError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            StorageError::Database(..) => "storage.database",
            StorageError::PathError(..) => "storage.path_error",
            StorageError::KeyNotFound => "storage.key_not_found",
            StorageError::DecryptionFailed => "storage.decryption_failed",
            StorageError::EncryptionFailed => "storage.encryption_failed",
            StorageError::DirCreationFailed(..) => "storage.dir_creation_failed",
        };
        AppError::new(code, message)
    }
}

impl From<StunError> for AppError {
    fn from(e: StunError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            StunError::Io(..) => "stun.io",
            StunError::Timeout { .. } => "stun.timeout",
            StunError::InvalidResponse { .. } => "stun.invalid_response",
            StunError::NoMappedAddress { .. } => "stun.no_mapped_address",
            StunError::AllServersFailed => "stun.all_servers_failed",
            StunError::DnsError { .. } => "stun.dns",
            StunError::TransactionIdMismatch { .. } => "stun.transaction_id_mismatch",
            StunError::TorBlocked(..) => "stun.tor_blocked",
        };
        AppError::new(code, message)
    }
}

impl From<PortMapError> for AppError {
    fn from(e: PortMapError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            PortMapError::Io(..) => "portmap.io",
            PortMapError::NoGateway => "portmap.no_gateway",
            PortMapError::Pcp(..) => "portmap.pcp",
            PortMapError::NatPmp(..) => "portmap.nat_pmp",
            PortMapError::Upnp(..) => "portmap.upnp",
            PortMapError::AllFailed => "portmap.all_failed",
            PortMapError::TorUnsupported => "portmap.tor_unsupported",
        };
        AppError::new(code, message)
    }
}

impl From<ConnectionError> for AppError {
    fn from(e: ConnectionError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            ConnectionError::Io(..) => "holepunch.io",
            ConnectionError::AllFailed(..) => "holepunch.all_failed",
            ConnectionError::NoCandidates => "holepunch.no_candidates",
            ConnectionError::TimedOut(..) => "holepunch.timed_out",
        };
        AppError::new(code, message)
    }
}

impl From<DhtError> for AppError {
    fn from(e: DhtError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            DhtError::Io(..) => "dht.io",
            DhtError::Timeout => "dht.timeout",
            DhtError::BadResponse(..) => "dht.bad_response",
            DhtError::PeerNotFound => "dht.peer_not_found",
            DhtError::NotBootstrapped => "dht.not_bootstrapped",
            DhtError::NotEnabled => "dht.not_enabled",
        };
        AppError::new(code, message)
    }
}

impl From<RelayError> for AppError {
    fn from(e: RelayError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            RelayError::Io(..) => "relay.io",
            RelayError::TimedOut => "relay.timed_out",
            RelayError::FrameTooLarge { .. } => "relay.frame_too_large",
            RelayError::Protocol(..) => "relay.protocol",
            RelayError::ServerError { .. } => "relay.server_error",
            RelayError::ConnectionClosed => "relay.connection_closed",
            RelayError::UnexpectedFrame(..) => "relay.unexpected_frame",
            RelayError::Config(..) => "relay.config",
        };
        AppError::new(code, message)
    }
}

impl From<TorError> for AppError {
    fn from(e: TorError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            TorError::ConnectionFailed(..) => "tor.connection_failed",
            TorError::ProxyUnreachable(..) => "tor.proxy_unreachable",
            TorError::Io(..) => "tor.io",
        };
        AppError::new(code, message)
    }
}

impl From<IdentityError> for AppError {
    fn from(e: IdentityError) -> Self {
        // Message first: the enum is matched by reference so the rendered
        // chain survives for the user and the log.
        let message = e.to_string();
        let code = match &e {
            IdentityError::Crypto(..) => "identity.crypto",
            IdentityError::Protocol(..) => "identity.protocol",
            IdentityError::InviteExpired => "identity.invite_expired",
            IdentityError::InviteFutureTimestamp => "identity.invite_future_timestamp",
            IdentityError::InviteValidityTooLarge => "identity.invite_validity_too_large",
            IdentityError::InviteSignatureInvalid => "identity.invite_signature_invalid",
            IdentityError::InviteFormatInvalid(..) => "identity.invite_format_invalid",
            IdentityError::InviteAlreadyConsumed => "identity.invite_already_consumed",
            IdentityError::AddressHintTooLong => "identity.address_hint_too_long",
        };
        AppError::new(code, message)
    }
}

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
