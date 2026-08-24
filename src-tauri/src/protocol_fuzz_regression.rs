//! Fuzz-derived regression tests (security roadmap §4 — Assurance).
//!
//! These encode malformed-input cases from the fuzzing campaign
//! (`src-tauri/fuzz/`) as DETERMINISTIC tests so parser regressions fail in
//! ordinary CI on every platform (libFuzzer itself needs nightly+Unix).
//! Contract under test: parsers return Err on garbage — NEVER panic,
//! never allocate unbounded memory.

use crate::crypto::{pad_message_variable, unpad_message_variable};
use crate::dht;
use crate::network;
use crate::protocol;

// ─── Frame parser ──────────────────────────────────────────────────────────

fn parse_frames(bytes: &[u8]) -> Result<network::RawFrame, network::NetworkError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    rt.block_on(async { network::read_frame(&mut &bytes[..]).await })
}

/// Regression for the FOUND BUG: a frame declaring 0 or 1 payload bytes
/// used to index payload[0] out of bounds — remote DoS.
#[test]
fn regression_frame_tiny_declared_len_never_panics() {
    // length prefix = 0 → empty payload
    assert!(parse_frames(&0u32.to_be_bytes()).is_err());
    // length prefix = 1 → only version byte, no type byte
    assert!(parse_frames(&1u32.to_be_bytes()).is_err());
}

#[test]
fn regression_frame_huge_declared_len_rejected() {
    // u32::MAX length must be rejected by the size cap BEFORE allocation.
    assert!(parse_frames(&u32::MAX.to_be_bytes()).is_err());
    assert!(parse_frames(&(u32::MAX / 2).to_be_bytes()).is_err());
}

#[test]
fn regression_frame_truncated_body_is_err_not_panic() {
    // Valid header claiming 64 bytes, but stream ends after 3.
    let mut b = 64u32.to_be_bytes().to_vec();
    b.extend_from_slice(&[crate::protocol::PROTOCOL_VERSION, 0x10, 0xAA]);
    assert!(parse_frames(&b).is_err());
}

/// Every single-byte and short-garbage stream is rejected gracefully.
#[test]
fn regression_frame_short_inputs_total() {
    let inputs: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        vec![0x00, 0x00],
        vec![0xFF; 3],
        vec![0xFF; 4],
        vec![0xFF; 5],
    ];
    for input in inputs {
        let _ = parse_frames(&input); // Err ok, panic NOT ok
    }
}

/// Valid frames with EVERY packet-type byte and hostile bodies parse or
/// reject — never panic. Covers version validation too.
#[test]
fn regression_frame_all_packet_types_with_hostile_body() {
    for type_byte in 0u8..=255 {
        let mut body = vec![0xEE; 300]; // msgpack-ish garbage
        body.insert(0, type_byte);
        body.insert(0, protocol::PROTOCOL_VERSION);
        let mut wire = (body.len() as u32).to_be_bytes().to_vec();
        wire.extend_from_slice(&body);
        if let Ok(frame) = parse_frames(&wire) {
            // If parsed, body deserializers must also survive garbage.
            let _ = protocol::deserialize::<protocol::MessageBody>(&frame.body);
            let _ = protocol::deserialize::<protocol::EncryptedEnvelope>(&frame.body);
        }
    }
}

// ─── Wire structs ──────────────────────────────────────────────────────────

/// Hand-crafted hostile MessagePack payloads against every attacker-
/// reachable struct: wrong types, truncated maps, absurd lengths.
#[test]
fn regression_wire_structs_hostile_msgpack() {
    let hostile: Vec<Vec<u8>> = vec![
        vec![],                       // empty
        vec![0xC0],                   // nil
        vec![0x91, 0x01],             // array where map expected
        vec![0xDE, 0xFF, 0xFF],       // truncated map16
        vec![0xDD, 0xFF, 0xFF, 0xFF, 0xFF], // huge map32 marker
        vec![0xC4, 0xFF],             // bin8 with absurd length claim
        vec![0xC5, 0xFF, 0xFF],       // bin16 absurd length
        vec![0xC6, 0x7F, 0xFF, 0xFF, 0xFF], // bin32 ~2GB length claim
        vec![0x92, 0xC3, 0xC2],       // array of bools where struct expected
        vec![0xA1],                   // truncated fixstr
        vec![0xD9, 0xFF, 0x41],       // str8 absurd length
        vec![0x01],                   // bare int
        vec![0xFF; 64],               // repeated negatives
    ];
    macro_rules! all {
        ($($t:ty),*) => {
            for input in &hostile {
                $(
                    let _ = protocol::deserialize::<$t>(input);
                )*
            }
        };
    }
    all!(
        protocol::HandshakeInit,
        protocol::HandshakeResponse,
        protocol::HandshakeComplete,
        protocol::EncryptedEnvelope,
        protocol::MessageBody,
        protocol::FileTransferRequestData,
        protocol::FileTransferChunkData,
        protocol::FileTransferCompleteData,
        protocol::FileTransferAcceptData,
        protocol::FileTransferRejectData,
        protocol::FileTransferCancelData,
        protocol::ConversationMetaData,
        protocol::DisconnectMessage,
        protocol::GroupEncryptedMessageData,
        protocol::GroupCreateData,
        protocol::GroupInviteData,
        protocol::GroupSenderKeyData,
        protocol::SyncRequestData,
        protocol::SyncDeviceInfo,
        protocol::SyncPayload
    );
}

