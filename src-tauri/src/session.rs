/// M2M — Session Module
///
/// Manages encrypted session state: handshake execution, message encryption/decryption,
/// replay protection, sequencing, and session lifecycle.
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncRead, AsyncWrite};
use zeroize::Zeroize;

use crate::crypto::{self, DoubleRatchet, EphemeralKeypair, IdentityKeypair, SessionKeys,
    X25519IdentityKeypair};
use crate::network::{self, ConnectionState, RawFrame};
use crate::protocol::{
    self, DRHeader, EncryptedEnvelope, HandshakeComplete, HandshakeInit, HandshakeResponse,
    MessageBody, PacketType, PROTOCOL_VERSION, MAX_SESSION_DURATION_SECS,
    FileTransferRequestData, FileTransferChunkData, FileTransferCompleteData,
    FileTransferAcceptData, FileTransferRejectData,
    FileTransferChunkAckData, FileTransferCancelData,
    ConversationMetaData, WireCandidate, MAX_FILE_CHUNK_SIZE,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::CryptoError),
    #[error("protocol error: {0}")]
    Protocol(#[from] protocol::ProtocolError),
    #[error("network error: {0}")]
    Network(#[from] network::NetworkError),
    #[error("handshake failed: {0}")]
    HandshakeFailed(String),
    #[error("session expired")]
    SessionExpired,
    #[error("replay detected: counter {received} <= high water mark {expected}")]
    ReplayDetected { received: u64, expected: u64 },
    #[error("invalid session state for operation")]
    InvalidState,
}

/// An active encrypted session with a peer.
pub struct Session {
    /// Current connection state.
    pub state: ConnectionState,
    /// The peer's identity public key.
    pub peer_identity_pub: [u8; 32],
    /// Whether the peer's fingerprint has been verified out-of-band.
    pub peer_verified: bool,
    /// Session keys for encryption/decryption (legacy, pre-X3DH).
    session_keys: Option<SessionKeys>,
    /// Double Ratchet state (X3DH+DR sessions).
    ratchet: Option<DoubleRatchet>,
    /// Outgoing message counter (monotonically increasing).
    tx_counter: u64,
    /// Highest received counter (for replay protection).
    rx_high_water_mark: u64,
    /// Timestamp when the session was established.
    established_at: u64,
    /// Peer's network candidates received during handshake.
    pub peer_candidates: Vec<WireCandidate>,
    /// Our own candidates sent during handshake.
    pub our_candidates: Vec<WireCandidate>,
    /// Our own Ed25519 identity public key (used for DR AAD construction).
    our_identity_pub: [u8; 32],
    /// How many messages between DH ratchets (default 100). 0 = never ratchet.
    pub ratchet_interval: u64,
}

impl Session {
    /// Create a new session in the initial state.
    /// Uses a random initial counter to prevent cross-session replay attacks.
    /// Each session starts with a different counter value, so messages from
    /// a previous session cannot be replayed into a new session.
    pub fn new() -> Self {
        // Generate a random initial counter — prevents replay across sessions.
        let initial_counter = {
            let mut buf = [0u8; 8];
            let rand_bytes = crate::crypto::random_bytes(8);
            buf.copy_from_slice(&rand_bytes);
            u64::from_be_bytes(buf)
        };

        Self {
            state: ConnectionState::Disconnected,
            peer_identity_pub: [0u8; 32],
            peer_verified: false,
            session_keys: None,
            ratchet: None,
            tx_counter: initial_counter,
            rx_high_water_mark: initial_counter,
            established_at: 0,
            peer_candidates: Vec::new(),
            our_candidates: Vec::new(),
            our_identity_pub: [0u8; 32],
            ratchet_interval: 100,
        }
    }

    /// Execute the handshake as the initiator (client).
    /// We already know the peer's identity from the invite.
    /// `local_candidates` are our network candidates sent to the peer for ICE-Lite.
    /// `x25519_pub` is our X25519 identity public key (for X3DH backward compat in the handshake).
    /// It is distinct from the Ed25519 `identity` keypair.
    pub async fn handshake_as_initiator<S: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
        identity: &IdentityKeypair,
        expected_peer_pub: &[u8; 32],
        local_candidates: Vec<WireCandidate>,
        x25519_pub: [u8; 32],
    ) -> Result<(), SessionError> {
        self.state = ConnectionState::Handshaking;
        self.our_candidates = local_candidates.clone();

        // Generate ephemeral keypair for this session
        let ephemeral = EphemeralKeypair::generate();
        let now = now_unix_secs();

        // Build the data to sign: ephemeral_pub + timestamp
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&ephemeral.public_key_bytes());
        sign_data.extend_from_slice(&now.to_be_bytes());

        let signature = identity.sign(&sign_data);

        // Send HandshakeInit with our network candidates
        let init = HandshakeInit {
            version: PROTOCOL_VERSION,
            ephemeral_pub: ephemeral.public_key_bytes(),
            identity_pub: identity.public_key_bytes(),
            x25519_identity_pub: x25519_pub,
            used_opk: None,
            timestamp: now,
            signature,
            candidates: local_candidates,
        };
        let init_bytes = protocol::serialize(&init)?;
        network::write_frame(stream, PacketType::HandshakeInit, &init_bytes).await?;

        // Read HandshakeResponse
        let response_frame = network::read_frame(stream).await?;
        if response_frame.packet_type != PacketType::HandshakeResponse {
            return Err(SessionError::HandshakeFailed(format!(
                "expected HandshakeResponse, got {:?}",
                response_frame.packet_type
            )));
        }

        let response: HandshakeResponse = protocol::deserialize(&response_frame.body)?;

        // Validate response version
        protocol::validate_version(response.version).map_err(|e| {
            SessionError::HandshakeFailed(format!("version mismatch: {e}"))
        })?;

        // Verify peer's identity matches expected (from invite)
        if response.identity_pub != *expected_peer_pub {
            return Err(SessionError::HandshakeFailed(
                "peer identity does not match invite".to_string(),
            ));
        }

        // Verify peer's signature on their ephemeral key
        let mut peer_sign_data = Vec::new();
        peer_sign_data.extend_from_slice(&response.ephemeral_pub);
        peer_sign_data.extend_from_slice(&response.timestamp.to_be_bytes());

        crypto::verify_signature(&response.identity_pub, &peer_sign_data, &response.signature)
            .map_err(|_| {
                SessionError::HandshakeFailed("peer signature invalid".to_string())
            })?;

        // Derive session keys (we are the client/initiator)
        let session_keys = ephemeral
            .client_session_keys(&response.ephemeral_pub)
            .map_err(|e| SessionError::HandshakeFailed(format!("key derivation failed: {e}")))?;

        // Send HandshakeComplete with encrypted verification
        let verify_data = b"m2m-handshake-complete-v1";
        let aad = [PacketType::HandshakeComplete.to_byte()];
        let (nonce, ciphertext) = session_keys.encrypt(verify_data, &aad)?;

        let complete = HandshakeComplete {
            encrypted_verify: ciphertext,
            nonce,
        };
        let complete_bytes = protocol::serialize(&complete)?;
        network::write_frame(stream, PacketType::HandshakeComplete, &complete_bytes).await?;

        // Store peer candidates for ICE-Lite
        self.peer_candidates = response.candidates;

        // Session established
        self.peer_identity_pub = response.identity_pub;
        self.our_identity_pub = identity.public_key_bytes();
        self.session_keys = Some(session_keys);
        self.established_at = now_unix_secs();
        self.state = ConnectionState::Established;

        tracing::info!(peer = %self.peer_fingerprint(), candidates = %self.peer_candidates.len(), "session established as initiator");
        Ok(())
    }

    /// Execute the handshake as the responder (server).
    /// `local_candidates` are our network candidates sent to the peer for ICE-Lite.
    /// Execute the handshake as the responder (server).
    /// `local_candidates` are our network candidates sent to the peer for ICE-Lite.
    /// `x25519_pub` is our X25519 identity public key (for X3DH backward compat in the handshake).
    /// It is distinct from the Ed25519 `identity` keypair and should be the
    /// public key of the X25519IdentityKeypair stored in AppState.
    pub async fn handshake_as_responder<S: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
        identity: &IdentityKeypair,
        init_frame: &RawFrame,
        local_candidates: Vec<WireCandidate>,
        x25519_pub: [u8; 32],
    ) -> Result<(), SessionError> {
        self.state = ConnectionState::Handshaking;
        self.our_candidates = local_candidates.clone();

        // Parse the HandshakeInit we already received
        let init: HandshakeInit = protocol::deserialize(&init_frame.body)?;

        // Validate init version
        protocol::validate_version(init.version).map_err(|e| {
            SessionError::HandshakeFailed(format!("version mismatch: {e}"))
        })?;

        // Verify initiator's signature
        let mut peer_sign_data = Vec::new();
        peer_sign_data.extend_from_slice(&init.ephemeral_pub);
        peer_sign_data.extend_from_slice(&init.timestamp.to_be_bytes());

        crypto::verify_signature(&init.identity_pub, &peer_sign_data, &init.signature)
            .map_err(|_| {
                SessionError::HandshakeFailed("initiator signature invalid".to_string())
            })?;

        // Generate our ephemeral keypair
        let ephemeral = EphemeralKeypair::generate();
        let now = now_unix_secs();

        // Sign our ephemeral key
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&ephemeral.public_key_bytes());
        sign_data.extend_from_slice(&now.to_be_bytes());

        let signature = identity.sign(&sign_data);

        // Send HandshakeResponse with our network candidates
        let response = HandshakeResponse {
            version: PROTOCOL_VERSION,
            ephemeral_pub: ephemeral.public_key_bytes(),
            identity_pub: identity.public_key_bytes(),
            x25519_identity_pub: x25519_pub,
            timestamp: now,
            signature,
            candidates: local_candidates,
        };
        let response_bytes = protocol::serialize(&response)?;
        network::write_frame(stream, PacketType::HandshakeResponse, &response_bytes).await?;

        // Derive session keys (we are the server/responder)
        let session_keys = ephemeral
            .server_session_keys(&init.ephemeral_pub)
            .map_err(|e| SessionError::HandshakeFailed(format!("key derivation failed: {e}")))?;

        // Read HandshakeComplete
        let complete_frame = network::read_frame(stream).await?;
        if complete_frame.packet_type != PacketType::HandshakeComplete {
            return Err(SessionError::HandshakeFailed(format!(
                "expected HandshakeComplete, got {:?}",
                complete_frame.packet_type
            )));
        }

        let complete: HandshakeComplete = protocol::deserialize(&complete_frame.body)?;

        // Verify the encrypted verification data
        let aad = [PacketType::HandshakeComplete.to_byte()];
        let plaintext = session_keys
            .decrypt(&complete.encrypted_verify, &complete.nonce, &aad)
            .map_err(|_| {
                SessionError::HandshakeFailed("handshake verification decryption failed".to_string())
            })?;

        if plaintext != b"m2m-handshake-complete-v1" {
            return Err(SessionError::HandshakeFailed(
                "handshake verification mismatch".to_string(),
            ));
        }

        // Store peer candidates for ICE-Lite
        self.peer_candidates = init.candidates;

        // Session established
        self.peer_identity_pub = init.identity_pub;
        self.our_identity_pub = identity.public_key_bytes();
        self.session_keys = Some(session_keys);
        self.established_at = now_unix_secs();
        self.state = ConnectionState::Established;

        tracing::info!(peer = %self.peer_fingerprint(), candidates = %self.peer_candidates.len(), "session established as responder");
        Ok(())
    }

    /// Execute the X3DH + Double Ratchet handshake as the initiator.
    ///
    /// The peer's prekey bundle is extracted from the invite by the caller.
    /// The caller MUST have verified `bundle.signed_prekey_sig` against the peer's
    /// Ed25519 identity key before calling this.
    pub async fn handshake_as_initiator_x3dh<S: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
        identity: &IdentityKeypair,
        x25519_identity: &X25519IdentityKeypair,
        expected_peer_pub: &[u8; 32],
        peer_bundle: &crate::crypto::PrekeyBundle,
        local_candidates: Vec<WireCandidate>,
    ) -> Result<(), SessionError> {
        self.state = ConnectionState::Handshaking;
        self.our_candidates = local_candidates.clone();

        let ek_a = EphemeralKeypair::generate();
        let now = now_unix_secs();

        let x3dh_out = crate::crypto::x3dh_initiate(x25519_identity, &ek_a, peer_bundle)
            .map_err(|e| SessionError::HandshakeFailed(format!("x3dh: {e}")))?;

        let dh_ratchet = EphemeralKeypair::generate();

        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&ek_a.public_key_bytes());
        sign_data.extend_from_slice(&x25519_identity.public_key_bytes());
        sign_data.extend_from_slice(&now.to_be_bytes());
        let signature = identity.sign(&sign_data);

        let init = HandshakeInit {
            version: PROTOCOL_VERSION,
            ephemeral_pub: ek_a.public_key_bytes(),
            identity_pub: identity.public_key_bytes(),
            x25519_identity_pub: x25519_identity.public_key_bytes(),
            used_opk: None,
            timestamp: now,
            signature,
            candidates: local_candidates,
        };
        let init_bytes = protocol::serialize(&init)?;
        network::write_frame(stream, PacketType::X3DHHandshakeInit, &init_bytes).await?;

        let resp_frame = network::read_frame(stream).await?;
        if resp_frame.packet_type != PacketType::X3DHHandshakeResponse {
            return Err(SessionError::HandshakeFailed(format!(
                "expected X3DHHandshakeResponse, got {:?}", resp_frame.packet_type
            )));
        }
        let response: HandshakeResponse = protocol::deserialize(&resp_frame.body)?;

        protocol::validate_version(response.version).map_err(|e| {
            SessionError::HandshakeFailed(format!("version mismatch: {e}"))
        })?;
        if response.identity_pub != *expected_peer_pub {
            return Err(SessionError::HandshakeFailed("peer identity mismatch".to_string()));
        }
        let mut peer_sign_data = Vec::new();
        peer_sign_data.extend_from_slice(&response.ephemeral_pub);
        peer_sign_data.extend_from_slice(&response.x25519_identity_pub);
        peer_sign_data.extend_from_slice(&response.timestamp.to_be_bytes());
        crypto::verify_signature(&response.identity_pub, &peer_sign_data, &response.signature)
            .map_err(|_| SessionError::HandshakeFailed("peer signature invalid".to_string()))?;

        // Initialize Double Ratchet
        self.ratchet = Some(DoubleRatchet::new(
            x3dh_out, dh_ratchet, response.ephemeral_pub, true,
        ));

        // Send HandshakeComplete encrypted with Double Ratchet
        let verify_data = b"m2m-x3dh-handshake-v1";
        let aad = [PacketType::X3DHComplete.to_byte()];
        let (ratchet_key, msg_num, nonce, ciphertext) = self.ratchet.as_mut().unwrap()
            .encrypt(verify_data, &aad, false)?;

        let complete = EncryptedEnvelope {
            nonce,
            counter: 0,
            ciphertext,
            dr_header: Some(DRHeader {
                ratchet_key,
                previous_chain_length: 0,
                message_number: msg_num,
            }),
        };
        let complete_bytes = protocol::serialize(&complete)?;
        network::write_frame(stream, PacketType::X3DHComplete, &complete_bytes).await?;

        self.peer_candidates = response.candidates;
        self.peer_identity_pub = response.identity_pub;
        self.our_identity_pub = identity.public_key_bytes();
        self.established_at = now_unix_secs();
        self.state = ConnectionState::Established;

        tracing::info!(peer = %self.peer_fingerprint(), "session established via X3DH initiator");
        Ok(())
    }

    /// Execute the X3DH + Double Ratchet handshake as the responder.
    pub async fn handshake_as_responder_x3dh<S: AsyncRead + AsyncWrite + Unpin>(
        &mut self,
        stream: &mut S,
        identity: &IdentityKeypair,
        x25519_identity: &X25519IdentityKeypair,
        signed_prekey: &EphemeralKeypair,
        init_frame: &RawFrame,
        local_candidates: Vec<WireCandidate>,
    ) -> Result<(), SessionError> {
        self.state = ConnectionState::Handshaking;
        self.our_candidates = local_candidates.clone();

        let init: HandshakeInit = protocol::deserialize(&init_frame.body)?;

        protocol::validate_version(init.version).map_err(|e| {
            SessionError::HandshakeFailed(format!("version mismatch: {e}"))
        })?;
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&init.ephemeral_pub);
        sign_data.extend_from_slice(&init.x25519_identity_pub);
        sign_data.extend_from_slice(&init.timestamp.to_be_bytes());
        crypto::verify_signature(&init.identity_pub, &sign_data, &init.signature)
            .map_err(|_| SessionError::HandshakeFailed("initiator signature invalid".to_string()))?;

        let x3dh_out = crate::crypto::x3dh_respond(
            x25519_identity, signed_prekey, None,
            &init.ephemeral_pub, &init.x25519_identity_pub,
        ).map_err(|e| SessionError::HandshakeFailed(format!("x3dh: {e}")))?;

        let ek_b = EphemeralKeypair::generate();
        let ek_b_pub = ek_b.public_key_bytes();
        let now = now_unix_secs();

        self.ratchet = Some(DoubleRatchet::new(
            x3dh_out, ek_b, init.ephemeral_pub, false,
        ));

        let mut our_sign_data = Vec::new();
        our_sign_data.extend_from_slice(&ek_b_pub);
        our_sign_data.extend_from_slice(&x25519_identity.public_key_bytes());
        our_sign_data.extend_from_slice(&now.to_be_bytes());
        let signature = identity.sign(&our_sign_data);

        let response = HandshakeResponse {
            version: PROTOCOL_VERSION,
            ephemeral_pub: ek_b_pub,
            identity_pub: identity.public_key_bytes(),
            x25519_identity_pub: x25519_identity.public_key_bytes(),
            timestamp: now,
            signature,
            candidates: local_candidates,
        };
        let resp_bytes = protocol::serialize(&response)?;
        network::write_frame(stream, PacketType::X3DHHandshakeResponse, &resp_bytes).await?;

        let complete_frame = network::read_frame(stream).await?;
        if complete_frame.packet_type != PacketType::X3DHComplete {
            return Err(SessionError::HandshakeFailed(format!(
                "expected X3DHComplete, got {:?}", complete_frame.packet_type
            )));
        }
        let complete: EncryptedEnvelope = protocol::deserialize(&complete_frame.body)?;

        let dr_hdr = complete.dr_header
            .ok_or_else(|| SessionError::HandshakeFailed("missing dr_header".to_string()))?;
        let plaintext = self.ratchet.as_mut().unwrap()
            .decrypt(&complete.ciphertext, &complete.nonce,
                     &[PacketType::X3DHComplete.to_byte()],
                     dr_hdr.message_number, dr_hdr.ratchet_key.as_ref())
            .map_err(|_| SessionError::HandshakeFailed("verification failed".to_string()))?;

        if plaintext != b"m2m-x3dh-handshake-v1" {
            return Err(SessionError::HandshakeFailed("verification mismatch".to_string()));
        }

        self.peer_candidates = init.candidates;
        self.peer_identity_pub = init.identity_pub;
        self.our_identity_pub = identity.public_key_bytes();
        self.established_at = now_unix_secs();
        self.state = ConnectionState::Established;

        tracing::info!(peer = %self.peer_fingerprint(), "session established via X3DH responder");
        Ok(())
    }

    /// Encrypt and send a text message.
    pub async fn send_text<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        text: &str,
    ) -> Result<String, SessionError> {
        self.send_text_with_timer(stream, text, None).await
    }

    /// Send a text message with an optional self-destruct timer.
    /// When `disappear_after` is Some(secs), the peer will auto-delete
    /// the message after that many seconds from receipt.
    pub async fn send_text_with_timer<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        text: &str,
        disappear_after: Option<u64>,
    ) -> Result<String, SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        self.check_expiry()?;

        let msg_id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let body = MessageBody::Text {
            id: msg_id.clone(),
            content: text.to_string(),
            disappear_after,
            timestamp: now,
        };
        let body_bytes = protocol::serialize(&body)?;

        self.send_encrypted(stream, &body_bytes).await?;
        Ok(msg_id)
    }

    /// Encrypt a payload and send it as an EncryptedMessage.
    /// Applies KDF ratchet after encryption for forward secrecy.
    /// Pads plaintext to obfuscate message length on the wire.
    async fn send_encrypted<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        plaintext: &[u8],
    ) -> Result<(), SessionError> {
        // ── Double Ratchet path (if active) ──
        let peer_pub = self.peer_identity_pub;
        let our_pub = self.our_identity_pub;
        if let Some(ratchet) = self.ratchet.as_mut() {
            let padded = crate::crypto::pad_message_variable(plaintext);
            let aad = session_dr_aad(PacketType::EncryptedMessage.to_byte(), &our_pub, &peer_pub);
            let do_ratchet = ratchet.should_ratchet(self.ratchet_interval);
            let (ratchet_key, msg_num, nonce, ciphertext) = ratchet
                .encrypt(&padded, &aad, do_ratchet)?;

            let envelope = EncryptedEnvelope {
                nonce,
                counter: 0,
                ciphertext,
                dr_header: Some(DRHeader {
                    ratchet_key,
                    previous_chain_length: 0,
                    message_number: msg_num,
                }),
            };
            let envelope_bytes = protocol::serialize(&envelope)?;
            network::write_frame(stream, PacketType::EncryptedMessage, &envelope_bytes).await?;
            return Ok(());
        }

        // ── Legacy SessionKeys path ──
        let keys = self
            .session_keys
            .as_mut()
            .ok_or(SessionError::InvalidState)?;

        self.tx_counter += 1;

        let padded = crate::crypto::pad_message_variable(plaintext);

        let mut aad = Vec::with_capacity(9);
        aad.push(PacketType::EncryptedMessage.to_byte());
        aad.extend_from_slice(&self.tx_counter.to_be_bytes());

        let (nonce, ciphertext) = keys.encrypt(&padded, &aad)?;

        keys.ratchet_tx();

        let envelope = EncryptedEnvelope {
            nonce,
            counter: self.tx_counter,
            ciphertext,
            dr_header: None,
        };
        let envelope_bytes = protocol::serialize(&envelope)?;

        network::write_frame(stream, PacketType::EncryptedMessage, &envelope_bytes).await?;
        Ok(())
    }

    /// Receive and decrypt an encrypted message.
    pub fn decrypt_message(&mut self, frame: &RawFrame) -> Result<MessageBody, SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        self.check_expiry()?;

        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body)?;

        // ── Double Ratchet path (if dr_header present) ──
        let peer_pub = self.peer_identity_pub;
        let our_pub = self.our_identity_pub;
        if let Some(dr_hdr) = &envelope.dr_header {
            let ratchet = self.ratchet
                .as_mut()
                .ok_or(SessionError::InvalidState)?;
            let aad = session_dr_aad(PacketType::EncryptedMessage.to_byte(), &our_pub, &peer_pub);
            let padded = ratchet
                .decrypt(&envelope.ciphertext, &envelope.nonce, &aad,
                         dr_hdr.message_number, dr_hdr.ratchet_key.as_ref())
                .map_err(SessionError::Crypto)?;
            let plaintext = crate::crypto::unpad_message_variable(&padded)?;
            let body: MessageBody = protocol::deserialize(&plaintext)?;
            return Ok(body);
        }

        // ── Legacy path: replay protection + SessionKeys ──
        if envelope.counter <= self.rx_high_water_mark {
            return Err(SessionError::ReplayDetected {
                received: envelope.counter,
                expected: self.rx_high_water_mark + 1,
            });
        }

        let keys = self
            .session_keys
            .as_mut()
            .ok_or(SessionError::InvalidState)?;

        let mut aad = Vec::with_capacity(9);
        aad.push(PacketType::EncryptedMessage.to_byte());
        aad.extend_from_slice(&envelope.counter.to_be_bytes());

        let padded = keys.decrypt(&envelope.ciphertext, &envelope.nonce, &aad)?;

        // Remove padding to recover original plaintext
        let plaintext = crate::crypto::unpad_message_variable(&padded)?;

        // ═══ Forward Secrecy Ratchet ═══
        // Evolve the receiving key AFTER successful decryption.
        // The sender ratcheted their tx_key after encrypting; we ratchet
        // our rx_key (which mirrors their tx_key) after decrypting.
        keys.ratchet_rx();

        // Update high water mark only after successful decryption + ratchet
        self.rx_high_water_mark = envelope.counter;

        let body: MessageBody = protocol::deserialize(&plaintext)?;
        Ok(body)
    }

    /// Check if the session has expired.
    fn check_expiry(&self) -> Result<(), SessionError> {
        if self.established_at == 0 {
            return Ok(());
        }
        let elapsed = now_unix_secs().saturating_sub(self.established_at);
        if elapsed > MAX_SESSION_DURATION_SECS {
            return Err(SessionError::SessionExpired);
        }
        Ok(())
    }

    /// Send a file transfer request to the peer (v2 — with per-chunk hashes and protocol version).
    pub async fn send_file_request_v2<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        req: &FileTransferRequestData,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        self.check_expiry()?;

        let body_bytes = protocol::serialize(req)?;
        self.send_encrypted_typed(stream, PacketType::FileTransferRequest, &body_bytes).await
    }

    /// Send a file transfer request to the peer (v1 — backward compat).
    #[cfg(test)]
    pub async fn send_file_request<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        transfer_id: &str,
        filename: &str,
        total_size: u64,
        total_chunks: u32,
        file_hash: Vec<u8>,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        self.check_expiry()?;

        let req = FileTransferRequestData {
            transfer_id: transfer_id.to_string(),
            filename: filename.to_string(),
            total_size,
            total_chunks,
            file_hash,
            chunk_hashes: Vec::new(),
            file_transfer_version: 0,
        };
        let body_bytes = protocol::serialize(&req)?;
        self.send_encrypted_typed(stream, PacketType::FileTransferRequest, &body_bytes).await
    }

    /// Send a single file chunk.
    pub async fn send_file_chunk<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        transfer_id: &str,
        chunk_index: u32,
        data: Vec<u8>,
        chunk_hash: Vec<u8>,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        self.check_expiry()?;
        if data.len() > MAX_FILE_CHUNK_SIZE {
            return Err(SessionError::Protocol(protocol::ProtocolError::MessageTooLarge));
        }

        let chunk = FileTransferChunkData {
            transfer_id: transfer_id.to_string(),
            chunk_index,
            data,
            chunk_hash,
        };
        let body_bytes = protocol::serialize(&chunk)?;
        self.send_encrypted_typed(stream, PacketType::FileTransferChunk, &body_bytes).await
    }

    /// Send file transfer complete notification.
    pub async fn send_file_complete<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        transfer_id: &str,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        self.check_expiry()?;

        let complete = FileTransferCompleteData {
            transfer_id: transfer_id.to_string(),
        };
        let body_bytes = protocol::serialize(&complete)?;
        self.send_encrypted_typed(stream, PacketType::FileTransferComplete, &body_bytes).await
    }

    /// Accept an incoming file transfer.
    pub async fn send_file_accept<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        transfer_id: &str,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        let body = protocol::serialize(&FileTransferAcceptData {
            transfer_id: transfer_id.to_string(),
        })?;
        self.send_encrypted_typed(stream, PacketType::FileTransferAccept, &body).await
    }

    /// Reject an incoming file transfer.
    pub async fn send_file_reject<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        transfer_id: &str,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        let body = protocol::serialize(&FileTransferRejectData {
            transfer_id: transfer_id.to_string(),
        })?;
        self.send_encrypted_typed(stream, PacketType::FileTransferReject, &body).await
    }

    /// Send a chunk acknowledgement to the sender, confirming the chunk was
    /// received, hash-verified, and written to disk.
    #[cfg(test)]
    pub async fn send_file_chunk_ack<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        transfer_id: &str,
        chunk_index: u32,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        let body = protocol::serialize(&FileTransferChunkAckData {
            transfer_id: transfer_id.to_string(),
            chunk_index,
        })?;
        self.send_encrypted_typed(stream, PacketType::FileTransferChunkAck, &body).await
    }

    /// Send a cancel notification to the peer for an in-progress file transfer.
    /// Either side can send this. The receiver stops accepting chunks and cleans up.
    /// The sender stops sending and marks the transfer as cancelled.
    pub async fn send_file_cancel<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        transfer_id: &str,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        let body = protocol::serialize(&FileTransferCancelData {
            transfer_id: transfer_id.to_string(),
        })?;
        self.send_encrypted_typed(stream, PacketType::FileTransferCancel, &body).await
    }

    /// Send conversation metadata (display names) to the peer.
    pub async fn send_conversation_meta<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        my_display_name: &str,
        your_display_name: &str,
    ) -> Result<(), SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        let meta = ConversationMetaData {
            my_display_name: my_display_name.to_string(),
            your_display_name: your_display_name.to_string(),
        };
        let body_bytes = protocol::serialize(&meta)?;
        self.send_encrypted_typed(stream, PacketType::ConversationMeta, &body_bytes).await
    }

    /// Encrypt and send data with a specific packet type.
    /// Applies KDF ratchet after encryption for forward secrecy.
    /// Pads plaintext to obfuscate message length.
    ///
    /// Automatically uses the Double Ratchet path if an X3DH+DR session is active,
    /// falling back to the legacy SessionKeys path for backward compatibility.
    pub async fn send_encrypted_typed<W: AsyncWrite + Unpin>(
        &mut self,
        stream: &mut W,
        packet_type: PacketType,
        plaintext: &[u8],
    ) -> Result<(), SessionError> {
        // ── Double Ratchet path (X3DH+DR sessions) ──
        let peer_pub = self.peer_identity_pub;
        let our_pub = self.our_identity_pub;
        if let Some(ratchet) = self.ratchet.as_mut() {
            let padded = crate::crypto::pad_message_variable(plaintext);
            let aad = session_dr_aad(packet_type.to_byte(), &our_pub, &peer_pub);
            let do_ratchet = ratchet.should_ratchet(self.ratchet_interval);
            let (ratchet_key, msg_num, nonce, ciphertext) = ratchet
                .encrypt(&padded, &aad, do_ratchet)?;

            let envelope = EncryptedEnvelope {
                nonce,
                counter: 0,
                ciphertext,
                dr_header: Some(DRHeader {
                    ratchet_key,
                    previous_chain_length: 0,
                    message_number: msg_num,
                }),
            };
            let envelope_bytes = protocol::serialize(&envelope)?;
            network::write_frame(stream, packet_type, &envelope_bytes).await?;
            return Ok(());
        }

        // ── Legacy SessionKeys path ──
        let keys = self
            .session_keys
            .as_mut()
            .ok_or(SessionError::InvalidState)?;

        self.tx_counter += 1;

        // Pad plaintext to obfuscate true length
        let padded = crate::crypto::pad_message_variable(plaintext);

        let mut aad = Vec::with_capacity(9);
        aad.push(packet_type.to_byte());
        aad.extend_from_slice(&self.tx_counter.to_be_bytes());

        let (nonce, ciphertext) = keys.encrypt(&padded, &aad)?;

        // Forward secrecy ratchet
        keys.ratchet_tx();

        let envelope = EncryptedEnvelope {
            nonce,
            counter: self.tx_counter,
            ciphertext,
            dr_header: None,
        };
        let envelope_bytes = protocol::serialize(&envelope)?;

        network::write_frame(stream, packet_type, &envelope_bytes).await?;
        Ok(())
    }

    /// Send a heartbeat to keep the connection alive.
    /// Heartbeats are unencrypted protocol-level keepalives.
    #[expect(dead_code, reason = "Reserved; network-level send_heartbeat used instead")]
    pub async fn send_heartbeat<W: AsyncWrite + Unpin>(
        &self,
        stream: &mut W,
    ) -> Result<(), SessionError> {
        network::write_frame(stream, PacketType::Heartbeat, &[])
            .await
            .map_err(SessionError::Network)
    }

    /// Send a heartbeat acknowledgement.
    #[expect(dead_code, reason = "Reserved; network-level send_heartbeat_ack used instead")]
    pub async fn send_heartbeat_ack<W: AsyncWrite + Unpin>(
        &self,
        stream: &mut W,
    ) -> Result<(), SessionError> {
        network::write_frame(stream, PacketType::HeartbeatAck, &[])
            .await
            .map_err(SessionError::Network)
    }

    /// Check if a received frame is a heartbeat and handle it.
    /// Returns true if the frame was a heartbeat (caller should not process further).
    #[expect(dead_code, reason = "Reserved for session-level heartbeat handling")]
    pub fn handle_heartbeat(&self, frame: &network::RawFrame) -> bool {
        frame.packet_type == PacketType::Heartbeat
            || frame.packet_type == PacketType::HeartbeatAck
    }
    /// Decrypt a typed frame (file transfers, conversation metadata, etc.).
    ///
    /// Automatically detects whether the envelope uses the Double Ratchet (DR header present)
    /// or the legacy SessionKeys path. For DR envelopes, replay protection is provided by
    /// the chain key advancement — old message numbers produce different chain states.
    pub fn decrypt_typed_frame(&mut self, frame: &RawFrame) -> Result<Vec<u8>, SessionError> {
        if self.state != ConnectionState::Established {
            return Err(SessionError::InvalidState);
        }
        self.check_expiry()?;

        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body)?;

        // ── Double Ratchet path (X3DH+DR sessions) ──
        let peer_pub = self.peer_identity_pub;
        let our_pub = self.our_identity_pub;
        if let Some(dr_hdr) = &envelope.dr_header {
            let ratchet = self.ratchet
                .as_mut()
                .ok_or(SessionError::InvalidState)?;
            let aad = session_dr_aad(frame.packet_type.to_byte(), &our_pub, &peer_pub);
            let padded = ratchet
                .decrypt(&envelope.ciphertext, &envelope.nonce, &aad,
                         dr_hdr.message_number, dr_hdr.ratchet_key.as_ref())
                .map_err(SessionError::Crypto)?;
            let plaintext = crate::crypto::unpad_message_variable(&padded)?;
            return Ok(plaintext);
        }

        // ── Legacy path: replay protection + SessionKeys ──
        if envelope.counter <= self.rx_high_water_mark {
            return Err(SessionError::ReplayDetected {
                received: envelope.counter,
                expected: self.rx_high_water_mark + 1,
            });
        }

        let keys = self
            .session_keys
            .as_mut()
            .ok_or(SessionError::InvalidState)?;

        let mut aad = Vec::with_capacity(9);
        aad.push(frame.packet_type.to_byte());
        aad.extend_from_slice(&envelope.counter.to_be_bytes());

        let padded = keys.decrypt(&envelope.ciphertext, &envelope.nonce, &aad)?;

        // Remove padding
        let plaintext = crate::crypto::unpad_message_variable(&padded)?;

        // Forward secrecy ratchet
        keys.ratchet_rx();

        self.rx_high_water_mark = envelope.counter;

        Ok(plaintext)
    }

    /// Get the peer's fingerprint for display/verification.
    pub fn peer_fingerprint(&self) -> String {
        crypto::fingerprint_from_public_key(&self.peer_identity_pub)
    }

    /// Mark the peer as verified (user confirmed fingerprint out-of-band).
    pub fn mark_peer_verified(&mut self) {
        self.peer_verified = true;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Ensure session keys are zeroized on drop (SessionKeys has its own Drop).
        self.session_keys.take();
        self.peer_identity_pub.zeroize();
        self.tx_counter = 0;
        self.rx_high_water_mark = 0;
    }
}

