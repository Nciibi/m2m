//! Fuzz target for protocol frame parsing.
//!
//! Feeds random byte sequences into frame validation functions to detect
//! panics, crashes, or excessive resource consumption.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // ── 1. Frame size validation ──
    if data.len() >= 4 {
        let size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let _ = m2m::protocol::validate_frame_size(size);
    }

    // ── 2. Packet type parsing ──
    if data.len() >= 5 {
        let pt_byte = data[4];
        let _ = m2m::protocol::PacketType::from_byte(pt_byte);
    }

    // ── 3. Version validation ──
    if data.len() >= 6 {
        let version = data[5];
        let _ = m2m::protocol::validate_version(version);
    }

    // ── 4. Frame construction ──
    let body = if data.len() > 6 { &data[6..] } else { &[0u8; 1] };
    let pt = match data[0] % 15 {
        0 => m2m::protocol::PacketType::HandshakeInit,
        1 => m2m::protocol::PacketType::HandshakeResponse,
        2 => m2m::protocol::PacketType::HandshakeComplete,
        3 => m2m::protocol::PacketType::EncryptedMessage,
        4 => m2m::protocol::PacketType::FileTransferRequest,
        5 => m2m::protocol::PacketType::FileTransferChunk,
        6 => m2m::protocol::PacketType::FileTransferComplete,
        7 => m2m::protocol::PacketType::FileTransferAccept,
        8 => m2m::protocol::PacketType::FileTransferReject,
        9 => m2m::protocol::PacketType::Heartbeat,
        10 => m2m::protocol::PacketType::HeartbeatAck,
        11 => m2m::protocol::PacketType::Disconnect,
        12 => m2m::protocol::PacketType::Error,
        13 => m2m::protocol::PacketType::ConversationMeta,
        _ => m2m::protocol::PacketType::X3DHHandshakeInit,
    };
    let _ = m2m::protocol::build_frame(pt, body);

    // ── 5. Body deserialization (must not panic on any input) ──
    let _ = m2m::protocol::deserialize::<m2m::protocol::DisconnectMessage>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::ErrorMessage>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::HandshakeInit>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::HandshakeResponse>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::EncryptedEnvelope>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::FileTransferRequestData>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::FileTransferChunkData>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::FileTransferCompleteData>(data);
    let _ = m2m::protocol::deserialize::<m2m::protocol::ConversationMetaData>(data);
});