/// Packet-type dispatch and version validation are TOTAL over u8.
#[test]
fn regression_packet_type_dispatch_total() {
    for b in 0u8..=255 {
        let _ = protocol::PacketType::from_byte(b);
        let _ = protocol::validate_version(b);
    }
}

/// Peer-declared transfer parameters are validated before allocation.
#[test]
fn regression_transfer_request_bounds() {
    // Absurd size/chunk combos must be rejected, not allocated.
    assert!(protocol::validate_transfer_request(u64::MAX, u32::MAX).is_err());
    assert!(protocol::validate_transfer_request(1 << 50, 1).is_err());
    assert!(protocol::validate_transfer_request(1024, 0).is_err());
}

// ─── Padding ───────────────────────────────────────────────────────────────

#[test]
fn regression_padding_hostile_suffixes() {
    // pad_len suffix claiming more padding than exists.
    let mut b = vec![0x41u8, 0x00];
    b.extend_from_slice(&1000u16.to_be_bytes());
    assert!(unpad_message_variable(&b).is_err());

    // Truncated below minimum size.
    assert!(unpad_message_variable(&[]).is_err());
    assert!(unpad_message_variable(&[0x41]).is_err());

    // Zero-pad-len with non-tier-aligned total must be rejected by the
    // tier verification.
    let b = vec![0x41u8, 0x42, 0x00, 0x00];
    assert!(unpad_message_variable(&b).is_err());
}

#[test]
fn regression_padding_roundtrip_property_small_inputs() {
    // Property check across a spread of sizes incl. tier boundaries.
    for len in [0usize, 1, 63, 64, 65, 255, 256, 257, 1023, 1024, 4095, 4097] {
        let pt = vec![0x5Au8; len];
        let padded = pad_message_variable(&pt);
        let out = unpad_message_variable(&padded).expect("valid padded input");
        assert_eq!(out, pt, "roundtrip failed at len {len}");
    }
}

// ─── DHT parsers ───────────────────────────────────────────────────────────

#[test]
fn regression_dht_parse_message_hostile() {
    // Too short.
    assert!(dht::parse_dht_message(&[]).is_err());
    assert!(dht::parse_dht_message(&[0u8; 4]).is_err());
    // Length-prefix mismatch.
    let mut b = 999u32.to_be_bytes().to_vec();
    b.push(0x01);
    assert!(dht::parse_dht_message(&b).is_err());
    // Huge declared length with tiny body.
    let mut b = u32::MAX.to_be_bytes().to_vec();
    b.push(0x02);
    assert!(dht::parse_dht_message(&b).is_err());
}

#[test]
fn regression_dht_node_response_hostile() {
    use crate::protocol;
    // Bad af_tag mid-stream.
    let mut body = Vec::new();
    body.extend_from_slice(&[0x11u8; 32]);
    body.push(5); // invalid af_tag
    body.extend_from_slice(&[10, 0, 0, 1]);
    body.extend_from_slice(&1234u16.to_be_bytes());
    body.extend_from_slice(&[0u8; 40]); // trailing junk breaks legacy alignment too
    assert!(dht::parse_node_response(&body).is_err());

    // Tagged IPv6 entry truncated mid-address.
    let mut body = vec![0u8; 32];
    body.push(6);
    body.extend_from_slice(&[0u8; 10]); // need 16
    assert!(dht::parse_node_response(&body).is_err());

    // Empty is fine.
    assert!(dht::parse_node_response(&[]).unwrap().is_empty());

    // Silence unused-import warning path if PROTOCOL_VERSION referenced.
    let _ = protocol::PROTOCOL_VERSION;
}
