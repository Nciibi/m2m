/// M2M — Protocol Module
///
/// Defines the wire protocol: packet types, framing, serialization.
/// Every packet is versioned, length-framed, and strictly validated.
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

/// Current protocol version (v0x02 — includes X3DH + Double Ratchet).
pub const PROTOCOL_VERSION: u8 = 0x02;

/// Legacy protocol version (v0x01 — pre-X3DH, SHA-256 KDF ratchet only).
/// Accepted for backward compatibility with older peers.
pub const PROTOCOL_VERSION_LEGACY: u8 = 0x01;

/// Reserved version values that must never be used.
const RESERVED_VERSIONS: [u8; 3] = [0x00, 0xFE, 0xFF];

/// Maximum frame size: 16 MiB (version + payload, excluding the 4-byte length prefix).
pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Maximum text message size: 64 KiB.
pub const MAX_TEXT_MESSAGE_SIZE: usize = 64 * 1024;

/// Maximum file chunk size: 256 KiB.
pub const MAX_FILE_CHUNK_SIZE: usize = 256 * 1024;

/// Maximum handshake message size: 4 KiB.
#[allow(dead_code)]
pub const MAX_HANDSHAKE_SIZE: usize = 4 * 1024;

/// Minimum frame size: version (1) + at least 1 byte payload type.
pub const MIN_FRAME_SIZE: u32 = 2;

/// Length prefix size in bytes.
pub const LENGTH_PREFIX_SIZE: usize = 4;

/// Heartbeat interval in seconds.
/// A heartbeat is sent every interval to keep the connection alive
/// and detect silent disconnections.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Heartbeat timeout in seconds.
/// If no HeartbeatAck is received within this time, the connection
/// is considered dead and will be cleaned up.
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 10;

/// Maximum session duration in seconds (24 hours).
pub const MAX_SESSION_DURATION_SECS: u64 = 24 * 60 * 60;

/// Key rotation interval in seconds (1 hour, reserved for future use).
#[allow(dead_code)]
pub const KEY_ROTATION_INTERVAL_SECS: u64 = 60 * 60;

/// Maximum invite validity duration in seconds (24 hours).
pub const MAX_INVITE_VALIDITY_SECS: u64 = 24 * 60 * 60;

/// Clock skew tolerance for invite validation (5 minutes).
pub const CLOCK_SKEW_TOLERANCE_SECS: u64 = 5 * 60;

/// Maximum invite string length.
pub const MAX_INVITE_LENGTH: usize = 512;

/// Maximum address hint length.
pub const MAX_ADDRESS_HINT_LENGTH: usize = 256;

/// Rate limit: max messages per second from a single peer.
#[allow(dead_code)]
pub const RATE_LIMIT_MSGS_PER_SEC: u32 = 20;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unsupported protocol version: {0:#04x}")]
    UnsupportedVersion(u8),
    #[error("reserved protocol version: {0:#04x}")]
    ReservedVersion(u8),
    #[error("frame too large: {size} bytes exceeds {max} byte limit")]
    FrameTooLarge { size: u32, max: u32 },
    #[error("frame too small: {size} bytes below minimum {min}")]
    FrameTooSmall { size: u32, min: u32 },
    #[error("unknown packet type: {0:#04x}")]
    UnknownPacketType(u8),
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("deserialization error: {0}")]
    DeserializationError(String),
    #[allow(dead_code)]
    #[error("invalid handshake message")]
    InvalidHandshake,
    #[allow(dead_code)]
    #[error("invalid invite format")]
    InvalidInvite,
    #[allow(dead_code)]
    #[error("invite expired")]
    InviteExpired,
    #[allow(dead_code)]
    #[error("invite signature invalid")]
    InviteSignatureInvalid,
    #[allow(dead_code)]
    #[error("invalid sequence number")]
    InvalidSequence,
    #[error("message too large")]
    MessageTooLarge,
}

/// Packet type identifiers. Each maps to a specific message structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketType {
    HandshakeInit = 0x01,
    HandshakeResponse = 0x02,
    HandshakeComplete = 0x03,
    X3DHHandshakeInit = 0x04,
    X3DHHandshakeResponse = 0x05,
    X3DHComplete = 0x06,
    EncryptedMessage = 0x10,
    FileTransferRequest = 0x11,
    FileTransferChunk = 0x12,
    FileTransferComplete = 0x13,
    FileTransferAccept = 0x14,
    FileTransferReject = 0x15,
    Heartbeat = 0x20,
    HeartbeatAck = 0x21,
    Disconnect = 0x30,
    Error = 0x31,
    ConversationMeta = 0x40,
}

