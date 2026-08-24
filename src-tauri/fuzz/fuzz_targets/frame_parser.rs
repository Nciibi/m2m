//! Fuzz target: the length-prefixed frame parser — the FIRST code every
//! internet-facing byte hits. Must never panic; any input may produce an
//! Err or a valid frame, nothing else.
//!
//! Additionally, any successfully parsed frame body is fed to the
//! MessagePack deserializer for the matching wire struct so parser and
//! deserializer are exercised together (the receive-loop path).

#![no_main]


libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(async {
        // Feed raw bytes as a socket stream into read_frame.
        let mut stream = data;
        if let Ok(frame) = m2m_lib::network::read_frame(&mut stream).await {
            // Frame parsed: exercise the body deserializers for the
            // packet types that carry attacker-controlled payloads.
            match frame.packet_type {
                m2m_lib::protocol::PacketType::HandshakeInit => {
                    let _ = m2m_lib::protocol::deserialize::<
                        m2m_lib::protocol::HandshakeInit,
                    >(&frame.body);
                }
                m2m_lib::protocol::PacketType::HandshakeResponse => {
                    let _ = m2m_lib::protocol::deserialize::<
                        m2m_lib::protocol::HandshakeResponse,
                    >(&frame.body);
                }
                m2m_lib::protocol::PacketType::EncryptedMessage => {
                    let _ = m2m_lib::protocol::deserialize::<
                        m2m_lib::protocol::EncryptedEnvelope,
                    >(&frame.body);
                }
                m2m_lib::protocol::PacketType::FileTransferRequest => {
                    let _ = m2m_lib::protocol::deserialize::<
                        m2m_lib::protocol::FileTransferRequestData,
                    >(&frame.body);
                }
                _ => {}
            }
        }
    });
});