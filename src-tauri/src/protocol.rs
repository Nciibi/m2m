/// M2M — Protocol Module
///
/// Defines the wire protocol: packet types, framing, serialization.
/// Every packet is versioned, length-framed, and strictly validated.
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::Zeroize;

/// Current protocol version (v0x03 — X3DH + Double Ratchet, authenticated
/// DR header).
///
/// ## Why 0x03 and not 0x02
///
/// The Double Ratchet header (`ratchet_key`, `message_number`) is now folded
/// into the AEAD's associated data. That is a **breaking wire-format change**:
/// a v0x03 peer computes `AAD = context ‖ has_ratchet ‖ ratchet_key ‖
/// message_number`, while a v0x02 peer computes just `context`.
///
/// Both would previously have reported `PROTOCOL_VERSION = 0x02` and passed the
/// version handshake, then failed to decrypt *every* message with no useful
/// diagnostic — indistinguishable from a broken network. Bumping the version
/// makes that failure mode explicit: `validate_version` rejects the peer at
/// handshake time with `UnsupportedVersion`, and the user is told to upgrade.
///
/// Never make an authenticated-format change without a version bump. If a
/// future change *is* backward compatible, note that here explicitly.
pub const PROTOCOL_VERSION: u8 = 0x03;

/// Legacy protocol version (v0x01 — pre-X3DH, SHA-256 KDF ratchet only).
///
/// **Not accepted.** Retained only so [`validate_version`] and its tests can
/// name the version it refuses.
///
/// v0x01 is pre-X3DH: there is no one-time prekey and no signed prekey, so a
/// session established with it has no forward secrecy once the peer's long-term
/// key is compromised. Accepting it was the downgrade path this project states
/// it does not have.
///
/// v0x02 (X3DH + Double Ratchet with an *unauthenticated* DR header) is likewise
/// NOT accepted: its AAD differs from v0x03's, so a v0x02 peer would handshake
/// and then fail to decrypt everything.
pub const PROTOCOL_VERSION_LEGACY: u8 = 0x01;

/// Reserved version values that must never be used.
const RESERVED_VERSIONS: [u8; 3] = [0x00, 0xFE, 0xFF];

/// Maximum frame size: 1 MiB (version + payload, excluding the 4-byte length prefix).
///
/// ## Why this dropped from 16 MiB
///
/// The old 16 MiB ceiling was a single unattributed number applied to every
/// packet type. Nothing in the protocol needs it: the largest legitimate
/// payload is a padded text message (64 KiB, doubling at worst under
/// variable padding) or a file chunk (256 KiB, plus AEAD/tag overhead).
///
/// The cost was that a *declared* length of `0x01000000` forced a 16 MiB
/// zeroed allocation before a single body byte arrived, and the 1-second
/// per-read Slowloris timeout let an attacker hold that allocation indefinitely
/// while trickling bytes. Multiplied by the 50-connection cap that is 800 MiB
/// of committed, attacker-paced memory from idle sockets.
///
/// 1 MiB is comfortably above every legitimate packet and cuts the
/// amplification 16×. [`max_frame_size_for`] applies a far tighter per-type
/// limit still.
pub const MAX_FRAME_SIZE: u32 = 1024 * 1024;

/// Maximum text message size: 64 KiB.
pub const MAX_TEXT_MESSAGE_SIZE: usize = 64 * 1024;

/// Maximum file chunk size: 256 KiB.
pub const MAX_FILE_CHUNK_SIZE: usize = 256 * 1024;

/// Maximum file transfer size accepted from (or offered to) a peer: 2 GiB.
/// Peer-declared sizes above this are rejected before any allocation or
/// disk pre-allocation happens.
pub const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// Maximum number of chunks in an incoming transfer. With the smallest
/// adaptive chunk size (128 KiB) this covers exactly MAX_FILE_SIZE, and
/// bounds the receive bitmask to 16 KiB per transfer.
pub const MAX_TOTAL_CHUNKS: u32 = 16 * 1024;

/// Minimum frame size: version (1) + at least 1 byte payload type.
pub const MIN_FRAME_SIZE: u32 = 2;

/// Length prefix size in bytes.
pub const LENGTH_PREFIX_SIZE: usize = 4;

/// Heartbeat interval in seconds.
/// A heartbeat is sent every interval to keep the connection alive
/// and detect silent disconnections. Heartbeats are ENCRYPTED at the
/// session layer (AEAD via `Session::send_heartbeat`) — plaintext
/// keepalives were a free liveness oracle for observers.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Heartbeat timeout in seconds.
/// If no HeartbeatAck is received within this time, the connection
/// is considered dead and will be cleaned up.
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 10;

/// Maximum session duration in seconds (24 hours).
pub const MAX_SESSION_DURATION_SECS: u64 = 24 * 60 * 60;

/// File transfer protocol version (v0x02 — adds per-chunk hashes, ACKs, cancel).
pub const PROTOCOL_FILE_TRANSFER_VERSION: u8 = 0x02;

/// Maximum invite validity duration in seconds (24 hours).
pub const MAX_INVITE_VALIDITY_SECS: u64 = 24 * 60 * 60;

/// Clock skew tolerance for invite validation (5 minutes).
pub const CLOCK_SKEW_TOLERANCE_SECS: u64 = 5 * 60;

