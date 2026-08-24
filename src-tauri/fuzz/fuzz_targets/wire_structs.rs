//! Fuzz target: MessagePack deserialization of every attacker-reachable
//! wire struct. Deserializers must return Err on garbage — never panic,
//! never allocate unbounded memory from length fields.

#![no_main]


libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    use m2m_lib::protocol;

    macro_rules! try_deser {
        ($t:ty) => {
            let _ = protocol::deserialize::<$t>(data);
        };
    }

    try_deser!(protocol::HandshakeInit);
    try_deser!(protocol::HandshakeResponse);
    try_deser!(protocol::HandshakeComplete);
    try_deser!(protocol::EncryptedEnvelope);
    try_deser!(protocol::MessageBody);
    try_deser!(protocol::FileTransferRequestData);
    try_deser!(protocol::FileTransferChunkData);
    try_deser!(protocol::FileTransferCompleteData);
    try_deser!(protocol::FileTransferAcceptData);
    try_deser!(protocol::FileTransferRejectData);
    try_deser!(protocol::FileTransferCancelData);
    try_deser!(protocol::ConversationMetaData);
    try_deser!(protocol::DisconnectMessage);
    try_deser!(protocol::GroupEncryptedMessageData);
    try_deser!(protocol::GroupCreateData);
    try_deser!(protocol::GroupInviteData);
    try_deser!(protocol::GroupSenderKeyData);
    try_deser!(protocol::SyncRequestData);
    try_deser!(protocol::SyncDeviceInfo);
    try_deser!(protocol::SyncPayload);

    // Packet-type byte dispatch must be total (Err for unknown, never panic).
    // Exercise every possible byte.
    for b in data.iter().take(64) {
        let _ = protocol::PacketType::from_byte(*b);
        let _ = protocol::validate_version(*b);
    }
});