/// Build the AAD for Double Ratchet encrypted messages.
///
/// Includes the packet type and BOTH identity public keys (ours and peer's)
/// sorted lexicographically to produce the same value on both sides of
/// the session. This binds ciphertexts to a specific session pair for
/// defense-in-depth — the chain keys are already session-unique via X3DH.
///
/// This is a free function (not a method on `Session`) so callers can
/// extract the identity keys before taking a mutable borrow on `self`.
fn session_dr_aad(
    packet_type_byte: u8,
    our_identity_pub: &[u8; 32],
    peer_identity_pub: &[u8; 32],
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(65);
    aad.push(packet_type_byte);
    // Include both keys sorted to ensure the same AAD on both sides.
    if our_identity_pub < peer_identity_pub {
        aad.extend_from_slice(our_identity_pub);
        aad.extend_from_slice(peer_identity_pub);
    } else {
        aad.extend_from_slice(peer_identity_pub);
        aad.extend_from_slice(our_identity_pub);
    }
    aad
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs()
}

#[cfg(test)]
mod session_tests {
    use super::*;
    use crate::crypto::{self, IdentityKeypair, SessionKeys};
    use crate::protocol::{EncryptedEnvelope, PacketType, PROTOCOL_VERSION};
    fn init_crypto() {
        let _ = crypto::init();
    }