/// Maximum invite string length (4096 bytes accommodates X3DH prekey bundle
/// + candidates + relay info after base64url encoding).
pub const MAX_INVITE_LENGTH: usize = 4096;

/// Maximum address hint length.
pub const MAX_ADDRESS_HINT_LENGTH: usize = 256;

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
    X3DHHandshakeInit = 0x04,
    X3DHHandshakeResponse = 0x05,
    X3DHComplete = 0x06,
    EncryptedMessage = 0x10,
    FileTransferRequest = 0x11,
    FileTransferChunk = 0x12,
    FileTransferComplete = 0x13,
    FileTransferAccept = 0x14,
    FileTransferReject = 0x15,
    FileTransferChunkAck = 0x16,
    FileTransferCancel = 0x17,
    Heartbeat = 0x20,
    HeartbeatAck = 0x21,
    Disconnect = 0x30,
    Error = 0x31,
    ConversationMeta = 0x40,
    MessageReaction = 0x41,
    MessageEdit = 0x42,
    MessageDelete = 0x43,
    /// Request to sync missed messages after reconnect.
    /// The reconnecting peer sends its most recent received timestamp;
    /// the peer responds by re-sending all messages after that timestamp.
    SyncRequest = 0x44,
    /// Multi-device sync: device identity exchange.
    /// Sent after X3DH handshake during device pairing.
    SyncDeviceInfo = 0x45,
    /// Multi-device sync: encrypted payload batch (peer keys, conversations, etc.).
    SyncPayload = 0x46,
    // ─── Group Chat (Phase 3) ───
    /// Create a new group (0x50).
    /// Sent by the group creator to all initial members over their 1:1 DR session.
    GroupCreate = 0x50,
    /// Invite a new member to an existing group (0x51).
    /// Sent by an admin to the invited member.
    GroupInvite = 0x51,
    /// Remove a member from a group (0x52).
    /// Sent by an admin to all remaining members (triggers key rotation).
    GroupRemove = 0x52,
    /// Distribute a Sender Key bundle to a group member (0x53).
    /// Sent over the recipient's 1:1 DR session during group creation or member add.
    GroupSenderKey = 0x53,
    /// An encrypted group message (0x54).
    /// Inner: Sender Key encrypted payload. Outer: pairwise DR envelope.
    GroupEncryptedMessage = 0x54,
    /// Group metadata update (0x55), e.g. name change.
    GroupInfo = 0x55,
    /// Member leaves a group voluntarily (0x56).
    GroupLeave = 0x56,
    // ─── Typing Indicators ───
    /// Notify peer that the user is typing (0x60).
    /// No payload. Sent periodically while user is typing.
    TypingIndicator = 0x60,
    /// Notify peer that the user stopped typing (0x61).
    /// Sent when user clears input or stops typing for 3 seconds.
    TypingIndicatorClear = 0x61,
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
            0x16 => Ok(PacketType::FileTransferChunkAck),
            0x17 => Ok(PacketType::FileTransferCancel),
            0x20 => Ok(PacketType::Heartbeat),
            0x21 => Ok(PacketType::HeartbeatAck),
            0x30 => Ok(PacketType::Disconnect),
            0x31 => Ok(PacketType::Error),
            0x40 => Ok(PacketType::ConversationMeta),
            0x41 => Ok(PacketType::MessageReaction),
            0x42 => Ok(PacketType::MessageEdit),
            0x43 => Ok(PacketType::MessageDelete),
            0x44 => Ok(PacketType::SyncRequest),
            0x45 => Ok(PacketType::SyncDeviceInfo),
            0x46 => Ok(PacketType::SyncPayload),
            // Group chat (Phase 3)
            0x50 => Ok(PacketType::GroupCreate),
            0x51 => Ok(PacketType::GroupInvite),
            0x52 => Ok(PacketType::GroupRemove),
            0x53 => Ok(PacketType::GroupSenderKey),
            0x54 => Ok(PacketType::GroupEncryptedMessage),
            0x55 => Ok(PacketType::GroupInfo),
            0x56 => Ok(PacketType::GroupLeave),
            0x60 => Ok(PacketType::TypingIndicator),
            0x61 => Ok(PacketType::TypingIndicatorClear),
            other => Err(ProtocolError::UnknownPacketType(other)),
        }
    }

    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Validate a protocol version byte.
///
/// Accepts the current version only ([`PROTOCOL_VERSION`], 0x03). There is no
/// downgrade path: 0x01 (pre-X3DH) and 0x02 (different AEAD AAD) are both
/// rejected. Reserved versions (0x00, 0xFE, 0xFF) are always rejected.
pub fn validate_version(version: u8) -> Result<(), ProtocolError> {
    if RESERVED_VERSIONS.contains(&version) {
        return Err(ProtocolError::ReservedVersion(version));
    }
    // No downgrade path.
    //
    // This used to `return Ok(())` for `PROTOCOL_VERSION_LEGACY` (0x01) after a
    // `tracing::warn!`, which contradicted the stated design directly: a 5.0.0
    // client *would* complete a handshake with a 4.x peer, and a `warn!` is
    // invisible to the user. It is also not merely a cosmetic mismatch — the
    // legacy branch selects `handshake_as_initiator`, the non-X3DH path, so a
    // peer that omitted the prekey bundle from its invite could force a 5.0.0
    // initiator onto a handshake with no forward secrecy.
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(version));
    }
    Ok(())
}

