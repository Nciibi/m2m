/// M2M — Identity Module
///
/// Manages long-term identity, invite creation/validation, and fingerprint display.
/// Identity is a single Ed25519 keypair generated on first launch.
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto::{self, IdentityKeypair};
use crate::protocol::{
    self, InvitePayload, SignedInvite, CLOCK_SKEW_TOLERANCE_SECS, INVITE_FLAG_LISTENER,
    INVITE_FLAG_ONE_TIME, INVITE_PREFIX, MAX_ADDRESS_HINT_LENGTH, MAX_INVITE_LENGTH,
    MAX_INVITE_VALIDITY_SECS, PROTOCOL_VERSION,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("crypto error: {0}")]
    Crypto(#[from] crypto::CryptoError),
    #[error("protocol error: {0}")]
    Protocol(#[from] protocol::ProtocolError),
    #[error("invite expired")]
    InviteExpired,
    #[error("invite created in the future")]
    InviteFutureTimestamp,
    #[error("invite validity window too large")]
    InviteValidityTooLarge,
    #[error("invite signature invalid")]
    InviteSignatureInvalid,
    #[error("invite format invalid: {0}")]
    InviteFormatInvalid(String),
    #[expect(dead_code, reason = "Reserved error variant for one-time invites")]
    #[error("invite already consumed")]
    InviteAlreadyConsumed,
    #[error("address hint too long")]
    AddressHintTooLong,
}

/// Get the current Unix timestamp in seconds.
fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs()
}

/// Create a signed invite link.
///
/// - `identity`: the local identity keypair (signs the invite)
/// - `address_hint`: IP:port or hostname:port where we're listening
/// - `validity_secs`: how long the invite is valid
/// - `one_time`: if true, invite can only be used once
/// - `candidates`: network candidates for ICE-Lite
/// - `prekey_bundle`: optional X3DH prekey bundle (identity_key, SPK, OPK)
pub fn create_invite(
    identity: &IdentityKeypair,
    address_hint: &str,
    validity_secs: u64,
    one_time: bool,
    candidates: Vec<crate::protocol::WireCandidate>,
    prekey_bundle: Option<&crate::crypto::PrekeyBundle>,
) -> Result<String, IdentityError> {
    if address_hint.len() > MAX_ADDRESS_HINT_LENGTH {
        return Err(IdentityError::AddressHintTooLong);
    }
    if validity_secs > MAX_INVITE_VALIDITY_SECS {
        return Err(IdentityError::InviteValidityTooLarge);
    }

    let now = now_unix_secs();
    let nonce = crypto::random_bytes(16);

    let mut flags: u8 = INVITE_FLAG_LISTENER; // We are the listener
    if one_time {
        flags |= INVITE_FLAG_ONE_TIME;
    }

    let (x25519_pub, spk, spk_sig, opk) = match prekey_bundle {
        Some(b) => (b.identity_key, b.signed_prekey, b.signed_prekey_sig.clone(), b.one_time_prekey),
        None => ([0u8; 32], [0u8; 32], vec![], None),
    };
    let payload = InvitePayload {
        version: PROTOCOL_VERSION,
        identity_pub: identity.public_key_bytes(),
        x25519_identity_pub: x25519_pub,
        signed_prekey: spk,
        signed_prekey_sig: spk_sig,
        one_time_prekey: opk,
        address_hint: address_hint.to_string(),
        created_at: now,
        expires_at: now + validity_secs,
        nonce,
        flags,
        candidates,
    };

    // Serialize payload for signing
    let payload_bytes = protocol::serialize(&payload)?;

    // Sign the serialized payload with identity key
    let signature = identity.sign(&payload_bytes);

    let signed_invite = SignedInvite { payload, signature };

    // Serialize the full signed invite
    let invite_bytes = protocol::serialize(&signed_invite)?;

    // Encode as base64url and prepend protocol prefix
    let encoded = URL_SAFE_NO_PAD.encode(&invite_bytes);
    let invite_string = format!("{}{}", INVITE_PREFIX, encoded);

    if invite_string.len() > MAX_INVITE_LENGTH {
        return Err(IdentityError::InviteFormatInvalid(
            "serialized invite exceeds maximum length".to_string(),
        ));
    }

    Ok(invite_string)
}