    /// Generate a pair of identities (Ed25519 + X25519) for testing.
    /// Returns (ed25519_keypair, x25519_keypair).
    fn make_identities() -> (IdentityKeypair, crate::crypto::X25519IdentityKeypair) {
        (IdentityKeypair::generate().unwrap(), crate::crypto::X25519IdentityKeypair::generate())
    }

    fn make_session_keys() -> SessionKeys {
        // We can't construct SessionKeys directly (fields are private),
        // so we do a minimal key exchange to get valid keys.
        let alice_eph = crate::crypto::EphemeralKeypair::generate();
        let bob_eph = crate::crypto::EphemeralKeypair::generate();
        let alice_keys = alice_eph
            .client_session_keys(&bob_eph.public_key_bytes())
            .unwrap();
        let _bob_keys = bob_eph
            .server_session_keys(&alice_eph.public_key_bytes())
            .unwrap();
        alice_keys
    }

    // ─── Unit Tests ───────────────────────────────────────────

    #[test]
    fn test_session_new() {
        init_crypto();
        let s = Session::new();
        assert_eq!(s.state, ConnectionState::Disconnected);
        assert_eq!(s.peer_identity_pub, [0u8; 32]);
        assert!(!s.peer_verified);
        assert!(s.session_keys.is_none());
        assert_eq!(s.established_at, 0);
        assert!(s.peer_candidates.is_empty());
        assert!(s.our_candidates.is_empty());
        // Random initial counters should be non-zero (extremely unlikely to be zero)
        assert!(s.tx_counter > 0 || s.rx_high_water_mark > 0,
            "counters should be random, got tx={} rx={}", s.tx_counter, s.rx_high_water_mark);
    }

