/// M2M — Protocol Module
///
/// Defines the wire protocol: packet types, framing, serialization.
/// Every packet is versioned, length-framed, and strictly validated.
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

/// Current protocol version.
pub const PROTOCOL_VERSION: u8 = 0x01;

/// Reserved version values that must never be used.
const RESERVED_VERSIONS: [u8; 3] = [0x00, 0xFE, 0xFF];

/// Maximum frame size: 16 MiB (version + payload, excluding the 4-byte length prefix).
pub const MAX_FRAME_SIZE: u32 = 16 * 1024 * 1024;

/// Maximum text message size: 64 KiB.
pub const MAX_TEXT_MESSAGE_SIZE: usize = 64 * 1024;

/// Maximum file chunk size: 256 KiB.
pub const MAX_FILE_CHUNK_SIZE: usize = 256 * 1024;

/// Maximum handshake message size: 4 KiB.
pub const MAX_HANDSHAKE_SIZE: usize = 4 * 1024;

/// Minimum frame size: version (1) + at least 1 byte payload type.
pub const MIN_FRAME_SIZE: u32 = 2;

/// Length prefix size in bytes.
pub const LENGTH_PREFIX_SIZE: usize = 4;

/// Heartbeat interval in seconds.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Heartbeat timeout in seconds.
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 10;

/// Maximum session duration in seconds (24 hours).
pub const MAX_SESSION_DURATION_SECS: u64 = 24 * 60 * 60;

/// Key rotation interval in seconds (1 hour).
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
    #[error("invalid handshake message")]
    InvalidHandshake,
    #[error("invalid invite format")]
    InvalidInvite,
    #[error("invite expired")]
    InviteExpired,
    #[error("invite signature invalid")]
    InviteSignatureInvalid,
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
pub fn validate_version(version: u8) -> Result<(), ProtocolError> {
    if RESERVED_VERSIONS.contains(&version) {
        return Err(ProtocolError::ReservedVersion(version));
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

// --- Handshake Messages ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeInit {
    pub version: u8,
    pub ephemeral_pub: [u8; 32],
    pub identity_pub: [u8; 32],
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub version: u8,
    pub ephemeral_pub: [u8; 32],
    pub identity_pub: [u8; 32],
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeComplete {
    pub encrypted_verify: Vec<u8>,
    pub nonce: Vec<u8>,
}

// --- Encrypted Message Envelope ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub nonce: Vec<u8>,
    pub counter: u64,
    pub ciphertext: Vec<u8>,
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
    pub address_hint: String,
    pub created_at: u64,
    pub expires_at: u64,
    pub nonce: Vec<u8>,
    pub flags: u8,
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