/// Validate a frame size against the global [`MAX_FRAME_SIZE`] ceiling.
///
/// This is the cheap first gate, applied immediately after the 4-byte length
/// prefix and *before* any allocation. Callers that know the packet type
/// should follow it with [`validate_frame_size_for`], which is far tighter.
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

/// Maximum frame size permitted for a given packet type.
///
/// Binding a *type* to a frame lets the reader reject an over-large
/// declaration using only the 2-byte header, before the body buffer is
/// allocated. A heartbeat is a few dozen bytes; accepting 1 MiB "because
/// file chunks are large" for every packet type is what made the pre-fix
/// allocation-amplification possible.
///
/// The allowances add room for MessagePack framing overhead, AEAD tag +
/// nonce, and `pad_message_variable` doubling the plaintext.
pub fn max_frame_size_for(packet_type: PacketType) -> u32 {
    // +2 for the version and type bytes themselves.
    let hdr = MIN_FRAME_SIZE;
    match packet_type {
        // Heartbeats and their acks are tiny.
        PacketType::Heartbeat | PacketType::HeartbeatAck => 4 * 1024,
        // Control frames: disconnect reasons, typing flags, error text.
        PacketType::Disconnect
        | PacketType::Error
        | PacketType::TypingIndicator
        | PacketType::TypingIndicatorClear
        | PacketType::MessageReaction
        | PacketType::MessageDelete => 8 * 1024,
        // Text messages: 64 KiB plaintext, doubling under worst-case padding.
        PacketType::EncryptedMessage | PacketType::MessageEdit | PacketType::ConversationMeta => {
            (MAX_TEXT_MESSAGE_SIZE * 2 + 8 * 1024) as u32 + hdr
        }
        // Handshakes carry a signed prekey bundle plus an ICE candidate list,
        // which is what the 4 KiB figure people remember from the removed
        // `MAX_HANDSHAKE_SIZE` constant did NOT bound once candidates are
        // counted. That constant was declared, documented, and never used.
        PacketType::HandshakeInit
        | PacketType::HandshakeResponse
        | PacketType::HandshakeComplete
        | PacketType::X3DHHandshakeInit
        | PacketType::X3DHHandshakeResponse
        | PacketType::X3DHComplete => 256 * 1024,
        // File transfer: a 256 KiB chunk plus headers, hashes and signature.
        PacketType::FileTransferRequest
        | PacketType::FileTransferChunk
        | PacketType::FileTransferComplete
        | PacketType::FileTransferAccept
        | PacketType::FileTransferReject
        | PacketType::FileTransferChunkAck
        | PacketType::FileTransferCancel => (MAX_FILE_CHUNK_SIZE + 64 * 1024) as u32 + hdr,
        // Group control frames carry a member roster and a sender-key bundle
        // with one verification key per member.
        PacketType::GroupCreate
        | PacketType::GroupInvite
        | PacketType::GroupRemove
        | PacketType::GroupSenderKey
        | PacketType::GroupInfo
        | PacketType::GroupLeave => 512 * 1024,
        // A single group message is a padded text message.
        PacketType::GroupEncryptedMessage => (MAX_TEXT_MESSAGE_SIZE * 2 + 8 * 1024) as u32 + hdr,
        // Sync carries a conversation list; bounded well below the global cap
        // so a peer cannot use it to force a large allocation.
        PacketType::SyncRequest | PacketType::SyncDeviceInfo | PacketType::SyncPayload => {
            256 * 1024
        }
    }
}

/// Validate a declared frame size against the limit for its packet type.
///
/// Applied after the 2-byte header is read but before the body buffer is
/// allocated, so an attacker cannot make the client reserve memory by
/// declaring an enormous frame of a type that has no legitimate use for one.
pub fn validate_frame_size_for(size: u32, packet_type: PacketType) -> Result<(), ProtocolError> {
    // The global bounds still apply.
    validate_frame_size(size)?;
    let max = max_frame_size_for(packet_type);
    if size > max {
        return Err(ProtocolError::FrameTooLarge { size, max });
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
    /// NOTE: no `skip_serializing_if` — rmp-serde uses positional encoding.
    #[serde(default)]
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
    /// True when the invite that produced this handshake was marked one-time,
    /// i.e. the initiator asserts it must never be replayed.
    ///
    /// Carried on the wire and covered by the signature, because the responder
    /// has no other way to learn it: the flag lives in the invite payload, which
    /// only the initiator holds. Without it the responder cannot tell an
    /// ordinary reusable invite (which legitimately carries a prekey and must
    /// keep working for a second recipient) from a spent one-time invite that
    /// is being replayed.
    ///
    /// `#[serde(default)]` keeps this wire-compatible with a peer that omits it,
    /// which is treated as "not one-time" — fail-open on the flag, but the
    /// one-time prekey itself is still consumed exactly once either way.
    #[serde(default)]
    pub one_time: bool,
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
    /// NOTE: no `skip_serializing_if` — rmp-serde uses positional encoding and
    /// skipping a field shifts subsequent elements, breaking deserialization.
    #[serde(default)]
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
    /// NOTE: no `skip_serializing_if` — rmp-serde uses positional encoding and
    /// skipping a field shifts subsequent elements, breaking deserialization.
    #[serde(default)]
    pub dr_header: Option<DRHeader>,
}

// --- Inner Message Types (decrypted content) ---

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize)]
#[serde(tag = "type")]
pub enum MessageBody {
    #[serde(rename = "text")]
    Text {
        id: String,
        content: String,
        #[serde(default)]
        disappear_after: Option<u64>,
        /// Unix timestamp when the message was created (sender's clock).
        /// Defaults to 0 for backward-compatible deserialization with older clients.
        #[serde(default)]
        timestamp: u64,
    },
    #[serde(rename = "ack")]
    Ack { id: String },
}