/// Parse and validate an invite string.
/// Returns the validated invite data on success.
///
/// Validation steps:
/// 1. Decode base64url
/// 2. Deserialize MessagePack
/// 3. Check version
/// 4. Check expiry
/// 5. Check clock skew
/// 6. Check validity window
/// 7. Verify Ed25519 signature
/// 8. Validate address hint
pub fn validate_invite(invite_str: &str) -> Result<SignedInvite, IdentityError> {
    // Step 0: Check prefix
    let encoded = invite_str.strip_prefix(INVITE_PREFIX).ok_or_else(|| {
        IdentityError::InviteFormatInvalid("missing m2m:// prefix".to_string())
    })?;

    if invite_str.len() > MAX_INVITE_LENGTH {
        return Err(IdentityError::InviteFormatInvalid(
            "invite string too long".to_string(),
        ));
    }

    // Step 1: Decode base64url
    let invite_bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| IdentityError::InviteFormatInvalid(format!("base64 decode error: {e}")))?;

    // Step 2: Deserialize
    let signed: SignedInvite = protocol::deserialize(&invite_bytes)?;

    // Step 3: Check version
    if let Err(e) = protocol::validate_version(signed.payload.version) {
        return Err(IdentityError::InviteFormatInvalid(format!(
            "unsupported version: {e}"
        )));
    }

    let now = now_unix_secs();

    // Step 4: Check expiry
    if signed.payload.expires_at <= now {
        return Err(IdentityError::InviteExpired);
    }

    // Step 5: Check clock skew (reject if created too far in the future)
    if signed.payload.created_at > now + CLOCK_SKEW_TOLERANCE_SECS {
        return Err(IdentityError::InviteFutureTimestamp);
    }

    // Step 6: Check validity window
    let validity_window = signed
        .payload
        .expires_at
        .saturating_sub(signed.payload.created_at);
    if validity_window > MAX_INVITE_VALIDITY_SECS {
        return Err(IdentityError::InviteValidityTooLarge);
    }

    // Step 7: Verify signature
    let payload_bytes =
        protocol::serialize(&signed.payload)?;
    crypto::verify_signature(
        &signed.payload.identity_pub,
        &payload_bytes,
        &signed.signature,
    )
    .map_err(|_| IdentityError::InviteSignatureInvalid)?;

    // Step 8: Validate address hint
    if signed.payload.address_hint.is_empty() {
        return Err(IdentityError::InviteFormatInvalid(
            "empty address hint".to_string(),
        ));
    }
    if signed.payload.address_hint.len() > MAX_ADDRESS_HINT_LENGTH {
        return Err(IdentityError::AddressHintTooLong);
    }

    Ok(signed)
}

/// Check if an invite is one-time use.
pub fn is_one_time(invite: &SignedInvite) -> bool {
    invite.payload.flags & INVITE_FLAG_ONE_TIME != 0
}