    #[test]
    fn test_session_initial_counters_random() {
        init_crypto();
        // Generate multiple sessions and verify counters aren't all the same
        let sessions: Vec<Session> = (0..5).map(|_| Session::new()).collect();
        let tx_vals: Vec<u64> = sessions.iter().map(|s| s.tx_counter).collect();
        let rx_vals: Vec<u64> = sessions.iter().map(|s| s.rx_high_water_mark).collect();
        // At most one session may have the same tx_counter (collision extremely unlikely)
        let unique_tx: std::collections::HashSet<&u64> = tx_vals.iter().collect();
        let unique_rx: std::collections::HashSet<&u64> = rx_vals.iter().collect();
        assert!(unique_tx.len() >= 4, "tx counters not random enough: {:?}", tx_vals);
        assert!(unique_rx.len() >= 4, "rx counters not random enough: {:?}", rx_vals);
    }

    #[test]
    fn test_mark_peer_verified() {
        init_crypto();
        let mut s = Session::new();
        assert!(!s.peer_verified);
        s.mark_peer_verified();
        assert!(s.peer_verified);
    }

    #[test]
    fn test_peer_fingerprint_no_identity() {
        init_crypto();
        let s = Session::new();
        let fp = s.peer_fingerprint();
        // Fingerprint of all-zero key should be deterministic
        assert_eq!(fp.len(), 39, "fingerprint should be 39 chars (16 hex bytes with separators)");
    }