impl Drop for MessageBody {
    fn drop(&mut self) {
        self.zeroize();
    }
}

// --- File Transfer Messages ---

/// Validate peer-declared transfer parameters before any allocation.
///
/// Rejects oversized files, absurd chunk counts, and inconsistent
/// size/chunk combinations. Returns the inferred chunk stride
/// (sender's chunk size) on success.
pub fn validate_transfer_request(total_size: u64, total_chunks: u32) -> Result<u64, &'static str> {
    if total_size > MAX_FILE_SIZE {
        return Err("file exceeds maximum transfer size");
    }
    if total_chunks > MAX_TOTAL_CHUNKS {
        return Err("too many chunks");
    }
    if total_size == 0 || total_chunks == 0 {
        // Empty transfers are pointless and a DoS nuisance; reject them.
        return Err("empty file transfer");
    }
    let stride = total_size.div_ceil(total_chunks as u64);
    if stride > MAX_FILE_CHUNK_SIZE as u64 {
        return Err("declared chunks too few for declared size");
    }
    Ok(stride)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferRequestData {
    pub transfer_id: String,
    pub filename: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub file_hash: Vec<u8>,
    /// Per-chunk SHA-256 hashes (v2 protocol). Empty for backward compat with v1 senders.
    /// When present, the receiver verifies each chunk hash before writing to disk.
    #[serde(default)]
    pub chunk_hashes: Vec<Vec<u8>>,
    /// File transfer protocol version (0x01 = legacy, 0x02 = chunk hashes + ACKs + cancel).
    #[serde(default)]
    pub file_transfer_version: u8,
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

/// Chunk acknowledgement (type 0x16). Receiver confirms a single chunk was
/// received, hash-verified, and written to disk. The sender uses this to
/// track which chunks are safe from retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferChunkAckData {
    pub transfer_id: String,
    pub chunk_index: u32,
}

/// Cancel an in-progress file transfer (type 0x17). Either side can send this.
/// The receiver stops accepting chunks and cleans up the temp file.
/// The sender stops sending and marks the transfer as cancelled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferCancelData {
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

// --- Message Reactions ---

/// A reaction (emoji) on a message, sent as a typed encrypted frame (type 0x41).
/// The peer_key_hex is implicit from the session — not serialized in the packet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReactionData {
    /// The message ID being reacted to.
    pub message_id: String,
    /// The emoji reaction (e.g. "👍", "❤️", "😂").
    pub reaction: String,
    /// Whether this is an add (false) or remove (true).
    #[serde(default)]
    pub remove: bool,
}

// --- Message Edit (0x42) ---

/// An edit to a previously-sent message, sent as a typed encrypted frame (type 0x42).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEditData {
    /// The ID of the message being edited.
    pub message_id: String,
    /// The new content replacing the original.
    pub new_content: String,
    /// Server timestamp of the edit (unix seconds).
    pub edited_at: u64,
}

// --- Message Delete (0x43) ---

/// A request to delete a previously-sent message, sent as a typed encrypted frame (type 0x43).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeleteData {
    /// The ID of the message being deleted.
    pub message_id: String,
}

// --- Sync Request (0x44) ---

/// Sent after reconnection to request missed messages.
/// The reconnecting peer sends the most recent *received* message timestamp
/// it has for this conversation. The peer responds by re-sending all
/// messages with timestamp greater than this value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequestData {
    /// Only send messages with timestamp > this value.
    pub since_timestamp: u64,
}

// --- Sync Device Info (0x45) ---

/// Exchanged after X3DH handshake during device pairing.
/// Identifies the connecting device and its display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDeviceInfo {
    /// Unique device identifier (UUID v4).
    pub device_id: String,
    /// Human-readable device name (e.g., "Laptop", "Phone").
    pub device_name: String,
    /// Sync protocol version (start at 1).
    pub sync_protocol_version: u8,
    /// The one-time sync invite token issued by the primary device, proving
    /// the sender was actually invited.
    ///
    /// ## Why this field exists
    ///
    /// Pairing was previously unconditional: any peer that completed an X3DH
    /// handshake — which, with `require_known_contact` off by default, is any
    /// Ed25519 identity on the internet — could send this frame and be
    /// recorded as a synced device, after which the primary immediately
    /// broadcast its full conversation metadata (the complete contact graph,
    /// including people the user had never spoken to). The token check the
    /// module documentation describes was never implemented.
    ///
    /// `#[serde(default)]` so a pre-existing frame still parses; the
    /// resulting empty token is rejected by the handler, which is the correct
    /// fail-closed outcome.
    #[serde(default)]
    pub sync_token: String,
}