impl PacketType {
    /// Parse a packet type from a raw byte. Unknown types are rejected.
    pub fn from_byte(byte: u8) -> Result<Self, ProtocolError> {
        match byte {
            0x01 => Ok(PacketType::HandshakeInit),
            0x02 => Ok(PacketType::HandshakeResponse),
            0x03 => Ok(PacketType::HandshakeComplete),
            0x04 => Ok(PacketType::X3DHHandshakeInit),
            0x05 => Ok(PacketType::X3DHHandshakeResponse),
            0x06 => Ok(PacketType::X3DHComplete),
            0x10 => Ok(PacketType::EncryptedMessage),
            0x11 => Ok(PacketType::FileTransferRequest),
            0x12 => Ok(PacketType::FileTransferChunk),
            0x13 => Ok(PacketType::FileTransferComplete),
            0x14 => Ok(PacketType::FileTransferAccept),
            0x15 => Ok(PacketType::FileTransferReject),
            0x20 => Ok(PacketType::Heartbeat),
            0x21 => Ok(PacketType::HeartbeatAck),
            0x30 => Ok(PacketType::Disconnect),
            0x31 => Ok(PacketType::Error),
            0x40 => Ok(PacketType::ConversationMeta),
            other => Err(ProtocolError::UnknownPacketType(other)),
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Validate a protocol version byte.
///
/// Accepts both the current version (0x02) and legacy version (0x01).
/// Logs a deprecation notice when a legacy peer connects.
/// Reserved versions (0x00, 0xFE, 0xFF) are always rejected.
pub fn validate_version(version: u8) -> Result<(), ProtocolError> {
    if RESERVED_VERSIONS.contains(&version) {
        return Err(ProtocolError::ReservedVersion(version));
    }
    if version == PROTOCOL_VERSION_LEGACY {
        tracing::warn!(
            "peer using legacy protocol version 0x01 — consider upgrading"
        );
        return Ok(());
    }
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(version));
    }
    Ok(())
}

/// Validate a frame size.
pub fn validate_frame_size(size: u32) -> Result<(), ProtocolError> {
    if size < MIN_FRAME_SIZE {
        return Err(ProtocolError::FrameTooSmall {
            size,
            min: MIN_FRAME_SIZE,
        });
    }
    if size > MAX_FRAME_SIZE {
        return Err(ProtocolError::FrameTooLarge {
            size,
            max: MAX_FRAME_SIZE,
        });
    }
    Ok(())
}

// --- ICE Candidate (Wire Format) ---

/// A network candidate in wire format, exchanged during handshake.
/// This is a compact representation — the full candidate object lives
/// only in the candidate module and is not serialized over the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCandidate {
    /// IP:port address (e.g. "1.2.3.4:5678").
    pub address: String,
    /// Candidate type as u8: 0=host, 1=srflx, 2=prflx, 3=relay.
    pub candidate_type: u8,
    /// Relay ID for type-3 (relay) candidates.
    /// Set by the invite creator when registering with a relay server.
    /// The connecting peer sends this to the relay to request bridging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_id: Option<String>,
}

// --- Handshake Messages ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeInit {
    pub version: u8,
    pub ephemeral_pub: [u8; 32],
    pub identity_pub: [u8; 32],
    pub timestamp: u64,
    pub signature: Vec<u8>,
    /// Network candidates for ICE-Lite connectivity.
    #[serde(default)]
    pub candidates: Vec<WireCandidate>,
    /// X25519 identity public key (X3DH). NEW in protocol v2, appended for backward compat.
    #[serde(default)]
    pub x25519_identity_pub: [u8; 32],
    /// The one-time prekey consumed, if any (X3DH).
    #[serde(default)]
    pub used_opk: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub version: u8,
    pub ephemeral_pub: [u8; 32],
    pub identity_pub: [u8; 32],
    pub timestamp: u64,
    pub signature: Vec<u8>,
    /// Network candidates for ICE-Lite connectivity.
    #[serde(default)]
    pub candidates: Vec<WireCandidate>,
    /// X25519 identity public key (X3DH). NEW in protocol v2, appended for backward compat.
    #[serde(default)]
    pub x25519_identity_pub: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeComplete {
    pub encrypted_verify: Vec<u8>,
    pub nonce: Vec<u8>,
}

