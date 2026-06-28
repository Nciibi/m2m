//! Fuzz target for protocol frame parsing.
//!
//! Feeds random byte sequences into frame validation functions to detect
//! panics, crashes, or excessive resource consumption.
//!
//! Does NOT use network::read_frame_impl (which requires async I/O).
//! Instead, it exercises the synchronous validation logic directly:
//! - validate_frame_size (bounds checking)
//! - PacketType::from_byte (packet type parsing)
//! - validate_version (version checking)
//! - build_frame (frame construction with arbitrary body)

#![no_main]

use libfuzzer_sys::fuzz_target;
use m2m::protocol::{self, PacketType};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // ── 1. Frame size validation ──
    // Interpret first 4 bytes as a u32 frame size
    if data.len() >= 4 {
        let size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let _ = protocol::validate_frame_size(size);
    }

    // ── 2. Packet type parsing ──
    // Try every byte as a packet type
    if data.len() >= 5 {
        let pt_byte = data[4];
        let _ = PacketType::from_byte(pt_byte);
    }

    // ── 3. Version validation ──
    if data.len() >= 6 {
        let version = data[5];
        let _ = protocol::validate_version(version);
    }

    // ── 4. Frame construction ──
    // Use the body as a frame payload with a valid packet type
    // (build_frame only validates size, not content)
    let body = if data.len() > 6 { &data[6..] } else { &[0u8; 1] };
    let pt = match data[0] % 15 {
        0 => PacketType::HandshakeInit,
        1 => PacketType::HandshakeResponse,
        2 => PacketType::HandshakeComplete,
        3 => PacketType::EncryptedMessage,
        4 => PacketType::FileTransferRequest,
        5 => PacketType::FileTransferChunk,
        6 => PacketType::FileTransferComplete,
        7 => PacketType::FileTransferAccept,
        8 => PacketType::FileTransferReject,
        9 => PacketType::Heartbeat,
        10 => PacketType::HeartbeatAck,
        11 => PacketType::Disconnect,
        12 => PacketType::Error,
        13 => PacketType::ConversationMeta,
        _ => PacketType::X3DHHandshakeInit,
    };
    let _ = protocol::build_frame(pt, body);

    // ── 5. Body deserialization (must not panic on any input) ──
    // Try to deserialize various protocol types from random data
    // These may return Err — that's fine, they must not panic.
    let _ = protocol::deserialize::<protocol::DisconnectMessage>(data);
    let _ = protocol::deserialize::<protocol::ErrorMessage>(data);
    let _ = protocol::deserialize::<protocol::HandshakeInit>(data);
    let _ = protocol::deserialize::<protocol::HandshakeResponse>(data);
    let _ = protocol::deserialize::<protocol::EncryptedEnvelope>(data);
    let _ = protocol::deserialize::<protocol::FileTransferRequestData>(data);
    let _ = protocol::deserialize::<protocol::FileTransferChunkData>(data);
    let _ = protocol::deserialize::<protocol::FileTransferCompleteData>(data);
    let _ = protocol::deserialize::<protocol::ConversationMetaData>(data);
});