// --- Sync Payload (0x46) ---

/// An encrypted batch of sync data sent over an established sync session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPayload {
    /// What kind of data this payload contains.
    pub payload_type: SyncPayloadType,
    /// Serialized payload data (encrypted at the frame level by DR).
    pub data: Vec<u8>,
}

// --- Group Chat (Phase 3) ---

/// Create a new group (packet 0x50).
/// Sent by the group creator to every initial member over their pairwise DR session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupCreateData {
    /// UUID v4 identifying the group.
    pub group_id: String,
    /// Human-readable group name.
    pub group_name: String,
    /// Creator's Ed25519 public key (hex-encoded).
    pub creator_peer_key_hex: String,
    /// When the group was created (unix seconds, sender's clock).
    pub created_at: u64,
    /// Ed25519 public keys of all initial members (excluding creator).
    pub initial_members: Vec<String>,
}

/// Invite a new member to an existing group (packet 0x51).
/// Sent over the invitee's pairwise DR session by an admin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInviteData {
    pub group_id: String,
    pub group_name: String,
    /// Inviter's Ed25519 public key (hex).
    pub inviter_peer_key_hex: String,
    /// Current number of members in the group.
    pub member_count: u32,
    /// All current members' Ed25519 public keys (hex).
    pub existing_members: Vec<String>,
    /// Ed25519 signature over (group_id || member_count) by the inviter.
    pub signature: Vec<u8>,
}

/// Remove a member from a group (packet 0x52).
/// Sent by an admin to ALL remaining members (triggers key rotation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupRemoveData {
    pub group_id: String,
    /// The member being removed (Ed25519 public key hex).
    pub removed_peer_key_hex: String,
    /// The admin who initiated the removal.
    pub removed_by_peer_key_hex: String,
    /// New Sender Key bundles for all remaining members (excluding the removed one).
    /// Each member gets their own bundle sent over their personal DR session.
    /// This field is populated in the packet to the specific recipient only.
    pub new_sender_key: Option<GroupSenderKeyData>,
}

/// Distribute a Sender Key bundle to a group member (packet 0x53).
/// Sent over the recipient's 1:1 DR session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSenderKeyData {
    pub group_id: String,
    /// The peer whose sender key this is (Ed25519 public key hex).
    pub sender_peer_key_hex: String,
    /// Initial chain key (32 bytes, HKDF-derived) for this sender.
    pub chain_key: [u8; 32],
    /// Starting message number (usually 0).
    pub message_number: u64,
    /// Ed25519 signing key (64 bytes: seed + secret). Only the recipient of THIS bundle gets this.
    /// All other members get the verification_key instead.
    /// The signing key recipient can now send messages as this sender.
    #[serde(default)]
    pub signing_key: Option<Vec<u8>>,
    /// Ed25519 verification key (32 bytes). Everyone except the signing key recipient gets this.
    pub verification_key: [u8; 32],
    /// Ed25519 signature over (group_id || sender_peer_key_hex || chain_key).
    /// Binds the sender key to the group and sender identity.
    pub signature: Vec<u8>,
}

/// An encrypted group message (packet 0x54).
/// Inner payload is encrypted with the sender's Sender Key chain.
/// The outer frame is encrypted with the pairwise DR session (standard envelope).
/// This means the same inner payload is sent N times (once per online member).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupEncryptedMessageData {
    pub group_id: String,
    /// The sender's Ed25519 public key (hex).
    pub sender_peer_key_hex: String,
    /// Message number in the sender's chain (for key derivation and replay protection).
    pub message_number: u64,
    /// XChaCha20-Poly1305 ciphertext (includes padded plaintext).
    pub ciphertext: Vec<u8>,
    /// Nonce for the XChaCha20-Poly1305 encryption.
    pub nonce: Vec<u8>,
    /// Ed25519 signature over (group_id || message_number || nonce || ciphertext).
    pub signature: Vec<u8>,
}

/// Group metadata update (packet 0x55).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfoData {
    pub group_id: String,
    /// New group name (None = no change).
    pub new_name: Option<String>,
    /// Who made the change (Ed25519 public key hex).
    pub changed_by_peer_key_hex: String,
}

/// Member leaves a group voluntarily (packet 0x56).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupLeaveData {
    pub group_id: String,
    /// The leaving member's Ed25519 public key (hex).
    pub leaving_peer_key_hex: String,
}