    #[test]
    fn test_drop_clears_keys() {
        init_crypto();
        let mut s = Session::new();
        s.session_keys = Some(make_session_keys());
        s.peer_identity_pub = [0xAB; 32];
        drop(s);
        // Can't assert post-drop, but this ensures the Drop impl doesn't panic.
        // (Valgrind/Miri would catch actual zeroize failures.)
    }

    #[test]
    fn test_check_expiry_no_established() {
        init_crypto();
        let s = Session::new();
        assert_eq!(s.established_at, 0);
        // Not yet established — check_expiry should return Ok
        assert!(s.check_expiry().is_ok());
    }

    #[test]
    fn test_check_expiry_fresh() {
        init_crypto();
        let mut s = Session::new();
        s.established_at = now_unix_secs();
        assert!(s.check_expiry().is_ok());
    }

    #[test]
    fn test_check_expiry_expired() {
        init_crypto();
        let mut s = Session::new();
        // Set established_at far in the past
        s.established_at = 1; // Unix epoch + 1 second
        assert!(s.check_expiry().is_err());
        assert!(matches!(s.check_expiry(), Err(SessionError::SessionExpired)));
    }

    #[test]
    fn test_state_transitions_reject_operations() {
        init_crypto();
        let mut s = Session::new();
        // No session keys, wrong state — operations should fail
        let _dummy: &[u8] = &[];
        let result = s.decrypt_message(&RawFrame {
            version: PROTOCOL_VERSION,
            packet_type: PacketType::EncryptedMessage,
            body: vec![],
        });
        assert!(matches!(result, Err(SessionError::InvalidState)));
    }

    #[test]
    fn test_replay_protection() {
        init_crypto();
        let mut s = Session::new();
        let keys = make_session_keys();
        s.session_keys = Some(keys);
        s.state = ConnectionState::Established;
        s.rx_high_water_mark = 100;
        s.established_at = now_unix_secs();

        // Build a frame with a counter <= high water mark
        let env = EncryptedEnvelope {
            nonce: vec![0u8; 24],
            counter: 50, // <= 100, should be rejected
            ciphertext: vec![0u8; 16],
            dr_header: None,
        };
        let body = crate::protocol::serialize(&env).unwrap();
        let frame = RawFrame {
            version: PROTOCOL_VERSION,
            packet_type: PacketType::EncryptedMessage,
            body,
        };

        let result = s.decrypt_message(&frame);
        assert!(matches!(result, Err(SessionError::ReplayDetected { .. })));
    }

    #[test]
    fn test_established_at_tracking() {
        init_crypto();
        let now = now_unix_secs();
        let mut s = Session::new();
        s.established_at = now;
        assert_eq!(s.established_at, now);
    }

    // ─── Async Integration Tests ──────────────────────────────

    #[tokio::test]
    async fn test_send_and_receive_text() {
        init_crypto();
        // Set up sessions with keys manually
        let mut alice = Session::new();
        let mut bob = Session::new();
        let keys_alice = make_session_keys();
        let keys_bob = make_session_keys();

        alice.session_keys = Some(keys_alice);
        alice.state = ConnectionState::Established;
        alice.established_at = now_unix_secs();

        bob.session_keys = Some(keys_bob);
        bob.state = ConnectionState::Established;
        bob.established_at = now_unix_secs();

        let (mut alice_stream, mut bob_read) = tokio::io::duplex(65536);

        // Alice sends a text message
        let msg_id = alice.send_text(&mut alice_stream, "Hello, Bob!").await.unwrap();
        assert!(!msg_id.is_empty(), "message ID should not be empty");

        // Bob reads the frame
        let frame = crate::network::read_frame_impl(&mut bob_read).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::EncryptedMessage);