// --- Double Ratchet Header ---

/// Header for Double Ratchet encrypted messages.
/// Carries the DH ratchet key (if ratcheting) and message number for chain derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DRHeader {
    /// New DH ratchet public key (None for continuation messages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratchet_key: Option<[u8; 32]>,
    /// Number of messages in the previous sending chain (PN in the spec).
    pub previous_chain_length: u32,
    /// Message number within the current chain (N in the spec).
    pub message_number: u64,
}

// --- Encrypted Message Envelope ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub nonce: Vec<u8>,
    /// Message counter (legacy, used in pre-X3DH sessions).
    #[serde(default)]
    pub counter: u64,
    pub ciphertext: Vec<u8>,
    /// Double Ratchet header (used in X3DH+DR sessions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dr_header: Option<DRHeader>,
}

// --- Inner Message Types (decrypted content) ---

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize)]
#[serde(tag = "type")]
pub enum MessageBody {
    #[serde(rename = "text")]
    Text { id: String, content: String },
    #[serde(rename = "ack")]
    Ack { id: String },
}

impl Drop for MessageBody {
    fn drop(&mut self) {
        self.zeroize();
    }
}

// --- File Transfer Messages ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequestData {
    pub transfer_id: String,
    pub filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub file_hash: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferChunkData {
    pub transfer_id: String,
    pub chunk_index: u32,
    pub data: Vec<u8>,
    pub chunk_hash: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferCompleteData {
    pub transfer_id: String,
}

/// Request to accept an incoming file transfer (type 0x14).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferAcceptData {
    pub transfer_id: String,
}

/// Request to reject an incoming file transfer (type 0x15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRejectData {
    pub transfer_id: String,
}

// --- Conversation Metadata ---

/// Exchanged between peers after handshake to set conversation display names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetaData {
    /// The name the sender chose for their own side of this conversation.
    pub my_display_name: String,
    /// The name the sender suggests for the receiver's side.
    pub your_display_name: String,
}

// --- Disconnect ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DisconnectReason {
    UserInitiated = 0x01,
    SessionExpired = 0x02,
    Error = 0x03,
    VersionMismatch = 0x04,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectMessage {
    pub reason: DisconnectReason,
}

// --- Error Codes ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum ErrorCode {
    UnknownPacketType = 0x0001,
    FrameTooLarge = 0x0002,
    HandshakeFailed = 0x0003,
    DecryptionFailed = 0x0004,
    InvalidSequence = 0x0005,
    SessionExpired = 0x0006,
    RateLimitExceeded = 0x0007,
    VersionMismatch = 0x0008,
    InternalError = 0x0009,
    InvalidSignature = 0x000A,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: ErrorCode,
    pub description: String,
}

// --- Invite Format ---

/// Invite link prefix.
pub const INVITE_PREFIX: &str = "m2m://";

/// Invite flags.
pub const INVITE_FLAG_ONE_TIME: u8 = 0x01;
pub const INVITE_FLAG_LISTENER: u8 = 0x02;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitePayload {
    pub version: u8,
    pub identity_pub: [u8; 32],
    /// X25519 public key for X3DH key agreement.
    #[serde(default)]
    pub x25519_identity_pub: [u8; 32],
    /// X25519 signed prekey public key (X3DH).
    #[serde(default)]
    pub signed_prekey: [u8; 32],
    /// Ed25519 signature over the signed prekey, binding it to the identity.
    #[serde(default)]
    pub signed_prekey_sig: Vec<u8>,
    /// Optional one-time prekey for forward secrecy (X3DH).
    #[serde(default)]
    pub one_time_prekey: Option<[u8; 32]>,
    pub address_hint: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub nonce: Vec<u8>,
    pub flags: u8,
    /// Network candidates for ICE-Lite connectivity (host, srflx).
    #[serde(default)]
    pub candidates: Vec<WireCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedInvite {
    pub payload: InvitePayload,
    pub signature: Vec<u8>,
}

/// Serialize a packet body to MessagePack bytes.
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    rmp_serde::to_vec(value).map_err(|e| ProtocolError::SerializationError(e.to_string()))
}