/// Check if the inviter is the TCP listener.
#[expect(dead_code, reason = "Reserved for listener role detection")]
pub fn is_listener(invite: &SignedInvite) -> bool {
    invite.payload.flags & INVITE_FLAG_LISTENER != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;

    fn init() {
        let _ = crypto::init();
    }

    fn make_identity() -> crypto::IdentityKeypair {
        crypto::IdentityKeypair::generate().unwrap()
    }

    fn short_invite(identity: &crypto::IdentityKeypair) -> String {
        create_invite(identity, "192.168.1.5:12345", 3600, false, vec![], None).unwrap()
    }

    // ─── Creation + validation round-trip ─────────────────────

    #[test]
    fn test_create_invite_roundtrip() {
        init();
        let identity = make_identity();
        let invite_str = short_invite(&identity);

        let parsed = validate_invite(&invite_str).unwrap();
        assert_eq!(parsed.payload.identity_pub, identity.public_key_bytes());
        assert_eq!(parsed.payload.address_hint, "192.168.1.5:12345");
        assert_eq!(parsed.payload.version, PROTOCOL_VERSION);
        assert!(!is_one_time(&parsed));
        assert!(is_listener(&parsed));
        assert!(parsed.payload.created_at > 0);
        assert!(parsed.payload.expires_at > parsed.payload.created_at);
        assert_eq!(parsed.payload.nonce.len(), 16);
    }

    #[test]
    fn test_one_time_flag_preserved() {
        init();
        let identity = make_identity();
        let invite_str = create_invite(&identity, "1.2.3.4:5678", 3600, true, vec![], None).unwrap();
        let parsed = validate_invite(&invite_str).unwrap();
        assert!(is_one_time(&parsed));
    }

    #[test]
    fn test_invite_with_candidates() {
        init();
        let identity = make_identity();
        let candidates = vec![
            protocol::WireCandidate { address: "10.0.0.1:9000".to_string(), candidate_type: 0, relay_id: None },
            protocol::WireCandidate { address: "1.2.3.4:9001".to_string(), candidate_type: 1, relay_id: None },
        ];
        let invite_str = create_invite(&identity, "5.6.7.8:9000", 3600, false, candidates.clone(), None).unwrap();
        let parsed = validate_invite(&invite_str).unwrap();
        assert_eq!(parsed.payload.candidates.len(), 2);
        assert_eq!(parsed.payload.candidates[0].address, "10.0.0.1:9000");
        assert_eq!(parsed.payload.candidates[1].candidate_type, 1);
    }

    #[test]
    fn test_invite_with_prekey_bundle() {
        init();
        let identity = make_identity();
        let x25519 = crypto::X25519IdentityKeypair::generate();
        let spk = crypto::EphemeralKeypair::generate();
        let spk_sig = identity.sign(&spk.public_key_bytes());
        let bundle = crypto::PrekeyBundle {
            identity_key: x25519.public_key_bytes(),
            signed_prekey: spk.public_key_bytes(),
            signed_prekey_sig: spk_sig.clone(),
            one_time_prekey: None,
        };

        // Test that payload serialization preserves prekey fields
        // (the full invite may exceed MAX_INVITE_LENGTH with bundle data)
        let payload = protocol::InvitePayload {
            version: PROTOCOL_VERSION,
            identity_pub: identity.public_key_bytes(),
            x25519_identity_pub: bundle.identity_key,
            signed_prekey: bundle.signed_prekey,
            signed_prekey_sig: bundle.signed_prekey_sig.clone(),
            one_time_prekey: bundle.one_time_prekey,
            address_hint: "1.2.3.4:5678".to_string(),
            created_at: now_unix_secs(),
            expires_at: now_unix_secs() + 3600,
            nonce: crypto::random_bytes(16),
            flags: protocol::INVITE_FLAG_LISTENER,
            candidates: vec![],
        };
        let payload_bytes = protocol::serialize(&payload).unwrap();
        let decoded: protocol::InvitePayload = protocol::deserialize(&payload_bytes).unwrap();

        assert_eq!(decoded.x25519_identity_pub, x25519.public_key_bytes());
        assert_eq!(decoded.signed_prekey, spk.public_key_bytes());
        assert_eq!(decoded.signed_prekey_sig, spk_sig);
        assert_eq!(decoded.one_time_prekey, None);
    }

    // ─── Expiry and time validation ───────────────────────────

    #[test]
    fn test_expired_invite_rejected() {
        init();
        let identity = make_identity();
        // validity_secs=0 means expires_at == created_at, which fails expires_at <= now
        let invite_str = create_invite(&identity, "1.2.3.4:5678", 0, false, vec![], None).unwrap();
        let result = validate_invite(&invite_str);
        assert!(matches!(result, Err(IdentityError::InviteExpired)));
    }

    #[test]
    fn test_future_invite_rejected() {
        init();
        // We can't easily set created_at in the future via create_invite.
        // Instead we craft an invite manually with a future timestamp.
        let identity = make_identity();
        let far_future = now_unix_secs() + CLOCK_SKEW_TOLERANCE_SECS + 60;
        let payload = protocol::InvitePayload {
            version: PROTOCOL_VERSION,
            identity_pub: identity.public_key_bytes(),
            x25519_identity_pub: [0u8; 32],
            signed_prekey: [0u8; 32],
            signed_prekey_sig: vec![],
            one_time_prekey: None,
            address_hint: "1.2.3.4:5678".to_string(),
            created_at: far_future,
            expires_at: far_future + 3600,
            nonce: crypto::random_bytes(16),
            flags: protocol::INVITE_FLAG_LISTENER,
            candidates: vec![],
        };
        let payload_bytes = protocol::serialize(&payload).unwrap();
        let signature = identity.sign(&payload_bytes);
        let signed = protocol::SignedInvite { payload, signature };
        let invite_bytes = protocol::serialize(&signed).unwrap();
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&invite_bytes);
        let invite_str = format!("{}{}", protocol::INVITE_PREFIX, encoded);

        let result = validate_invite(&invite_str);
        assert!(matches!(result, Err(IdentityError::InviteFutureTimestamp)));
    }

    // ─── Tamper detection ─────────────────────────────────────

    #[test]
    fn test_tampered_invite_rejected() {
        init();
        let identity = make_identity();
        let invite_str = short_invite(&identity);

        // Flip a byte in the base64 portion of the invite (skip the m2m:// prefix)
        let prefix = protocol::INVITE_PREFIX;
        let base64_part = &invite_str[prefix.len()..];
        let mut bytes = base64_part.as_bytes().to_vec();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        // Reconstruct with the original prefix + tampered base64
        let tampered = format!("{}{}", prefix, String::from_utf8_lossy(&bytes));

        let result = validate_invite(&tampered);
        assert!(result.is_err(), "tampered invite should be rejected");
    }

    #[test]
    fn test_tampered_signature_rejected() {
        init();
        // Create a valid invite, then tamper a payload byte after encoding
        let identity = make_identity();
        let invite_str = short_invite(&identity);
        let parsed = validate_invite(&invite_str).unwrap();

        // Modify a payload field and re-encode with wrong sig
        let mut payload = parsed.payload.clone();
        payload.address_hint = "999.999.999.999:9999".to_string();
        let _payload_bytes = protocol::serialize(&payload).unwrap();
        let wrong_sig = vec![0xCC; 64]; // bogus signature
        let bad_signed = protocol::SignedInvite { payload, signature: wrong_sig };
        let bad_bytes = protocol::serialize(&bad_signed).unwrap();
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bad_bytes);
        let bad_str = format!("{}{}", protocol::INVITE_PREFIX, encoded);

        let result = validate_invite(&bad_str);
        assert!(matches!(result, Err(IdentityError::InviteSignatureInvalid)));
    }

    // ─── Validation error cases ───────────────────────────────

    #[test]
    fn test_missing_prefix_rejected() {
        init();
        let result = validate_invite("bogus-invite-without-prefix");
        assert!(matches!(result, Err(IdentityError::InviteFormatInvalid(_))));
    }

    #[test]
    fn test_malformed_base64_rejected() {
        init();
        let result = validate_invite(&format!("{}!!!not-base64!!!", protocol::INVITE_PREFIX));
        assert!(matches!(result, Err(IdentityError::InviteFormatInvalid(_))));
    }

    #[test]
    fn test_empty_address_hint_rejected() {
        init();
        let identity = make_identity();
        // create_invite accepts empty but validate_invite rejects it
        let invite_str = create_invite(&identity, "", 3600, false, vec![], None).unwrap();
        let result = validate_invite(&invite_str);
        assert!(matches!(result, Err(IdentityError::InviteFormatInvalid(_))));
    }

    #[test]
    fn test_address_hint_too_long_rejected() {
        init();
        let identity = make_identity();
        let long_hint = "a".repeat(MAX_ADDRESS_HINT_LENGTH + 1);
        let result = create_invite(&identity, &long_hint, 3600, false, vec![], None);
        assert!(matches!(result, Err(IdentityError::AddressHintTooLong)));
    }

    #[test]
    fn test_validity_too_large_rejected() {
        init();
        let identity = make_identity();
        let result = create_invite(&identity, "1.2.3.4:5678", MAX_INVITE_VALIDITY_SECS + 1, false, vec![], None);
        assert!(matches!(result, Err(IdentityError::InviteValidityTooLarge)));
    }

    #[test]
    fn test_version_mismatch_rejected() {
        init();
        // Craft an invite with wrong version manually
        let identity = make_identity();
        let payload = protocol::InvitePayload {
            version: 0xFC, // wrong version
            identity_pub: identity.public_key_bytes(),
            x25519_identity_pub: [0u8; 32],
            signed_prekey: [0u8; 32],
            signed_prekey_sig: vec![],
            one_time_prekey: None,
            address_hint: "1.2.3.4:5678".to_string(),
            created_at: now_unix_secs(),
            expires_at: now_unix_secs() + 3600,
            nonce: crypto::random_bytes(16),
            flags: protocol::INVITE_FLAG_LISTENER,
            candidates: vec![],
        };
        let payload_bytes = protocol::serialize(&payload).unwrap();
        let signature = identity.sign(&payload_bytes);
        let signed = protocol::SignedInvite { payload, signature };
        let invite_bytes = protocol::serialize(&signed).unwrap();
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&invite_bytes);
        let invite_str = format!("{}{}", protocol::INVITE_PREFIX, encoded);

        let result = validate_invite(&invite_str);
        assert!(result.is_err(), "wrong version should be rejected");
    }

    #[test]
    fn test_invite_length_limit() {
        init();
        let identity = make_identity();
        // Create an invite with a large payload (many candidates) to approach the limit
        let many_candidates: Vec<protocol::WireCandidate> = (0..20).map(|i| {
            protocol::WireCandidate {
                address: format!("10.0.{}.{}:9000", i / 256, i % 256),
                candidate_type: 0,
                relay_id: None,
            }
        }).collect();
        let result = create_invite(&identity, "1.2.3.4:5678", 3600, false, many_candidates, None);
        // Should either succeed or fail with format error if too big
        match result {
            Ok(invite) => assert!(invite.len() <= MAX_INVITE_LENGTH, "invite {} exceeds max {}", invite.len(), MAX_INVITE_LENGTH),
            Err(IdentityError::InviteFormatInvalid(_)) => {} // also acceptable
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_is_one_time_and_listener_flags() {
        init();
        let identity = make_identity();
        let one_time_str = create_invite(&identity, "1.2.3.4:5678", 3600, true, vec![], None).unwrap();
        let parsed_one_time = validate_invite(&one_time_str).unwrap();
        assert!(is_one_time(&parsed_one_time));
        assert!(is_listener(&parsed_one_time));

        let normal_str = create_invite(&identity, "1.2.3.4:5678", 3600, false, vec![], None).unwrap();
        let parsed_normal = validate_invite(&normal_str).unwrap();
        assert!(!is_one_time(&parsed_normal));
        assert!(is_listener(&parsed_normal));
    }
}