/// The type of data contained in a SyncPayload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncPayloadType {
    /// Peer public keys for the KeyStore.
    PeerKeys,
    /// Conversation metadata (display names, last_message_at, retention).
    Conversations,
    /// Per-conversation unread counts.
    UnreadCounts,
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
            (0x16, PacketType::FileTransferChunkAck),
            (0x17, PacketType::FileTransferCancel),
            (0x20, PacketType::Heartbeat),
            (0x21, PacketType::HeartbeatAck),
            (0x30, PacketType::Disconnect),
            (0x31, PacketType::Error),
            (0x40, PacketType::ConversationMeta),
            (0x41, PacketType::MessageReaction),
            (0x42, PacketType::MessageEdit),
            (0x43, PacketType::MessageDelete),
            (0x44, PacketType::SyncRequest),
            (0x45, PacketType::SyncDeviceInfo),
            (0x46, PacketType::SyncPayload),
            (0x50, PacketType::GroupCreate),
            (0x51, PacketType::GroupInvite),
            (0x52, PacketType::GroupRemove),
            (0x53, PacketType::GroupSenderKey),
            (0x54, PacketType::GroupEncryptedMessage),
            (0x55, PacketType::GroupInfo),
            (0x56, PacketType::GroupLeave),
        ];
        for &(byte, expected) in valid {
            let parsed = PacketType::from_byte(byte).unwrap();
            assert_eq!(parsed, expected, "from_byte(0x{byte:02X}) failed");
            assert_eq!(
                parsed.to_byte(),
                byte,
                "to_byte() roundtrip failed for 0x{byte:02X}"
            );
        }
    }

    #[test]
    fn test_unknown_packet_type_rejected() {
        let invalid_bytes: &[u8] = &[0x00, 0x0F, 0x18, 0x22, 0x32, 0x47, 0x57, 0xFF];
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
        assert!(matches!(
            validate_version(0x00),
            Err(ProtocolError::ReservedVersion(0x00))
        ));
        assert!(matches!(
            validate_version(0xFE),
            Err(ProtocolError::ReservedVersion(0xFE))
        ));
        assert!(matches!(
            validate_version(0xFF),
            Err(ProtocolError::ReservedVersion(0xFF))
        ));
    }

    #[test]
    fn test_unsupported_version_rejected() {
        // Anything that's not reserved and not a known version
        assert!(matches!(
            validate_version(0x10),
            Err(ProtocolError::UnsupportedVersion(0x10))
        ));
        // 0x03 is the current version, so it is accepted, not rejected.
        assert!(validate_version(PROTOCOL_VERSION).is_ok());
        assert!(matches!(
            validate_version(0xFD),
            Err(ProtocolError::UnsupportedVersion(0xFD))
        ));
    }

    /// v0x02 must be REJECTED, not accepted.
    ///
    /// v0x02 computed the Double Ratchet AAD as just `context`; v0x03 folds the
    /// DR header in. A v0x02 peer would pass a permissive version check and
    /// then fail to decrypt every message with no useful error — so it is
    /// excluded at the handshake instead, where the user gets a clear
    /// "upgrade" signal.
    #[test]
    fn test_v02_is_rejected_because_its_aad_differs() {
        assert_eq!(PROTOCOL_VERSION, 0x03);
        // v0x02 must not be accepted: its AEAD associated data differs.
        assert!(matches!(
            validate_version(0x02),
            Err(ProtocolError::UnsupportedVersion(0x02))
        ));
    }

    #[test]
    fn test_legacy_version_is_rejected() {
        // 0x01 is the pre-X3DH version: SHA-256 KDF ratchet, no one-time prekey,
        // and therefore no forward secrecy against a peer whose long-term key
        // is later compromised. Accepting it is exactly the downgrade the
        // project documents as forbidden, so it is refused at the handshake
        // where the user gets a clear "upgrade" signal instead of a session
        // that silently has weaker guarantees.
        assert!(matches!(
            validate_version(PROTOCOL_VERSION_LEGACY),
            Err(ProtocolError::UnsupportedVersion(0x01))
        ));
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
            Err(ProtocolError::FrameTooSmall {
                size: 0,
                min: MIN_FRAME_SIZE
            })
        ));
    }

    #[test]
    fn test_frame_size_overflow_rejected() {
        assert!(matches!(
            validate_frame_size(u32::MAX),
            Err(ProtocolError::FrameTooLarge { .. })
        ));
    }

    // ─── Per-type frame size limits ─────────────────────────────
    //
    // The point of these caps is that the reader learns the packet type from a
    // 2-byte header and can therefore reject an over-large declaration BEFORE
    // allocating the body. Without them, a 4-byte length prefix alone forced a
    // 16 MiB allocation that an attacker could hold open by trickling bytes.

    #[test]
    fn test_per_type_limits_are_tighter_than_global() {
        // The heavy types may legitimately approach the global ceiling...
        assert!(max_frame_size_for(PacketType::FileTransferChunk) > 256 * 1024);
        // ...but everything else must be far below it.
        for pt in [
            PacketType::Heartbeat,
            PacketType::HeartbeatAck,
            PacketType::Disconnect,
            PacketType::Error,
            PacketType::TypingIndicator,
            PacketType::TypingIndicatorClear,
        ] {
            assert!(
                max_frame_size_for(pt) <= 16 * 1024,
                "{pt:?} should be capped far below the global ceiling, got {}",
                max_frame_size_for(pt)
            );
        }
    }

    #[test]
    fn test_oversized_heartbeat_rejected_by_type_cap() {
        // A heartbeat is ~30 bytes. A 900 KiB one is an attack, and would be
        // under the global 1 MiB ceiling, so only the per-type cap catches it.
        let declared = 900 * 1024;
        assert!(
            validate_frame_size(declared).is_ok(),
            "under the global ceiling, so the cheap first gate passes"
        );
        assert!(
            validate_frame_size_for(declared, PacketType::Heartbeat).is_err(),
            "the per-type cap must reject it before allocation"
        );
    }

    #[test]
    fn test_per_type_cap_still_applies_global_bounds() {
        // Below MIN_FRAME_SIZE is rejected regardless of type.
        assert!(validate_frame_size_for(0, PacketType::Heartbeat).is_err());
        assert!(validate_frame_size_for(1, PacketType::Heartbeat).is_err());
        // Above the global ceiling is rejected regardless of type.
        assert!(validate_frame_size_for(u32::MAX, PacketType::FileTransferChunk).is_err());
    }

    #[test]
    fn test_every_packet_type_has_a_sane_limit() {
        // Guards against a new PacketType silently getting a generous cap:
        // every type must be at least large enough for a minimal frame.
        for byte in 0u8..=0xFF {
            if let Ok(pt) = PacketType::from_byte(byte) {
                let cap = max_frame_size_for(pt);
                assert!(
                    cap >= MIN_FRAME_SIZE,
                    "{pt:?} cap {cap} is below the minimum frame size"
                );
                assert!(
                    cap <= MAX_FRAME_SIZE,
                    "{pt:?} cap {cap} exceeds the global ceiling {MAX_FRAME_SIZE}"
                );
            }
        }
    }

    #[test]
    fn test_file_chunk_and_text_caps_accommodate_real_payloads() {
        // A maximum-size text message plus padding must fit its frame.
        let text_cap = max_frame_size_for(PacketType::EncryptedMessage);
        assert!(
            text_cap as usize >= MAX_TEXT_MESSAGE_SIZE * 2 + 64,
            "text frame cap {text_cap} too small for a padded {MAX_TEXT_MESSAGE_SIZE}-byte message"
        );
        // A maximum-size file chunk must fit its frame.
        let chunk_cap = max_frame_size_for(PacketType::FileTransferChunk) as usize;
        assert!(
            chunk_cap >= MAX_FILE_CHUNK_SIZE + 1024,
            "chunk frame cap {chunk_cap} too small for a {MAX_FILE_CHUNK_SIZE}-byte chunk"
        );
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
            PacketType::FileTransferChunkAck,
            PacketType::FileTransferCancel,
            PacketType::Heartbeat,
            PacketType::HeartbeatAck,
            PacketType::Disconnect,
            PacketType::Error,
            PacketType::ConversationMeta,
            PacketType::MessageReaction,
            PacketType::MessageEdit,
            PacketType::MessageDelete,
            PacketType::SyncRequest,
            PacketType::SyncDeviceInfo,
            PacketType::SyncPayload,
            PacketType::GroupCreate,
            PacketType::GroupInvite,
            PacketType::GroupRemove,
            PacketType::GroupSenderKey,
            PacketType::GroupEncryptedMessage,
            PacketType::GroupInfo,
            PacketType::GroupLeave,
        ];
        for pt in types {
            let frame = build_frame(pt, b"test");
            assert!(frame.is_ok(), "build_frame failed for {:?}", pt);
        }
    }

    #[test]
    fn test_group_create_data_roundtrip() {
        let data = GroupCreateData {
            group_id: "test-group-uuid".to_string(),
            group_name: "Test Group".to_string(),
            creator_peer_key_hex: "aabb".to_string(),
            created_at: 1719000000,
            initial_members: vec!["ccdd".to_string(), "eeff".to_string()],
        };
        let bytes = serialize(&data).unwrap();
        let decoded: GroupCreateData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.group_id, "test-group-uuid");
        assert_eq!(decoded.group_name, "Test Group");
        assert_eq!(decoded.initial_members.len(), 2);
    }

    #[test]
    fn test_group_sender_key_data_roundtrip() {
        let data = GroupSenderKeyData {
            group_id: "gid".to_string(),
            sender_peer_key_hex: "alice".to_string(),
            chain_key: [0xAA; 32],
            message_number: 0,
            signing_key: Some(vec![0xBB; 64]),
            verification_key: [0xCC; 32],
            signature: vec![0xDD; 64],
        };
        let bytes = serialize(&data).unwrap();
        let decoded: GroupSenderKeyData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.group_id, "gid");
        assert!(decoded.signing_key.is_some());
        assert_eq!(decoded.verification_key, [0xCC; 32]);
    }

    #[test]
    fn test_group_encrypted_message_data_roundtrip() {
        let data = GroupEncryptedMessageData {
            group_id: "gid".to_string(),
            sender_peer_key_hex: "alice".to_string(),
            message_number: 0,
            ciphertext: vec![0xEE; 64],
            nonce: vec![0xFF; 24],
            signature: vec![0xDD; 64],
        };
        let bytes = serialize(&data).unwrap();
        let decoded: GroupEncryptedMessageData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.group_id, "gid");
        assert_eq!(decoded.message_number, 0);
        assert_eq!(decoded.ciphertext.len(), 64);
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
            disappear_after: None,
            timestamp: 1719000000,
        };
        let bytes = serialize(&body).unwrap();
        let decoded: MessageBody = deserialize(&bytes).unwrap();
        match &decoded {
            MessageBody::Text { id, content, .. } => {
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
            chunk_hashes: Vec::new(),
            file_transfer_version: 0,
        };
        let bytes = serialize(&req).unwrap();
        let decoded: FileTransferRequestData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.transfer_id, "xfer-001");
        assert_eq!(decoded.filename, "document.pdf");
        assert_eq!(decoded.total_size, 1_048_576);
        assert_eq!(decoded.total_chunks, 16);
        assert_eq!(decoded.file_hash.len(), 32);
        // New fields should default to empty/0 for backward compat
        assert!(decoded.chunk_hashes.is_empty());
        assert_eq!(decoded.file_transfer_version, 0);
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
    fn test_serialize_deserialize_file_transfer_chunk_ack() {
        let ack = FileTransferChunkAckData {
            transfer_id: "xfer-001".to_string(),
            chunk_index: 42,
        };
        let bytes = serialize(&ack).unwrap();
        let decoded: FileTransferChunkAckData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.transfer_id, "xfer-001");
        assert_eq!(decoded.chunk_index, 42);
    }

    #[test]
    fn test_serialize_deserialize_file_transfer_cancel() {
        let cancel = FileTransferCancelData {
            transfer_id: "xfer-001".to_string(),
        };
        let bytes = serialize(&cancel).unwrap();
        let decoded: FileTransferCancelData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.transfer_id, "xfer-001");
    }

    #[test]
    fn test_serialize_deserialize_file_transfer_request_v2() {
        let req = FileTransferRequestData {
            transfer_id: "xfer-v2-001".to_string(),
            filename: "large_file.iso".to_string(),
            total_size: 4_294_967_296,
            total_chunks: 16384,
            file_hash: vec![0xCC; 32],
            chunk_hashes: vec![vec![0xDD; 32]; 16384],
            file_transfer_version: 2,
        };
        let bytes = serialize(&req).unwrap();
        let decoded: FileTransferRequestData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.transfer_id, "xfer-v2-001");
        assert_eq!(decoded.chunk_hashes.len(), 16384);
        assert_eq!(decoded.chunk_hashes[0], vec![0xDD; 32]);
        assert_eq!(decoded.file_transfer_version, 2);
        assert_eq!(decoded.total_size, 4_294_967_296);
    }

    #[test]
    fn test_deserialize_v2_file_transfer_request_as_v1_client() {
        // Old clients without the new fields should still be able to
        // deserialize v2 requests — unknown fields are ignored by serde.
        let req = FileTransferRequestData {
            transfer_id: "xfer-v2-001".to_string(),
            filename: "large_file.iso".to_string(),
            total_size: 1_048_576,
            total_chunks: 16,
            file_hash: vec![0xCC; 32],
            chunk_hashes: vec![vec![0xDD; 32]; 16],
            file_transfer_version: 2,
        };
        let bytes = serialize(&req).unwrap();
        // Deserialize into a struct that has the v1 fields only
        // (via serde_json to simulate different schema — MessagePack ignores unknown)
        let decoded: FileTransferRequestData = deserialize(&bytes).unwrap();
        assert_eq!(decoded.transfer_id, "xfer-v2-001");
        // Old client ignores extra fields — they have defaults
        assert_eq!(decoded.file_transfer_version, 2);
        assert_eq!(decoded.chunk_hashes.len(), 16);
    }

    #[test]
    fn test_deserialize_v1_file_transfer_request_as_v2_client() {
        // A v2 client receiving a v1 request gets empty defaults for new fields
        let v1_req_bytes = {
            // Simulate v1 serialization: only the original fields
            #[derive(Serialize)]
            struct V1Request {
                transfer_id: String,
                filename: String,
                total_size: u64,
                total_chunks: u32,
                file_hash: Vec<u8>,
            }
            let v1 = V1Request {
                transfer_id: "v1-xfer".to_string(),
                filename: "doc.pdf".to_string(),
                total_size: 65536,
                total_chunks: 1,
                file_hash: vec![0xAB; 32],
            };
            serialize(&v1).unwrap()
        };

        let decoded: FileTransferRequestData = deserialize(&v1_req_bytes).unwrap();
        assert_eq!(decoded.transfer_id, "v1-xfer");
        assert_eq!(decoded.total_chunks, 1);
        // New fields get defaults
        assert!(
            decoded.chunk_hashes.is_empty(),
            "v1 request should have no chunk_hashes"
        );
        assert_eq!(
            decoded.file_transfer_version, 0,
            "v1 request should have version 0"
        );
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
            one_time: false,
            timestamp: 1719446400,
            signature: vec![0xCC; 64],
            candidates: vec![
                WireCandidate {
                    address: "192.168.1.5:12345".to_string(),
                    candidate_type: 0,
                    relay_id: None,
                },
                WireCandidate {
                    address: "1.2.3.4:54321".to_string(),
                    candidate_type: 1,
                    relay_id: None,
                },
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
            one_time: false,
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
    #[allow(clippy::assertions_on_constants)]
    fn test_protocol_constants_sane() {
        assert!(MAX_FRAME_SIZE >= MIN_FRAME_SIZE);
        assert!(MAX_TEXT_MESSAGE_SIZE < MAX_FRAME_SIZE as usize);
        assert!(MAX_FILE_CHUNK_SIZE < MAX_FRAME_SIZE as usize);
        assert!(MAX_SESSION_DURATION_SECS > 0);
        assert!(MAX_INVITE_VALIDITY_SECS > 0);
        assert!(CLOCK_SKEW_TOLERANCE_SECS > 0);
        assert_eq!(LENGTH_PREFIX_SIZE, 4);
    }
}
