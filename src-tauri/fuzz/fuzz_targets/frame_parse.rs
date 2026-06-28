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
        let _ = m2m_lib::protocol::validate_frame_size(size);
    }

    // ── 2. Packet type parsing ──
    if data.len() >= 5 {
        let pt_byte = data[4];
        let _ = m2m_lib::protocol::PacketType::from_byte(pt_byte);
    }

    // ── 3. Version validation ──
    if data.len() >= 6 {
        let version = data[5];
        let _ = m2m_lib::protocol::validate_version(version);
    }

    // ── 4. Frame construction ──
    let body = if data.len() > 6 { &data[6..] } else { &[0u8; 1] };
    let pt = match data[0] % 15 {
        0 => m2m_lib::protocol::PacketType::HandshakeInit,
        1 => m2m_lib::protocol::PacketType::HandshakeResponse,
        2 => m2m_lib::protocol::PacketType::HandshakeComplete,
        3 => m2m_lib::protocol::PacketType::EncryptedMessage,
        4 => m2m_lib::protocol::PacketType::FileTransferRequest,
        5 => m2m_lib::protocol::PacketType::FileTransferChunk,
        6 => m2m_lib::protocol::PacketType::FileTransferComplete,
        7 => m2m_lib::protocol::PacketType::FileTransferAccept,
        8 => m2m_lib::protocol::PacketType::FileTransferReject,
        9 => m2m_lib::protocol::PacketType::Heartbeat,
        10 => m2m_lib::protocol::PacketType::HeartbeatAck,
        11 => m2m_lib::protocol::PacketType::Disconnect,
        12 => m2m_lib::protocol::PacketType::Error,
        13 => m2m_lib::protocol::PacketType::ConversationMeta,
        _ => m2m_lib::protocol::PacketType::X3DHHandshakeInit,
    };
    let _ = m2m_lib::protocol::build_frame(pt, body);

    // ── 5. Body deserialization (must not panic on any input) ──
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::DisconnectMessage>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::ErrorMessage>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::HandshakeInit>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::HandshakeResponse>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::EncryptedEnvelope>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::FileTransferRequestData>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::FileTransferChunkData>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::FileTransferCompleteData>(data);
    let _ = m2m_lib::protocol::deserialize::<m2m_lib::protocol::ConversationMetaData>(data);
});