/// Deserialize a packet body from MessagePack bytes.
pub fn deserialize<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, ProtocolError> {
    rmp_serde::from_slice(bytes).map_err(|e| ProtocolError::DeserializationError(e.to_string()))
}

/// Build a complete wire frame: [length (4B)] [version (1B)] [type (1B)] [body]
pub fn build_frame(packet_type: PacketType, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    // payload = version (1) + type (1) + body
    let payload_len = 1 + 1 + body.len();
    let total_len = payload_len as u32;
    validate_frame_size(total_len)?;

    let mut frame = Vec::with_capacity(LENGTH_PREFIX_SIZE + payload_len);
    frame.extend_from_slice(&total_len.to_be_bytes());
    frame.push(PROTOCOL_VERSION);
    frame.push(packet_type.to_byte());
    frame.extend_from_slice(body);
    Ok(frame)
}

#[cfg(test)]
mod protocol_tests {
    use super::*;

    // ─── PacketType parsing ─────────────────────────────────────

    #[test]
    fn test_all_valid_packet_types_roundtrip() {
        let valid: &[(u8, PacketType)] = &[
            (0x01, PacketType::HandshakeInit),
            (0x02, PacketType::HandshakeResponse),
            (0x03, PacketType::HandshakeComplete),
            (0x10, PacketType::EncryptedMessage),
            (0x11, PacketType::FileTransferRequest),
            (0x12, PacketType::FileTransferChunk),
            (0x13, PacketType::FileTransferComplete),
            (0x14, PacketType::FileTransferAccept),
            (0x15, PacketType::FileTransferReject),
            (0x20, PacketType::Heartbeat),
            (0x21, PacketType::HeartbeatAck),
            (0x30, PacketType::Disconnect),
            (0x31, PacketType::Error),
            (0x40, PacketType::ConversationMeta),
        ];
        for &(byte, expected) in valid {
            let parsed = PacketType::from_byte(byte).unwrap();
            assert_eq!(parsed, expected, "from_byte(0x{byte:02X}) failed");
            assert_eq!(parsed.to_byte(), byte, "to_byte() roundtrip failed for 0x{byte:02X}");
        }
    }

    #[test]
    fn test_unknown_packet_type_rejected() {
        let invalid_bytes: &[u8] = &[0x00, 0x0F, 0x16, 0x22, 0x32, 0x41, 0x50, 0xFF];
        for &byte in invalid_bytes {
            assert!(
                PacketType::from_byte(byte).is_err(),
                "byte 0x{byte:02X} should be rejected as unknown"
            );
        }
    }

    // ─── Version validation ─────────────────────────────────────

    #[test]
    fn test_valid_version() {
        assert!(validate_version(PROTOCOL_VERSION).is_ok());
    }

    #[test]
    fn test_reserved_versions_rejected() {
        // 0x00, 0xFE, 0xFF are reserved
        assert!(matches!(validate_version(0x00), Err(ProtocolError::ReservedVersion(0x00))));
        assert!(matches!(validate_version(0xFE), Err(ProtocolError::ReservedVersion(0xFE))));
        assert!(matches!(validate_version(0xFF), Err(ProtocolError::ReservedVersion(0xFF))));
    }

    #[test]
    fn test_unsupported_version_rejected() {
        // Anything that's not reserved and not a known version
        assert!(matches!(validate_version(0x10), Err(ProtocolError::UnsupportedVersion(0x10))));
        assert!(matches!(validate_version(0x03), Err(ProtocolError::UnsupportedVersion(0x03))));
        assert!(matches!(validate_version(0xFD), Err(ProtocolError::UnsupportedVersion(0xFD))));
    }

    #[test]
    fn test_legacy_version_accepted() {
        // 0x01 is the legacy version — should be accepted with warning
        assert!(validate_version(PROTOCOL_VERSION_LEGACY).is_ok());
    }

    #[test]
    fn test_current_version_accepted() {
        assert!(validate_version(PROTOCOL_VERSION).is_ok());
    }