        // Bob couldn't decrypt it because keys don't match (we used different key pairs)
        // The test just verifies that send_text completes and produces a valid frame.
        // For a proper decrypt test we need matching keys.
        let _ = frame;
        let _ = msg_id;
    }

    #[tokio::test]
    async fn test_send_and_decrypt_with_matching_keys() {
        init_crypto();
        // Generate a matching keypair for both sessions
        let eph = EphemeralKeypair::generate();
        let eph2 = EphemeralKeypair::generate();

        let alice_keys = eph.client_session_keys(&eph2.public_key_bytes()).unwrap();
        let bob_keys = eph2.server_session_keys(&eph.public_key_bytes()).unwrap();

        let mut alice = Session::new();
        let mut bob = Session::new();
        alice.session_keys = Some(alice_keys);
        alice.state = ConnectionState::Established;
        alice.established_at = now_unix_secs();
        alice.tx_counter = 100;
        bob.session_keys = Some(bob_keys);
        bob.state = ConnectionState::Established;
        bob.established_at = now_unix_secs();
        bob.rx_high_water_mark = 0;

        // Duplex for communication
        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        // Alice encodes a text message body directly and sends it via send_message's
        // internal send_encrypted path. Since send_text is async, we use it.
        let test_text = "Secret message 🔒";
        let msg_id = alice.send_text(&mut alice_w, test_text).await.unwrap();
        assert!(!msg_id.is_empty());

        // Bob reads and decrypts the frame
        let frame = crate::network::read_frame_impl(&mut bob_r).await.unwrap();
        let body = bob.decrypt_message(&frame).unwrap();
        match &body {
            crate::protocol::MessageBody::Text { id, content, .. } => {
                assert_eq!(content, test_text);
                assert_eq!(id, &msg_id);
            }
            other => panic!("expected Text body, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_file_transfer_request_roundtrip() {
        init_crypto();
        let eph = EphemeralKeypair::generate();
        let eph2 = EphemeralKeypair::generate();
        let alice_keys = eph.client_session_keys(&eph2.public_key_bytes()).unwrap();
        let bob_keys = eph2.server_session_keys(&eph.public_key_bytes()).unwrap();

        let mut alice = Session::new();
        let mut bob = Session::new();
        alice.session_keys = Some(alice_keys);
        alice.state = ConnectionState::Established;
        alice.established_at = now_unix_secs();
        alice.tx_counter = 100;
        bob.session_keys = Some(bob_keys);
        bob.state = ConnectionState::Established;
        bob.established_at = now_unix_secs();
        bob.rx_high_water_mark = 0;

        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        // Send a file transfer request
        let transfer_id = "test-transfer-001";
        alice.send_file_request(
            &mut alice_w,
            transfer_id,
            "report.pdf",
            1048576,
            16,
            vec![0xAB; 32],
        ).await.unwrap();

        // Bob receives and decrypts
        let frame = crate::network::read_frame_impl(&mut bob_r).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::FileTransferRequest);

        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let req: crate::protocol::FileTransferRequestData = crate::protocol::deserialize(&plaintext).unwrap();
        assert_eq!(req.transfer_id, transfer_id);
        assert_eq!(req.filename, "report.pdf");
        assert_eq!(req.total_size, 1048576);
        assert_eq!(req.total_chunks, 16);
    }

    #[tokio::test]
    async fn test_replay_protection_integration() {
        init_crypto();
        let eph = EphemeralKeypair::generate();
        let eph2 = EphemeralKeypair::generate();
        let alice_keys = eph.client_session_keys(&eph2.public_key_bytes()).unwrap();
        let bob_keys = eph2.server_session_keys(&eph.public_key_bytes()).unwrap();

        let mut alice = Session::new();
        let mut bob = Session::new();
        alice.session_keys = Some(alice_keys);
        alice.state = ConnectionState::Established;
        alice.established_at = now_unix_secs();
        alice.tx_counter = 100;
        bob.session_keys = Some(bob_keys);
        bob.state = ConnectionState::Established;
        bob.established_at = now_unix_secs();
        bob.rx_high_water_mark = 0;

        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        // Send first message
        alice.send_text(&mut alice_w, "Message 1").await.unwrap();
        let frame1 = crate::network::read_frame_impl(&mut bob_r).await.unwrap();
        bob.decrypt_message(&frame1).unwrap();

        // Save the raw frame data for replay attempt
        let frame1_clone = RawFrame {
            version: frame1.version,
            packet_type: frame1.packet_type,
            body: frame1.body.clone(),
        };

        // Send second message (advances counters)
        alice.send_text(&mut alice_w, "Message 2").await.unwrap();
        let frame2 = crate::network::read_frame_impl(&mut bob_r).await.unwrap();
        bob.decrypt_message(&frame2).unwrap();

        // Try to replay frame1 — should be rejected
        let replay_result = bob.decrypt_message(&frame1_clone);
        assert!(matches!(replay_result, Err(SessionError::ReplayDetected { .. })),
            "replayed message should be rejected");
    }

    #[tokio::test]
    async fn test_conversation_meta_roundtrip() {
        init_crypto();

        // Test via send_conversation_meta
        let eph = EphemeralKeypair::generate();
        let eph2 = EphemeralKeypair::generate();
        let alice_keys = eph.client_session_keys(&eph2.public_key_bytes()).unwrap();
        let bob_keys = eph2.server_session_keys(&eph.public_key_bytes()).unwrap();

        let mut alice = Session::new();
        let mut bob = Session::new();
        alice.session_keys = Some(alice_keys);
        alice.state = ConnectionState::Established;
        alice.established_at = now_unix_secs();
        alice.tx_counter = 100;
        bob.session_keys = Some(bob_keys);
        bob.state = ConnectionState::Established;
        bob.established_at = now_unix_secs();
        bob.rx_high_water_mark = 0;

        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        alice.send_conversation_meta(&mut alice_w, "Alice", "Bob").await.unwrap();
        let frame = crate::network::read_frame_impl(&mut bob_r).await.unwrap();
        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let meta: crate::protocol::ConversationMetaData = crate::protocol::deserialize(&plaintext).unwrap();
        assert_eq!(meta.my_display_name, "Alice");
        assert_eq!(meta.your_display_name, "Bob");
    }

    #[tokio::test]
    async fn test_encrypted_message_uses_ratchet() {
        init_crypto();
        let eph = EphemeralKeypair::generate();
        let eph2 = EphemeralKeypair::generate();
        let alice_keys = eph.client_session_keys(&eph2.public_key_bytes()).unwrap();
        let bob_keys = eph2.server_session_keys(&eph.public_key_bytes()).unwrap();

        let mut alice = Session::new();
        let mut bob = Session::new();
        alice.session_keys = Some(alice_keys);
        alice.state = ConnectionState::Established;
        alice.established_at = now_unix_secs();
        alice.tx_counter = 100;
        bob.session_keys = Some(bob_keys);
        bob.state = ConnectionState::Established;
        bob.established_at = now_unix_secs();
        bob.rx_high_water_mark = 0;

        // Save initial keys
        let initial_tx = alice.session_keys.as_ref().unwrap().tx_key;
        let initial_rx = bob.session_keys.as_ref().unwrap().rx_key;

        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        // Send one message — this triggers ratchet on both sides
        alice.send_text(&mut alice_w, "Message 1").await.unwrap();
        let frame = crate::network::read_frame_impl(&mut bob_r).await.unwrap();
        bob.decrypt_message(&frame).unwrap();

        // After ratchet, keys should have changed
        assert_ne!(alice.session_keys.as_ref().unwrap().tx_key, initial_tx,
            "alice tx_key should change after ratchet");
        assert_ne!(bob.session_keys.as_ref().unwrap().rx_key, initial_rx,
            "bob rx_key should change after ratchet (mirrors alice tx)");
    }

    // ═══════════════════════════════════════════════════════════
    // Handshake integration — full success
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_handshake_full_success() {
        init_crypto();
        let (alice_identity, alice_x25519) = make_identities();
        let (bob_identity, bob_x25519) = make_identities();
        let bob_pub = bob_identity.public_key_bytes();

        let (mut alice_io, mut bob_io) = tokio::io::duplex(65536);
        let alice_xp = alice_x25519.public_key_bytes();

        // Alice as initiator
        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator(
                &mut alice_io, &alice_identity, &bob_pub, vec![], alice_xp,
            ).await?;
            Ok::<_, SessionError>(session)
        });

        // Bob as responder
        let frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::HandshakeInit);

        let bob_xp = bob_x25519.public_key_bytes();
        let mut bob_session = Session::new();
        bob_session.handshake_as_responder(
            &mut bob_io, &bob_identity, &frame, vec![], bob_xp,
        ).await.unwrap();

        assert_eq!(bob_session.state, ConnectionState::Established);
        assert!(bob_session.session_keys.is_some());

        let alice_session = alice.await.unwrap().unwrap();
        assert_eq!(alice_session.state, ConnectionState::Established);
        assert!(alice_session.session_keys.is_some());
    }

    #[tokio::test]
    async fn test_handshake_with_candidates() {
        init_crypto();
        let (alice_identity, alice_x25519) = make_identities();
        let (bob_identity, bob_x25519) = make_identities();
        let bob_pub = bob_identity.public_key_bytes();

        let candidates = vec![WireCandidate {
            address: "192.168.1.5:12345".to_string(),
            candidate_type: 0,
            relay_id: None,
        }];
        let alice_candidates = candidates.clone(); // for the spawn closure

        let (mut alice_io, mut bob_io) = tokio::io::duplex(65536);
        let alice_xp = alice_x25519.public_key_bytes();

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator(
                &mut alice_io, &alice_identity, &bob_pub, alice_candidates, alice_xp,
            ).await?;
            Ok::<_, SessionError>(session)
        });

        let frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::HandshakeInit);

        let bob_xp = bob_x25519.public_key_bytes();
        let mut bob_session = Session::new();
        bob_session.handshake_as_responder(
            &mut bob_io, &bob_identity, &frame, candidates, bob_xp,
        ).await.unwrap();

        assert_eq!(bob_session.peer_candidates.len(), 1,
            "responder should have initiator's candidates");

        let alice_session = alice.await.unwrap().unwrap();
        assert_eq!(alice_session.peer_candidates.len(), 1,
            "initiator should have responder's candidates");
    }

    // ═══════════════════════════════════════════════════════════
    // Handshake — initiator failure modes
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_initiator_rejects_wrong_packet_type() {
        init_crypto();
        let (alice_identity, alice_x25519) = make_identities();
        let (bob_identity, _bob_x25519) = make_identities();
        let bob_pub = bob_identity.public_key_bytes();

        let (mut alice_io, mut peer_io) = tokio::io::duplex(65536);
        let alice_xp = alice_x25519.public_key_bytes();

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator(
                &mut alice_io, &alice_identity, &bob_pub, vec![], alice_xp,
            ).await
        });

        let frame = network::read_frame_impl(&mut peer_io).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::HandshakeInit);
        network::write_frame(&mut peer_io, PacketType::Heartbeat, &[]).await.unwrap();

        let result = alice.await.unwrap();
        assert!(matches!(result, Err(SessionError::HandshakeFailed(_))),
            "expected HandshakeFailed for wrong packet type, got: {:?}", result);
    }

    #[tokio::test]
    async fn test_initiator_rejects_version_mismatch() {
        init_crypto();
        let (alice_identity, alice_x25519) = make_identities();
        let (bob_identity, _bob_x25519) = make_identities();
        let bob_pub = bob_identity.public_key_bytes();

        let (mut alice_io, mut peer_io) = tokio::io::duplex(65536);
        let alice_xp = alice_x25519.public_key_bytes();

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator(
                &mut alice_io, &alice_identity, &bob_pub, vec![], alice_xp,
            ).await
        });

        let frame = network::read_frame_impl(&mut peer_io).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::HandshakeInit);

        let bad_response = HandshakeResponse {
            version: 0xFC,
            ephemeral_pub: [0xAA; 32],
            identity_pub: bob_pub,
            x25519_identity_pub: [0u8; 32],
            timestamp: 12345,
            signature: vec![0xBB; 64],
            candidates: vec![],
        };
        let body = protocol::serialize(&bad_response).unwrap();
        network::write_frame(&mut peer_io, PacketType::HandshakeResponse, &body).await.unwrap();

        let result = alice.await.unwrap();
        assert!(matches!(result, Err(SessionError::HandshakeFailed(_))),
            "expected HandshakeFailed for version mismatch, got: {:?}", result);
    }

    #[tokio::test]
    async fn test_initiator_rejects_bad_signature() {
        init_crypto();
        let (alice_identity, alice_x25519) = make_identities();
        let (bob_identity, _bob_x25519) = make_identities();
        let bob_pub = bob_identity.public_key_bytes();

        let (mut alice_io, mut peer_io) = tokio::io::duplex(65536);
        let alice_xp = alice_x25519.public_key_bytes();

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator(
                &mut alice_io, &alice_identity, &bob_pub, vec![], alice_xp,
            ).await
        });

        let frame = network::read_frame_impl(&mut peer_io).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::HandshakeInit);

        let bad_response = HandshakeResponse {
            version: PROTOCOL_VERSION,
            ephemeral_pub: [0xAA; 32],
            identity_pub: bob_pub,
            x25519_identity_pub: [0u8; 32],
            timestamp: 12345,
            signature: vec![0xCC; 64],
            candidates: vec![],
        };
        let body = protocol::serialize(&bad_response).unwrap();
        network::write_frame(&mut peer_io, PacketType::HandshakeResponse, &body).await.unwrap();

        let result = alice.await.unwrap();
        assert!(matches!(result, Err(SessionError::HandshakeFailed(_))),
            "expected HandshakeFailed for bad signature, got: {:?}", result);
    }

    #[tokio::test]
    async fn test_initiator_rejects_identity_mismatch() {
        init_crypto();
        let (alice_identity, alice_x25519) = make_identities();
        let (bob_identity, _bob_x25519) = make_identities();
        let bob_pub = bob_identity.public_key_bytes();
        let (wrong_key, _wrong_x25519) = make_identities();

        let (mut alice_io, mut peer_io) = tokio::io::duplex(65536);
        let alice_xp = alice_x25519.public_key_bytes();

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator(
                &mut alice_io, &alice_identity, &bob_pub, vec![], alice_xp,
            ).await
        });

        let frame = network::read_frame_impl(&mut peer_io).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::HandshakeInit);

        let bad_response = HandshakeResponse {
            version: PROTOCOL_VERSION,
            ephemeral_pub: [0xAA; 32],
            identity_pub: wrong_key.public_key_bytes(),
            x25519_identity_pub: [0u8; 32],
            timestamp: 12345,
            signature: vec![0xBB; 64],
            candidates: vec![],
        };
        let body = protocol::serialize(&bad_response).unwrap();
        network::write_frame(&mut peer_io, PacketType::HandshakeResponse, &body).await.unwrap();

        let result = alice.await.unwrap();
        assert!(matches!(result, Err(SessionError::HandshakeFailed(_))),
            "expected HandshakeFailed for identity mismatch, got: {:?}", result);
    }

    // ═══════════════════════════════════════════════════════════
    // Handshake — responder failure modes
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_responder_rejects_version_mismatch() {
        init_crypto();
        let (bob_identity, bob_x25519) = make_identities();
        let (alice_identity, _alice_x25519) = make_identities();

        let eph = EphemeralKeypair::generate();
        let bad_init = HandshakeInit {
            version: 0xFC,
            ephemeral_pub: eph.public_key_bytes(),
            identity_pub: alice_identity.public_key_bytes(),
            x25519_identity_pub: [0u8; 32],
            used_opk: None,
            timestamp: 12345,
            signature: vec![0xDD; 64],
            candidates: vec![],
        };
        let body = protocol::serialize(&bad_init).unwrap();
        let frame = RawFrame {
            version: PROTOCOL_VERSION,
            packet_type: PacketType::HandshakeInit,
            body,
        };

        let (mut bob_io, _peer_io) = tokio::io::duplex(65536);
        let bob_xp = bob_x25519.public_key_bytes();
        let mut session = Session::new();
        let result = session.handshake_as_responder(
            &mut bob_io, &bob_identity, &frame, vec![], bob_xp,
        ).await;

        assert!(matches!(result, Err(SessionError::HandshakeFailed(_))),
            "expected HandshakeFailed for version mismatch, got: {:?}", result);
    }

    #[tokio::test]
    async fn test_responder_rejects_bad_signature() {
        init_crypto();
        let (bob_identity, bob_x25519) = make_identities();
        let (alice_identity, _alice_x25519) = make_identities();
        let (wrong_signer, _wrong_x25519) = make_identities();

        let eph = EphemeralKeypair::generate();
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&eph.public_key_bytes());
        sign_data.extend_from_slice(&12345u64.to_be_bytes());
        let signature = wrong_signer.sign(&sign_data);

        let bad_init = HandshakeInit {
            version: PROTOCOL_VERSION,
            ephemeral_pub: eph.public_key_bytes(),
            identity_pub: alice_identity.public_key_bytes(),
            x25519_identity_pub: [0u8; 32],
            used_opk: None,
            timestamp: 12345,
            signature,
            candidates: vec![],
        };
        let body = protocol::serialize(&bad_init).unwrap();
        let frame = RawFrame {
            version: PROTOCOL_VERSION,
            packet_type: PacketType::HandshakeInit,
            body,
        };

        let (mut bob_io, _peer_io) = tokio::io::duplex(65536);
        let bob_xp = bob_x25519.public_key_bytes();
        let mut session = Session::new();
        let result = session.handshake_as_responder(
            &mut bob_io, &bob_identity, &frame, vec![], bob_xp,
        ).await;

        assert!(matches!(result, Err(SessionError::HandshakeFailed(_))),
            "expected HandshakeFailed for bad signature, got: {:?}", result);
    }

    #[tokio::test]
    async fn test_responder_rejects_bad_verification() {
        init_crypto();
        let (bob_identity, bob_x25519) = make_identities();
        let (alice_identity, _alice_x25519) = make_identities();

        let eph = EphemeralKeypair::generate();
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&eph.public_key_bytes());
        sign_data.extend_from_slice(&12345u64.to_be_bytes());
        let signature = alice_identity.sign(&sign_data);

        let init = HandshakeInit {
            version: PROTOCOL_VERSION,
            ephemeral_pub: eph.public_key_bytes(),
            identity_pub: alice_identity.public_key_bytes(),
            x25519_identity_pub: [0u8; 32],
            used_opk: None,
            timestamp: 12345,
            signature,
            candidates: vec![],
        };
        let body = protocol::serialize(&init).unwrap();
        let frame = RawFrame {
            version: PROTOCOL_VERSION,
            packet_type: PacketType::HandshakeInit,
            body,
        };

        let (mut bob_io, mut peer_io) = tokio::io::duplex(65536);
        let bob_xp = bob_x25519.public_key_bytes();

        let bob = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_responder(
                &mut bob_io, &bob_identity, &frame, vec![], bob_xp,
            ).await
        });

        let resp_frame = network::read_frame_impl(&mut peer_io).await.unwrap();
        assert_eq!(resp_frame.packet_type, PacketType::HandshakeResponse);

        let bad_complete = HandshakeComplete {
            encrypted_verify: vec![0xDE, 0xAD, 0xBE, 0xEF],
            nonce: vec![0u8; 24],
        };
        let complete_body = protocol::serialize(&bad_complete).unwrap();
        network::write_frame(&mut peer_io, PacketType::HandshakeComplete, &complete_body)
            .await.unwrap();

        let result = bob.await.unwrap();
        assert!(matches!(result, Err(SessionError::HandshakeFailed(_))),
            "expected HandshakeFailed for bad verification, got: {:?}", result);
    }

    // ═══════════════════════════════════════════════════════════
    // State machine edge cases
    // ═══════════════════════════════════════════════════════════

    #[tokio::test]
    async fn test_handshake_from_wrong_state() {
        init_crypto();
        let (identity, x25519) = make_identities();
        let (peer_identity, _peer_x25519) = make_identities();
        let peer_pub = peer_identity.public_key_bytes();
        let xp = x25519.public_key_bytes();

        let (mut io, _other) = tokio::io::duplex(65536);
        let mut session = Session::new();
        session.state = ConnectionState::Established;

        let result = session.handshake_as_initiator(
            &mut io, &identity, &peer_pub, vec![], xp,
        ).await;

        assert!(result.is_err(),
            "handshake from Established state should fail");
    }

    // ═══════════════════════════════════════════════════════════
    // X3DH + Double Ratchet Integration Tests
    // ═══════════════════════════════════════════════════════════

    /// Helper: build Bob's PrekeyBundle for X3DH integration tests.
    /// Returns (spk, bundle) — the caller needs spk secret key for responder handshake.
    fn make_bob_prekey(
        bob_x25519: &crate::crypto::X25519IdentityKeypair,
        bob_identity: &IdentityKeypair,
    ) -> (crate::crypto::EphemeralKeypair, crate::crypto::PrekeyBundle) {
        let spk = crate::crypto::EphemeralKeypair::generate();
        let sig = bob_identity.sign(&spk.public_key_bytes());
        let bundle = crate::crypto::PrekeyBundle {
            identity_key: bob_x25519.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: sig,
            one_time_prekey: None,
        };
        (spk, bundle)
    }

    #[tokio::test]
    async fn test_x3dh_full_handshake_text_message() {
        init_crypto();
        let (alice_id, alice_x25519) = make_identities();
        let (bob_id, bob_x25519) = make_identities();
        let bob_pub = bob_id.public_key_bytes();
        let (bob_spk, bob_bundle) = make_bob_prekey(&bob_x25519, &bob_id);

        let (mut alice_io, mut bob_io) = tokio::io::duplex(65536);

        // Alice as initiator (background)
        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator_x3dh(
                &mut alice_io, &alice_id, &alice_x25519, &bob_pub, &bob_bundle, vec![],
            ).await?;
            // Send a text message over the DR path
            let msg_id = session.send_text(&mut alice_io, "Hello via X3DH+DR!").await?;
            Ok::<_, SessionError>((session, msg_id))
        });

        // Bob as responder
        let init_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        assert_eq!(init_frame.packet_type, PacketType::X3DHHandshakeInit);

        let mut bob_session = Session::new();
        bob_session.handshake_as_responder_x3dh(
            &mut bob_io, &bob_id, &bob_x25519, &bob_spk, &init_frame, vec![],
        ).await.unwrap();
        assert_eq!(bob_session.state, ConnectionState::Established);
        assert!(bob_session.ratchet.is_some(), "Bob should have DR after X3DH");

        // Bob reads Alice's text message
        let msg_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        assert_eq!(msg_frame.packet_type, PacketType::EncryptedMessage);
        let body = bob_session.decrypt_message(&msg_frame).unwrap();
        match body {
            MessageBody::Text { ref content, .. } => assert_eq!(content, "Hello via X3DH+DR!"),
            ref other => panic!("expected Text, got {:?}", other),
        }

        let (alice_session, _msg_id) = alice.await.unwrap().unwrap();
        assert_eq!(alice_session.state, ConnectionState::Established);
        assert!(alice_session.ratchet.is_some(), "Alice should have DR after X3DH");
    }

    #[tokio::test]
    async fn test_x3dh_full_handshake_file_transfer_dr_path() {
        init_crypto();
        let (alice_id, alice_x25519) = make_identities();
        let (bob_id, bob_x25519) = make_identities();
        let bob_pub = bob_id.public_key_bytes();
        let (bob_spk, bob_bundle) = make_bob_prekey(&bob_x25519, &bob_id);

        let (mut alice_io, mut bob_io) = tokio::io::duplex(65536);

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator_x3dh(
                &mut alice_io, &alice_id, &alice_x25519, &bob_pub, &bob_bundle, vec![],
            ).await?;
            session.send_file_request(
                &mut alice_io, "integ-test-001", "secret.pdf", 524288, 8, vec![0xAB; 32],
            ).await?;
            Ok::<_, SessionError>(session)
        });

        let init_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        let mut bob_session = Session::new();
        bob_session.handshake_as_responder_x3dh(
            &mut bob_io, &bob_id, &bob_x25519, &bob_spk, &init_frame, vec![],
        ).await.unwrap();

        // Bob reads the file request — typed frame via DR path
        let req_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        assert_eq!(req_frame.packet_type, PacketType::FileTransferRequest);
        let plaintext = bob_session.decrypt_typed_frame(&req_frame).unwrap();
        let req: crate::protocol::FileTransferRequestData =
            crate::protocol::deserialize(&plaintext).unwrap();
        assert_eq!(req.transfer_id, "integ-test-001");
        assert_eq!(req.filename, "secret.pdf");
        assert_eq!(req.total_size, 524288);
        assert_eq!(req.total_chunks, 8);

        alice.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_x3dh_full_handshake_conversation_meta_dr_path() {
        init_crypto();
        let (alice_id, alice_x25519) = make_identities();
        let (bob_id, bob_x25519) = make_identities();
        let bob_pub = bob_id.public_key_bytes();
        let (bob_spk, bob_bundle) = make_bob_prekey(&bob_x25519, &bob_id);

        let (mut alice_io, mut bob_io) = tokio::io::duplex(65536);

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator_x3dh(
                &mut alice_io, &alice_id, &alice_x25519, &bob_pub, &bob_bundle, vec![],
            ).await?;
            session.send_conversation_meta(&mut alice_io, "Alice", "Bob").await?;
            Ok::<_, SessionError>(session)
        });

        let init_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        let mut bob_session = Session::new();
        bob_session.handshake_as_responder_x3dh(
            &mut bob_io, &bob_id, &bob_x25519, &bob_spk, &init_frame, vec![],
        ).await.unwrap();

        let meta_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        assert_eq!(meta_frame.packet_type, PacketType::ConversationMeta);
        let plaintext = bob_session.decrypt_typed_frame(&meta_frame).unwrap();
        let meta: crate::protocol::ConversationMetaData =
            crate::protocol::deserialize(&plaintext).unwrap();
        assert_eq!(meta.my_display_name, "Alice");
        assert_eq!(meta.your_display_name, "Bob");

        alice.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_x3dh_dh_ratchet_during_message_exchange() {
        init_crypto();
        let (alice_id, alice_x25519) = make_identities();
        let (bob_id, bob_x25519) = make_identities();
        let bob_pub = bob_id.public_key_bytes();
        let (bob_spk, bob_bundle) = make_bob_prekey(&bob_x25519, &bob_id);

        let (mut alice_io, mut bob_io) = tokio::io::duplex(65536);

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator_x3dh(
                &mut alice_io, &alice_id, &alice_x25519, &bob_pub, &bob_bundle, vec![],
            ).await?;
            // Send 105 messages to trigger DH ratchet at 100
            for i in 0..105 {
                let msg = format!("Message {}", i);
                session.send_text(&mut alice_io, &msg).await?;
            }
            Ok::<_, SessionError>(session)
        });

        let init_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        let mut bob_session = Session::new();
        bob_session.handshake_as_responder_x3dh(
            &mut bob_io, &bob_id, &bob_x25519, &bob_spk, &init_frame, vec![],
        ).await.unwrap();

        // Verify all 105 messages decrypt correctly (including across DH ratchet)
        for i in 0..105 {
            let frame = network::read_frame_impl(&mut bob_io).await.unwrap();
            let body = bob_session.decrypt_message(&frame).unwrap();
            match body {
                MessageBody::Text { ref content, .. } => {
                    assert_eq!(content, &format!("Message {}", i));
                }
                ref other => panic!("expected Text msg {}, got {:?}", i, other),
            }
        }

        alice.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_x3dh_replay_detected_integration() {
        init_crypto();
        let (alice_id, alice_x25519) = make_identities();
        let (bob_id, bob_x25519) = make_identities();
        let bob_pub = bob_id.public_key_bytes();
        let (bob_spk, bob_bundle) = make_bob_prekey(&bob_x25519, &bob_id);

        let (mut alice_io, mut bob_io) = tokio::io::duplex(65536);

        let alice = tokio::spawn(async move {
            let mut session = Session::new();
            session.handshake_as_initiator_x3dh(
                &mut alice_io, &alice_id, &alice_x25519, &bob_pub, &bob_bundle, vec![],
            ).await?;
            session.send_text(&mut alice_io, "Message 1").await?;
            session.send_text(&mut alice_io, "Message 2").await?;
            Ok::<_, SessionError>(session)
        });

        let init_frame = network::read_frame_impl(&mut bob_io).await.unwrap();
        let mut bob_session = Session::new();
        bob_session.handshake_as_responder_x3dh(
            &mut bob_io, &bob_id, &bob_x25519, &bob_spk, &init_frame, vec![],
        ).await.unwrap();

        // Read both messages
        let f1 = network::read_frame_impl(&mut bob_io).await.unwrap();
        bob_session.decrypt_message(&f1).unwrap();
        let f2 = network::read_frame_impl(&mut bob_io).await.unwrap();
        bob_session.decrypt_message(&f2).unwrap();

        // Replay f1 — should be rejected by DR chain advancement
        let replay = bob_session.decrypt_message(&f1);
        assert!(replay.is_err(), "replayed message should fail");

        alice.await.unwrap().unwrap();
    }

    // ═══════════════════════════════════════════════════════════
    // Typed-Frame Double Ratchet Tests (direct setup)
    // ═══════════════════════════════════════════════════════════

    fn make_session_with_ratchet() -> (Session, Session) {
        init_crypto();
        // Build a DR pair from X3DH key exchange
        let ik_alice = crate::crypto::X25519IdentityKeypair::generate();
        let ik_bob = crate::crypto::X25519IdentityKeypair::generate();
        let ek_alice = crate::crypto::EphemeralKeypair::generate();
        let spk = crate::crypto::EphemeralKeypair::generate();
        let bundle = crate::crypto::PrekeyBundle {
            identity_key: ik_bob.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: vec![0xAAu8; 64],
            one_time_prekey: None,
        };
        let x3dh_alice = crate::crypto::x3dh_initiate(&ik_alice, &ek_alice, &bundle).unwrap();
        let dh_alice = crate::crypto::EphemeralKeypair::generate();
        let dh_bob = crate::crypto::EphemeralKeypair::generate();
        let alice_pub = dh_alice.public_key_bytes();
        let bob_pub = dh_bob.public_key_bytes();

        let mut alice = Session::new();
        alice.ratchet = Some(crate::crypto::DoubleRatchet::new(
            x3dh_alice, dh_alice, bob_pub, true,
        ));
        alice.state = ConnectionState::Established;
        alice.established_at = now_unix_secs();
        // Set peer identity key for DR AAD
        alice.peer_identity_pub = [0xBBu8; 32];

        let bob_x3dh = crate::crypto::x3dh_respond(
            &ik_bob, &spk, None,
            &ek_alice.public_key_bytes(),
            &ik_alice.public_key_bytes(),
        ).unwrap();
        let mut bob = Session::new();
        bob.ratchet = Some(crate::crypto::DoubleRatchet::new(
            bob_x3dh, dh_bob, alice_pub, false,
        ));
        bob.state = ConnectionState::Established;
        bob.established_at = now_unix_secs();
        bob.peer_identity_pub = [0xBBu8; 32];

        (alice, bob)
    }

    #[tokio::test]
    async fn test_dr_file_transfer_request_roundtrip() {
        let (mut alice, mut bob) = make_session_with_ratchet();
        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        alice.send_file_request(
            &mut alice_w, "dr-file-001", "document.pdf", 1048576, 16, vec![0xCD; 32],
        ).await.unwrap();

        let frame = network::read_frame_impl(&mut bob_r).await.unwrap();
        // Verify it has a DR header
        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body).unwrap();
        assert!(envelope.dr_header.is_some(), "should use DR path");

        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let req: crate::protocol::FileTransferRequestData =
            protocol::deserialize(&plaintext).unwrap();
        assert_eq!(req.transfer_id, "dr-file-001");
        assert_eq!(req.filename, "document.pdf");
        assert_eq!(req.total_size, 1048576);
    }

    #[tokio::test]
    async fn test_dr_file_accept_roundtrip() {
        let (mut alice, mut bob) = make_session_with_ratchet();
        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        alice.send_file_accept(&mut alice_w, "dr-accept-001").await.unwrap();

        let frame = network::read_frame_impl(&mut bob_r).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::FileTransferAccept);
        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body).unwrap();
        assert!(envelope.dr_header.is_some());

        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let accept: crate::protocol::FileTransferAcceptData =
            protocol::deserialize(&plaintext).unwrap();
        assert_eq!(accept.transfer_id, "dr-accept-001");
    }

    #[tokio::test]
    async fn test_dr_file_reject_roundtrip() {
        let (mut alice, mut bob) = make_session_with_ratchet();
        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        alice.send_file_reject(&mut alice_w, "dr-reject-001").await.unwrap();

        let frame = network::read_frame_impl(&mut bob_r).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::FileTransferReject);
        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body).unwrap();
        assert!(envelope.dr_header.is_some());

        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let reject: crate::protocol::FileTransferRejectData =
            protocol::deserialize(&plaintext).unwrap();
        assert_eq!(reject.transfer_id, "dr-reject-001");
    }

    #[tokio::test]
    async fn test_dr_file_chunk_roundtrip() {
        let (mut alice, mut bob) = make_session_with_ratchet();
        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        let chunk_data = vec![0x42u8; 1024];
        alice.send_file_chunk(
            &mut alice_w, "dr-chunk-001", 0, chunk_data.clone(), vec![0xEF; 32],
        ).await.unwrap();

        let frame = network::read_frame_impl(&mut bob_r).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::FileTransferChunk);
        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body).unwrap();
        assert!(envelope.dr_header.is_some());

        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let chunk: crate::protocol::FileTransferChunkData =
            protocol::deserialize(&plaintext).unwrap();
        assert_eq!(chunk.transfer_id, "dr-chunk-001");
        assert_eq!(chunk.chunk_index, 0);
        assert_eq!(chunk.data, chunk_data);
    }

    #[tokio::test]
    async fn test_dr_file_complete_roundtrip() {
        let (mut alice, mut bob) = make_session_with_ratchet();
        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        alice.send_file_complete(&mut alice_w, "dr-complete-001").await.unwrap();

        let frame = network::read_frame_impl(&mut bob_r).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::FileTransferComplete);
        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body).unwrap();
        assert!(envelope.dr_header.is_some());

        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let complete: crate::protocol::FileTransferCompleteData =
            protocol::deserialize(&plaintext).unwrap();
        assert_eq!(complete.transfer_id, "dr-complete-001");
    }

    #[tokio::test]
    async fn test_dr_conversation_meta_roundtrip() {
        let (mut alice, mut bob) = make_session_with_ratchet();
        let (mut alice_w, mut bob_r) = tokio::io::duplex(65536);

        alice.send_conversation_meta(&mut alice_w, "AliceDR", "BobDR").await.unwrap();

        let frame = network::read_frame_impl(&mut bob_r).await.unwrap();
        assert_eq!(frame.packet_type, PacketType::ConversationMeta);
        let envelope: EncryptedEnvelope = protocol::deserialize(&frame.body).unwrap();
        assert!(envelope.dr_header.is_some(), "should use DR path");

        let plaintext = bob.decrypt_typed_frame(&frame).unwrap();
        let meta: crate::protocol::ConversationMetaData =
            protocol::deserialize(&plaintext).unwrap();
        assert_eq!(meta.my_display_name, "AliceDR");
        assert_eq!(meta.your_display_name, "BobDR");
    }

    #[tokio::test]
    async fn test_dr_decrypt_typed_frame_without_ratchet_fails() {
        init_crypto();
        // Legacy session without ratchet
        let mut session = Session::new();
        session.state = ConnectionState::Established;
        session.established_at = now_unix_secs();

        // Construct a DR envelope manually
        let envelope = EncryptedEnvelope {
            nonce: vec![0u8; 24],
            counter: 0,
            ciphertext: vec![0u8; 32],
            dr_header: Some(DRHeader {
                ratchet_key: None,
                previous_chain_length: 0,
                message_number: 0,
            }),
        };
        let frame = RawFrame {
            version: PROTOCOL_VERSION,
            packet_type: PacketType::FileTransferRequest,
            body: protocol::serialize(&envelope).unwrap(),
        };

        let result = session.decrypt_typed_frame(&frame);
        assert!(matches!(result, Err(SessionError::InvalidState)),
            "expected InvalidState without ratchet, got {:?}", result);
    }
}