    // ─── Frame size validation ──────────────────────────────────

    #[test]
    fn test_frame_size_minimum_boundary() {
        assert!(validate_frame_size(MIN_FRAME_SIZE).is_ok());
        assert!(validate_frame_size(MIN_FRAME_SIZE - 1).is_err());
    }

    #[test]
    fn test_frame_size_maximum_boundary() {
        assert!(validate_frame_size(MAX_FRAME_SIZE).is_ok());
        assert!(validate_frame_size(MAX_FRAME_SIZE + 1).is_err());
    }

    #[test]
    fn test_frame_size_zero_rejected() {
        assert!(matches!(
            validate_frame_size(0),
            Err(ProtocolError::FrameTooSmall { size: 0, min: MIN_FRAME_SIZE })
        ));
    }

    #[test]
    fn test_frame_size_overflow_rejected() {
        assert!(matches!(
            validate_frame_size(u32::MAX),
            Err(ProtocolError::FrameTooLarge { .. })
        ));
    }

    // ─── build_frame structure ──────────────────────────────────

    #[test]
    fn test_build_frame_structure() {
        let body = b"hello";
        let frame = build_frame(PacketType::EncryptedMessage, body).unwrap();

        // Frame layout: [4B length] [1B version] [1B type] [body]
        assert_eq!(frame.len(), 4 + 1 + 1 + body.len());

        // Length prefix = payload length (version + type + body)
        let payload_len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]);
        assert_eq!(payload_len as usize, 1 + 1 + body.len());

        // Version byte
        assert_eq!(frame[4], PROTOCOL_VERSION);

        // Packet type byte
        assert_eq!(frame[5], PacketType::EncryptedMessage.to_byte());

        // Body bytes
        assert_eq!(&frame[6..], body);
    }

    #[test]
    fn test_build_frame_empty_body() {
        let frame = build_frame(PacketType::Heartbeat, &[]).unwrap();
        // Minimum valid frame: 4B length + 1B version + 1B type
        assert_eq!(frame.len(), 6);
        let payload_len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]);
        assert_eq!(payload_len, MIN_FRAME_SIZE);
    }

    #[test]
    fn test_build_frame_all_packet_types() {
        // Every packet type should produce a valid frame
        let types = [
            PacketType::HandshakeInit,
            PacketType::HandshakeResponse,
            PacketType::HandshakeComplete,
            PacketType::EncryptedMessage,
            PacketType::FileTransferRequest,
            PacketType::FileTransferChunk,
            PacketType::FileTransferComplete,
            PacketType::FileTransferAccept,
            PacketType::FileTransferReject,
            PacketType::Heartbeat,
            PacketType::HeartbeatAck,
            PacketType::Disconnect,
            PacketType::Error,
            PacketType::ConversationMeta,
        ];
        for pt in types {
            let frame = build_frame(pt, b"test");
            assert!(frame.is_ok(), "build_frame failed for {:?}", pt);
        }
    }

    // ─── Serialization roundtrips ───────────────────────────────

    #[test]
    fn test_serialize_deserialize_disconnect() {
        let msg = DisconnectMessage {
            reason: DisconnectReason::UserInitiated,
        };
        let bytes = serialize(&msg).unwrap();
        let decoded: DisconnectMessage = deserialize(&bytes).unwrap();
        assert_eq!(decoded.reason, DisconnectReason::UserInitiated);
    }

    #[test]
    fn test_serialize_deserialize_error_message() {
        let msg = ErrorMessage {
            code: ErrorCode::RateLimitExceeded,
            description: "too many connections".to_string(),
        };
        let bytes = serialize(&msg).unwrap();
        let decoded: ErrorMessage = deserialize(&bytes).unwrap();
        assert_eq!(decoded.code, ErrorCode::RateLimitExceeded);
        assert_eq!(decoded.description, "too many connections");
    }

    #[test]
    fn test_serialize_deserialize_encrypted_envelope() {
        let env = EncryptedEnvelope {
            nonce: vec![0xAA; 24],
            counter: 42,
            ciphertext: vec![0xBB; 128],
            dr_header: None,
        };
        let bytes = serialize(&env).unwrap();
        let decoded: EncryptedEnvelope = deserialize(&bytes).unwrap();
        assert_eq!(decoded.nonce, env.nonce);
        assert_eq!(decoded.counter, 42);
        assert_eq!(decoded.ciphertext, env.ciphertext);
    }

    #[test]
    fn test_serialize_deserialize_message_body_text() {
        let body = MessageBody::Text {
            id: "msg-001".to_string(),
            content: "Hello, world! 🔒".to_string(),
        };
        let bytes = serialize(&body).unwrap();
        let decoded: MessageBody = deserialize(&bytes).unwrap();
        match &decoded {
            MessageBody::Text { id, content } => {
                assert_eq!(id, "msg-001");
                assert_eq!(content, "Hello, world! 🔒");
            }
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn test_serialize_deserialize_file_transfer_request() {
        let req = FileTransferRequestData {
            transfer_id: "xfer-001".to_string(),
            filename: "document.pdf".to_string(),
            total_size: 1_048_576,
            total_chunks: 16,
            file_hash: vec![0xCC; 32],
        };
        let bytes = serialize(&req).unwrap();
        let decoded: FileTransferRequestData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.transfer_id, "xfer-001");
        assert_eq!(decoded.filename, "document.pdf");
        assert_eq!(decoded.total_size, 1_048_576);
        assert_eq!(decoded.total_chunks, 16);
        assert_eq!(decoded.file_hash.len(), 32);
    }

    #[test]
    fn test_serialize_deserialize_conversation_meta() {
        let meta = ConversationMetaData {
            my_display_name: "Alice".to_string(),
            your_display_name: "Bob".to_string(),
        };
        let bytes = serialize(&meta).unwrap();
        let decoded: ConversationMetaData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.my_display_name, "Alice");
        assert_eq!(decoded.your_display_name, "Bob");
    }

    #[test]
    fn test_deserialize_garbage_rejected() {
        let garbage = vec![0xFF, 0x00, 0x01, 0x02];
        let result: Result<DisconnectMessage, _> = deserialize(&garbage);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_deserialize_handshake_with_candidates() {
        let init = HandshakeInit {
            version: PROTOCOL_VERSION,
            ephemeral_pub: [0xAA; 32],
            identity_pub: [0xBB; 32],
            x25519_identity_pub: [0xBB; 32],
            used_opk: None,
            timestamp: 1719446400,
            signature: vec![0xCC; 64],
            candidates: vec![
                WireCandidate { address: "192.168.1.5:12345".to_string(), candidate_type: 0, relay_id: None },
                WireCandidate { address: "1.2.3.4:54321".to_string(), candidate_type: 1, relay_id: None },
            ],
        };
        let bytes = serialize(&init).unwrap();
        let decoded: HandshakeInit = deserialize(&bytes).unwrap();
        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.candidates.len(), 2);
        assert_eq!(decoded.candidates[0].address, "192.168.1.5:12345");
        assert_eq!(decoded.candidates[1].candidate_type, 1);
    }

    #[test]
    fn test_serialize_deserialize_handshake_no_candidates() {
        // Candidates are optional (skip_serializing_if = Vec::is_empty)
        let init = HandshakeInit {
            version: PROTOCOL_VERSION,
            ephemeral_pub: [0xAA; 32],
            identity_pub: [0xBB; 32],
            x25519_identity_pub: [0xBB; 32],
            used_opk: None,
            timestamp: 1719446400,
            signature: vec![0xCC; 64],
            candidates: vec![],
        };
        let bytes = serialize(&init).unwrap();
        let decoded: HandshakeInit = deserialize(&bytes).unwrap();
        assert!(decoded.candidates.is_empty());
    }

    // ─── Constants sanity checks ────────────────────────────────

    #[test]
    fn test_protocol_constants_sane() {
        assert!(MAX_FRAME_SIZE >= MIN_FRAME_SIZE);
        assert!(MAX_TEXT_MESSAGE_SIZE < MAX_FRAME_SIZE as usize);
        assert!(MAX_FILE_CHUNK_SIZE < MAX_FRAME_SIZE as usize);
        assert!(MAX_SESSION_DURATION_SECS > 0);
        assert!(MAX_INVITE_VALIDITY_SECS > 0);
        assert!(CLOCK_SKEW_TOLERANCE_SECS > 0);
        assert!(LENGTH_PREFIX_SIZE == 4);
    }
}
